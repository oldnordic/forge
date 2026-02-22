//! Fluent builder API for workflow construction.
//!
//! Provides a convenient, chainable API for constructing workflows
//! with multiple tasks and dependencies between them.

use crate::workflow::dag::{Workflow, WorkflowError};
use crate::workflow::task::{TaskId, WorkflowTask};
use std::collections::HashMap;
use std::sync::Arc;

/// Fluent builder for constructing workflows.
///
/// WorkflowBuilder provides a chainable API for creating workflows
/// with multiple tasks and dependencies between them.
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::{WorkflowBuilder, MockTask, TaskId};
///
/// let workflow = WorkflowBuilder::new()
///     .add_task(Box::new(MockTask::new("a", "Task A")))
///     .add_task(Box::new(MockTask::new("b", "Task B")))
///     .add_task(Box::new(MockTask::new("c", "Task C")))
///     .dependency(TaskId::new("a"), TaskId::new("b"))
///     .dependency(TaskId::new("b"), TaskId::new("c"))
///     .build()
///     .unwrap();
/// ```
pub struct WorkflowBuilder {
    /// Tasks to be added to the workflow
    tasks: HashMap<TaskId, Box<dyn WorkflowTask>>,
    /// Dependencies between tasks (from, to)
    dependencies: Vec<(TaskId, TaskId)>,
    /// Forge instance for auto-detection (optional)
    forge: Option<Arc<forge_core::Forge>>,
}

impl WorkflowBuilder {
    /// Creates a new WorkflowBuilder.
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            dependencies: Vec::new(),
            forge: None,
        }
    }

    /// Configures the builder with a Forge instance for auto-detection.
    ///
    /// # Arguments
    ///
    /// * `forge` - Forge instance for graph-based dependency detection
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```ignore
    /// let builder = WorkflowBuilder::new()
    ///     .with_auto_detect(&forge)
    ///     .add_task(Box::new(GraphQueryTask::find_symbol("main")));
    /// ```
    pub fn with_auto_detect(mut self, forge: &forge_core::Forge) -> Self {
        self.forge = Some(Arc::new(forge.clone()));
        self
    }

    /// Adds a task to the workflow.
    ///
    /// # Arguments
    ///
    /// * `task` - Boxed trait object implementing WorkflowTask
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```ignore
    /// let builder = WorkflowBuilder::new()
    ///     .add_task(Box::new(MockTask::new("task-1", "First Task")));
    /// ```
    pub fn add_task(mut self, task: Box<dyn WorkflowTask>) -> Self {
        let id = task.id();
        self.tasks.insert(id, task);
        self
    }

    /// Adds a dependency between two tasks.
    ///
    /// Creates a directed edge from `from` to `to`, indicating that `to`
    /// depends on `from` (from must execute first).
    ///
    /// # Arguments
    ///
    /// * `from` - Task ID of the prerequisite (executes first)
    /// * `to` - Task ID of the dependent (executes after)
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Note
    ///
    /// Dependencies are validated when [`build`](Self::build) is called.
    /// Invalid dependencies (cycles, missing tasks) will cause build to fail.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let builder = WorkflowBuilder::new()
    ///     .add_task(Box::new(MockTask::new("a", "Task A")))
    ///     .add_task(Box::new(MockTask::new("b", "Task B")))
    ///     .dependency(TaskId::new("a"), TaskId::new("b"));
    /// ```
    pub fn dependency(mut self, from: TaskId, to: TaskId) -> Self {
        self.dependencies.push((from, to));
        self
    }

    /// Builds the workflow from configured tasks and dependencies.
    ///
    /// # Returns
    ///
    /// - `Ok(Workflow)` - If workflow is valid
    /// - `Err(WorkflowError)` - If validation fails (cycles, missing tasks, empty)
    ///
    /// # Errors
    ///
    /// - `WorkflowError::EmptyWorkflow` - No tasks were added
    /// - `WorkflowError::CycleDetected` - Dependencies contain a cycle
    /// - `WorkflowError::TaskNotFound` - Dependency references non-existent task
    ///
    /// # Example
    ///
    /// ```ignore
    /// let workflow = WorkflowBuilder::new()
    ///     .add_task(Box::new(MockTask::new("task-1", "Task")))
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn build(self) -> Result<Workflow, WorkflowError> {
        // Check for empty workflow
        if self.tasks.is_empty() {
            return Err(WorkflowError::EmptyWorkflow);
        }

        // Create workflow and add all tasks
        let mut workflow = Workflow::new();
        for (_id, task) in self.tasks {
            workflow.add_task(task);
        }

        // Add all dependencies
        for (from, to) in self.dependencies {
            // Validate that both tasks exist
            if !workflow.contains_task(&from) {
                return Err(WorkflowError::TaskNotFound(from));
            }
            if !workflow.contains_task(&to) {
                return Err(WorkflowError::TaskNotFound(to));
            }

            // Add dependency (will fail if cycle detected)
            workflow.add_dependency(from, to)?;
        }

        Ok(workflow)
    }

    /// Builds the workflow with automatic dependency detection.
    ///
    /// This method builds the workflow, runs dependency detection using
    /// the stored Forge instance, applies high-confidence suggestions,
    /// and validates the completed workflow.
    ///
    /// # Returns
    ///
    /// - `Ok(Workflow)` - Valid workflow with auto-detected dependencies
    /// - `Err(WorkflowError)` - If validation fails or no Forge configured
    ///
    /// # Errors
    ///
    /// - `WorkflowError::EmptyWorkflow` - No tasks were added
    /// - `WorkflowError::CycleDetected` - Auto-detection created a cycle
    /// - `WorkflowError::TaskNotFound` - Dependency references non-existent task
    ///
    /// # Example
    ///
    /// ```ignore
    /// let workflow = WorkflowBuilder::new()
    ///     .with_auto_detect(&forge)
    ///     .add_task(Box::new(GraphQueryTask::find_symbol("process_data")))
    ///     .add_task(Box::new(GraphQueryTask::references("process_data")))
    ///     .build_auto_detect()
    ///     .await?;
    /// ```
    pub async fn build_auto_detect(self) -> Result<Workflow, WorkflowError> {
        use crate::workflow::auto_detect::DependencyAnalyzer;

        // Extract forge reference before moving self
        let forge_ref = self.forge.clone();

        // Build workflow without validation first
        let mut workflow = self.build_no_validate()?;

        // Run dependency detection if Forge is configured
        if let Some(forge) = forge_ref {
            let analyzer = DependencyAnalyzer::new(forge.graph());
            let suggestions = analyzer.detect_dependencies(&workflow).await?;

            // Apply high-confidence suggestions
            let high_confidence: Vec<_> = suggestions
                .into_iter()
                .filter(|s| s.is_high_confidence())
                .collect();

            let applied = workflow.apply_suggestions(high_confidence)?;
            // Note: Auto-detected and applied {} dependencies
            let _ = applied; // Suppress unused warning in Phase 8
        }

        Ok(workflow)
    }

    /// Builds workflow without validation (internal helper).
    fn build_no_validate(self) -> Result<Workflow, WorkflowError> {
        // Check for empty workflow
        if self.tasks.is_empty() {
            return Err(WorkflowError::EmptyWorkflow);
        }

        // Create workflow and add all tasks
        let mut workflow = Workflow::new();
        for (_id, task) in self.tasks {
            workflow.add_task(task);
        }

        // Add all dependencies
        for (from, to) in self.dependencies {
            // Validate that both tasks exist
            if !workflow.contains_task(&from) {
                return Err(WorkflowError::TaskNotFound(from));
            }
            if !workflow.contains_task(&to) {
                return Err(WorkflowError::TaskNotFound(to));
            }

            // Add dependency (will fail if cycle detected)
            workflow.add_dependency(from, to)?;
        }

        Ok(workflow)
    }

    /// Creates a sequential workflow from a list of tasks.
    ///
    /// Tasks are executed in the order provided, with each task
    /// depending on the previous task.
    ///
    /// # Arguments
    ///
    /// * `tasks` - Vector of boxed trait objects in execution order
    ///
    /// # Returns
    ///
    /// - `Ok(Workflow)` - If workflow is valid
    /// - `Err(WorkflowError)` - If tasks vector is empty
    ///
    /// # Example
    ///
    /// ```ignore
    /// let workflow = WorkflowBuilder::sequential(vec![
    ///     Box::new(MockTask::new("step-1", "Step 1")),
    ///     Box::new(MockTask::new("step-2", "Step 2")),
    ///     Box::new(MockTask::new("step-3", "Step 3")),
    /// ]).unwrap();
    /// ```
    pub fn sequential(tasks: Vec<Box<dyn WorkflowTask>>) -> Result<Workflow, WorkflowError> {
        if tasks.is_empty() {
            return Err(WorkflowError::EmptyWorkflow);
        }

        // Collect task IDs for dependency chaining
        let mut builder = Self::new();
        let mut prev_id: Option<TaskId> = None;

        for task in tasks {
            let id = task.id();
            if let Some(prev) = prev_id {
                builder = builder.dependency(prev, id.clone());
            }
            prev_id = Some(id);
            builder = builder.add_task(task);
        }

        builder.build()
    }
}

impl Default for WorkflowBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::task::{TaskContext, TaskError, TaskResult, WorkflowTask};
    use async_trait::async_trait;

    // Mock task for testing
    struct MockTask {
        id: TaskId,
        name: String,
        deps: Vec<TaskId>,
    }

    impl MockTask {
        fn new(id: impl Into<TaskId>, name: &str) -> Self {
            Self {
                id: id.into(),
                name: name.to_string(),
                deps: Vec::new(),
            }
        }
    }

    #[async_trait]
    impl WorkflowTask for MockTask {
        async fn execute(&self, _context: &TaskContext) -> Result<TaskResult, TaskError> {
            Ok(TaskResult::Success)
        }

        fn id(&self) -> TaskId {
            self.id.clone()
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn dependencies(&self) -> Vec<TaskId> {
            self.deps.clone()
        }
    }

    #[test]
    fn test_builder_fluent_api() {
        let workflow = WorkflowBuilder::new()
            .add_task(Box::new(MockTask::new("a", "Task A")))
            .add_task(Box::new(MockTask::new("b", "Task B")))
            .add_task(Box::new(MockTask::new("c", "Task C")))
            .dependency(TaskId::new("a"), TaskId::new("b"))
            .dependency(TaskId::new("b"), TaskId::new("c"))
            .build()
            .unwrap();

        assert_eq!(workflow.task_count(), 3);
        assert!(workflow.contains_task(&TaskId::new("a")));
        assert!(workflow.contains_task(&TaskId::new("b")));
        assert!(workflow.contains_task(&TaskId::new("c")));
    }

    #[test]
    fn test_builder_with_dependencies() {
        let workflow = WorkflowBuilder::new()
            .add_task(Box::new(MockTask::new("a", "Task A")))
            .add_task(Box::new(MockTask::new("b", "Task B")))
            .add_task(Box::new(MockTask::new("c", "Task C")))
            .dependency(TaskId::new("a"), TaskId::new("b"))
            .dependency(TaskId::new("a"), TaskId::new("c"))
            .build()
            .unwrap();

        let order = workflow.execution_order().unwrap();
        assert_eq!(order.len(), 3);

        // 'a' must come first (no dependencies, b and c depend on it)
        assert_eq!(order[0], TaskId::new("a"));
    }

    #[test]
    fn test_builder_sequential_helper() {
        let workflow = WorkflowBuilder::sequential(vec![
            Box::new(MockTask::new("step-1", "Step 1")),
            Box::new(MockTask::new("step-2", "Step 2")),
            Box::new(MockTask::new("step-3", "Step 3")),
        ])
        .unwrap();

        assert_eq!(workflow.task_count(), 3);

        let order = workflow.execution_order().unwrap();
        assert_eq!(order.len(), 3);

        // Verify sequential order
        assert_eq!(order[0], TaskId::new("step-1"));
        assert_eq!(order[1], TaskId::new("step-2"));
        assert_eq!(order[2], TaskId::new("step-3"));
    }

    #[test]
    fn test_builder_validation_failure() {
        // Test empty workflow
        let result = WorkflowBuilder::new().build();
        assert!(matches!(result, Err(WorkflowError::EmptyWorkflow)));

        // Test empty sequential
        let result = WorkflowBuilder::sequential(vec![]);
        assert!(matches!(result, Err(WorkflowError::EmptyWorkflow)));
    }

    #[test]
    fn test_builder_cycle_detection() {
        let result = WorkflowBuilder::new()
            .add_task(Box::new(MockTask::new("a", "Task A")))
            .add_task(Box::new(MockTask::new("b", "Task B")))
            .add_task(Box::new(MockTask::new("c", "Task C")))
            .dependency(TaskId::new("a"), TaskId::new("b"))
            .dependency(TaskId::new("b"), TaskId::new("c"))
            .dependency(TaskId::new("c"), TaskId::new("a")) // Creates cycle
            .build();

        assert!(matches!(result, Err(WorkflowError::CycleDetected(_))));
    }

    #[test]
    fn test_builder_missing_task_dependency() {
        let result = WorkflowBuilder::new()
            .add_task(Box::new(MockTask::new("a", "Task A")))
            .dependency(TaskId::new("a"), TaskId::new("nonexistent"))
            .build();

        assert!(matches!(result, Err(WorkflowError::TaskNotFound(_))));
    }

    #[test]
    fn test_builder_default() {
        let builder = WorkflowBuilder::default();
        assert_eq!(builder.tasks.len(), 0);
        assert_eq!(builder.dependencies.len(), 0);
    }

    #[tokio::test]
    async fn test_builder_execute_workflow() {
        use crate::workflow::executor::WorkflowExecutor;

        let workflow = WorkflowBuilder::new()
            .add_task(Box::new(MockTask::new("a", "Task A")))
            .add_task(Box::new(MockTask::new("b", "Task B")))
            .dependency(TaskId::new("a"), TaskId::new("b"))
            .build()
            .unwrap();

        let mut executor = WorkflowExecutor::new(workflow);
        let result = executor.execute().await.unwrap();

        assert!(result.success);
        assert_eq!(result.completed_tasks.len(), 2);
    }

    #[test]
    fn test_builder_with_auto_detect() {
        use forge_core::Forge;

        // Create a forge instance (in-memory for testing)
        let rt = tokio::runtime::Runtime::new().unwrap();
        let forge = rt.block_on(async {
            Forge::open_with_backend(
                "/tmp/test_workflow_builder",
                forge_core::storage::BackendKind::Memory,
            )
            .await
            .unwrap()
        });

        let builder = WorkflowBuilder::new().with_auto_detect(&forge);

        // Verify that forge is stored
        assert!(builder.forge.is_some());
    }

    #[tokio::test]
    async fn test_builder_auto_detect_no_forge() {
        // Test that build_auto_detect works without Forge configured
        let workflow = WorkflowBuilder::new()
            .add_task(Box::new(MockTask::new("a", "Task A")))
            .add_task(Box::new(MockTask::new("b", "Task B")))
            .build_auto_detect()
            .await
            .unwrap();

        assert_eq!(workflow.task_count(), 2);
    }
}

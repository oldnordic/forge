//! Workflow examples demonstrating common patterns.
//!
//! This module provides example workflows that can be used as documentation
//! and templates for building custom workflows.

use crate::workflow::{
    builder::WorkflowBuilder,
    task::{TaskId, TaskResult},
    tasks::{AgentLoopTask, FunctionTask, GraphQueryTask},
    Workflow,
};

/// Creates a linear workflow that executes tasks sequentially.
///
/// Each task depends on the previous task, creating a straight-line
/// execution path.
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::examples::example_linear_workflow;
///
/// let workflow = example_linear_workflow();
/// assert_eq!(workflow.task_count(), 3);
/// ```
#[cfg(doc)]
pub fn example_linear_workflow() -> Result<Workflow, WorkflowError> {
    WorkflowBuilder::sequential(vec![
        Box::new(FunctionTask::new(
            TaskId::new("init"),
            "Initialize".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        )),
        Box::new(FunctionTask::new(
            TaskId::new("process"),
            "Process".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        )),
        Box::new(FunctionTask::new(
            TaskId::new("finalize"),
            "Finalize".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        )),
    ])
}

/// Creates a branching workflow with conditional paths.
///
/// Demonstrates using conditional tasks to create different execution
/// paths based on runtime conditions.
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::examples::example_branching_workflow;
///
/// let workflow = example_branching_workflow();
/// assert_eq!(workflow.task_count(), 4);
/// ```
#[cfg(doc)]
pub fn example_branching_workflow() -> Result<Workflow, WorkflowError> {
    // Condition task
    let condition = Box::new(FunctionTask::new(
        TaskId::new("check"),
        "Check Condition".to_string(),
        |_ctx| async { Ok(TaskResult::Success) },
    ));

    // Then branch
    let then_task = Box::new(FunctionTask::new(
        TaskId::new("then_task"),
        "Then Task".to_string(),
        |_ctx| async { Ok(TaskResult::Success) },
    ));

    // Else branch
    let else_task = Box::new(FunctionTask::new(
        TaskId::new("else_task"),
        "Else Task".to_string(),
        |_ctx| async { Ok(TaskResult::Success) },
    ));

    // Final task
    let finalize = Box::new(FunctionTask::new(
        TaskId::new("finalize"),
        "Finalize".to_string(),
        |_ctx| async { Ok(TaskResult::Success) },
    ));

    // Build workflow with conditional execution
    WorkflowBuilder::new()
        .add_task(condition)
        .add_task(then_task)
        .add_task(else_task)
        .add_task(finalize)
        .dependency(TaskId::new("check"), TaskId::new("then_task"))
        .dependency(TaskId::new("check"), TaskId::new("else_task"))
        .dependency(TaskId::new("then_task"), TaskId::new("finalize"))
        .dependency(TaskId::new("else_task"), TaskId::new("finalize"))
        .build()
}

/// Creates a graph-aware workflow that uses the Forge SDK.
///
/// Demonstrates integrating graph queries into workflows for code
/// intelligence operations.
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::examples::example_graph_aware_workflow;
///
/// let workflow = example_graph_aware_workflow();
/// assert_eq!(workflow.task_count(), 3);
/// ```
#[cfg(doc)]
pub fn example_graph_aware_workflow() -> Result<Workflow, WorkflowError> {
    WorkflowBuilder::new()
        .add_task(Box::new(GraphQueryTask::find_symbol("process_data")))
        .add_task(Box::new(GraphQueryTask::references("process_data")))
        .add_task(Box::new(FunctionTask::new(
            TaskId::new("analyze"),
            "Analyze Results".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        )))
        .dependency(
            TaskId::new("graph_query_FindSymbol"),
            TaskId::new("graph_query_References"),
        )
        .dependency(TaskId::new("graph_query_References"), TaskId::new("analyze"))
        .build()
}

/// Creates an agent workflow with AI-driven operations.
///
/// Demonstrates integrating the AgentLoop into workflows for
/// autonomous code analysis and modification.
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::examples::example_agent_workflow;
///
/// let workflow = example_agent_workflow();
/// assert_eq!(workflow.task_count(), 3);
/// ```
#[cfg(doc)]
pub fn example_agent_workflow() -> Result<Workflow, WorkflowError> {
    let graph_query = Box::new(GraphQueryTask::find_symbol("main"));

    let agent_task = Box::new(AgentLoopTask::new(
        TaskId::new("agent_analysis"),
        "Agent Analysis".to_string(),
        "Analyze the main function and suggest improvements",
    ));

    let report = Box::new(FunctionTask::new(
        TaskId::new("report"),
        "Generate Report".to_string(),
        |_ctx| async { Ok(TaskResult::Success) },
    ));

    WorkflowBuilder::new()
        .add_task(graph_query)
        .add_task(agent_task)
        .add_task(report)
        .dependency(
            TaskId::new("graph_query_FindSymbol"),
            TaskId::new("agent_analysis"),
        )
        .dependency(TaskId::new("agent_analysis"), TaskId::new("report"))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_workflow_example() {
        // Verify the example builds correctly (even if not cfg(doc))
        let result = WorkflowBuilder::sequential(vec![
            Box::new(FunctionTask::new(
                TaskId::new("init"),
                "Initialize".to_string(),
                |_ctx| async { Ok(TaskResult::Success) },
            )),
            Box::new(FunctionTask::new(
                TaskId::new("process"),
                "Process".to_string(),
                |_ctx| async { Ok(TaskResult::Success) },
            )),
            Box::new(FunctionTask::new(
                TaskId::new("finalize"),
                "Finalize".to_string(),
                |_ctx| async { Ok(TaskResult::Success) },
            )),
        ]);

        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert_eq!(workflow.task_count(), 3);
    }

    #[test]
    fn test_branching_workflow_example() {
        let condition = Box::new(FunctionTask::new(
            TaskId::new("check"),
            "Check Condition".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let then_task = Box::new(FunctionTask::new(
            TaskId::new("then_task"),
            "Then Task".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let else_task = Box::new(FunctionTask::new(
            TaskId::new("else_task"),
            "Else Task".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let finalize = Box::new(FunctionTask::new(
            TaskId::new("finalize"),
            "Finalize".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let result = WorkflowBuilder::new()
            .add_task(condition)
            .add_task(then_task)
            .add_task(else_task)
            .add_task(finalize)
            .dependency(TaskId::new("check"), TaskId::new("then_task"))
            .dependency(TaskId::new("check"), TaskId::new("else_task"))
            .dependency(TaskId::new("then_task"), TaskId::new("finalize"))
            .dependency(TaskId::new("else_task"), TaskId::new("finalize"))
            .build();

        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert_eq!(workflow.task_count(), 4);
    }

    #[test]
    fn test_graph_aware_workflow_example() {
        let result = WorkflowBuilder::new()
            .add_task(Box::new(GraphQueryTask::find_symbol("process_data")))
            .add_task(Box::new(GraphQueryTask::references("process_data")))
            .add_task(Box::new(FunctionTask::new(
                TaskId::new("analyze"),
                "Analyze Results".to_string(),
                |_ctx| async { Ok(TaskResult::Success) },
            )))
            .dependency(
                TaskId::new("graph_query_FindSymbol"),
                TaskId::new("graph_query_References"),
            )
            .dependency(TaskId::new("graph_query_References"), TaskId::new("analyze"))
            .build();

        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert_eq!(workflow.task_count(), 3);
    }

    #[test]
    fn test_agent_workflow_example() {
        let graph_query = Box::new(GraphQueryTask::find_symbol("main"));

        let agent_task = Box::new(AgentLoopTask::new(
            TaskId::new("agent_analysis"),
            "Agent Analysis".to_string(),
            "Analyze the main function and suggest improvements",
        ));

        let report = Box::new(FunctionTask::new(
            TaskId::new("report"),
            "Generate Report".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let result = WorkflowBuilder::new()
            .add_task(graph_query)
            .add_task(agent_task)
            .add_task(report)
            .dependency(
                TaskId::new("graph_query_FindSymbol"),
                TaskId::new("agent_analysis"),
            )
            .dependency(TaskId::new("agent_analysis"), TaskId::new("report"))
            .build();

        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert_eq!(workflow.task_count(), 3);
    }
}

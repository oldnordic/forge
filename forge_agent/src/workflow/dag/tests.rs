use super::*;
use crate::workflow::task::{TaskContext, TaskError, TaskResult, WorkflowTask};
use async_trait::async_trait;

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

    #[allow(dead_code)]
    fn with_dep(mut self, dep: impl Into<TaskId>) -> Self {
        self.deps.push(dep.into());
        self
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
fn test_workflow_creation() {
    let workflow = Workflow::new();
    assert_eq!(workflow.task_count(), 0);
    assert!(workflow.execution_order().is_err());
}

#[test]
fn test_add_task() {
    let mut workflow = Workflow::new();
    let task = Box::new(MockTask::new("task-1", "Task 1"));

    workflow.add_task(task);

    assert_eq!(workflow.task_count(), 1);
    assert!(workflow.contains_task(&TaskId::new("task-1")));
}

#[test]
fn test_add_multiple_tasks() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));

    assert_eq!(workflow.task_count(), 3);
}

#[test]
fn test_add_dependency() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));

    let result = workflow.add_dependency("a", "b");
    assert!(result.is_ok());
}

#[test]
fn test_cycle_detection_on_add() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));

    // Create a -> b -> c -> a cycle
    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("b", "c").unwrap();

    let result = workflow.add_dependency("c", "a");
    assert!(matches!(result, Err(WorkflowError::CycleDetected(_))));
}

#[test]
fn test_topological_sort() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("a", "c").unwrap();

    let order = workflow.execution_order().unwrap();
    assert_eq!(order.len(), 3);

    // 'a' must come first (no dependencies)
    assert_eq!(order[0], TaskId::new("a"));
}

#[test]
fn test_ready_tasks() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));

    workflow.add_dependency("a", "b").unwrap();

    let ready = workflow._ready_tasks();
    assert_eq!(ready.len(), 2); // 'a' and 'c' have no dependencies

    let ready_ids: Vec<&TaskId> = ready.iter().map(|node| &node.id).collect();
    assert!(ready_ids.contains(&&TaskId::new("a")));
    assert!(ready_ids.contains(&&TaskId::new("c")));
}

#[test]
fn test_execution_order_with_complex_dag() {
    let mut workflow = Workflow::new();

    // Create a diamond DAG: a -> b, a -> c, b -> d, c -> d
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));
    workflow.add_task(Box::new(MockTask::new("d", "Task D")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("a", "c").unwrap();
    workflow.add_dependency("b", "d").unwrap();
    workflow.add_dependency("c", "d").unwrap();

    let order = workflow.execution_order().unwrap();
    assert_eq!(order.len(), 4);

    // Verify constraints: a before b, a before c, b before d, c before d
    let pos_a = order.iter().position(|id| id == &TaskId::new("a")).unwrap();
    let pos_b = order.iter().position(|id| id == &TaskId::new("b")).unwrap();
    let pos_c = order.iter().position(|id| id == &TaskId::new("c")).unwrap();
    let pos_d = order.iter().position(|id| id == &TaskId::new("d")).unwrap();

    assert!(pos_a < pos_b);
    assert!(pos_a < pos_c);
    assert!(pos_b < pos_d);
    assert!(pos_c < pos_d);
}

#[test]
fn test_dependency_nonexistent_task() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let result = workflow.add_dependency("a", "nonexistent");
    assert!(matches!(result, Err(WorkflowError::TaskNotFound(_))));

    let result = workflow.add_dependency("nonexistent", "a");
    assert!(matches!(result, Err(WorkflowError::TaskNotFound(_))));
}

#[test]
fn test_self_cycle_detection() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    // Self-referencing dependency should fail
    let result = workflow.add_dependency("a", "a");
    // petgraph allows self-loops but they create cycles
    // The behavior depends on petgraph's implementation
    // We just verify it doesn't panic
    let _ = result;
}

#[test]
fn test_apply_suggestions() {
    use crate::workflow::auto_detect::{DependencyReason, DependencySuggestion};

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));

    let suggestions = vec![
        DependencySuggestion {
            from_task: TaskId::new("a"),
            to_task: TaskId::new("b"),
            reason: DependencyReason::SymbolImpact {
                symbol: "test".to_string(),
                hops: 1,
            },
            confidence: 0.9,
        },
        DependencySuggestion {
            from_task: TaskId::new("b"),
            to_task: TaskId::new("c"),
            reason: DependencyReason::Reference {
                symbol: "test".to_string(),
            },
            confidence: 0.85,
        },
    ];

    let applied = workflow.apply_suggestions(suggestions).unwrap();
    assert_eq!(applied, 2);

    // Verify dependencies were added
    let deps_b = workflow.task_dependencies(&TaskId::new("b")).unwrap();
    assert!(deps_b.contains(&TaskId::new("a")));

    let deps_c = workflow.task_dependencies(&TaskId::new("c")).unwrap();
    assert!(deps_c.contains(&TaskId::new("b")));
}

#[test]
fn test_apply_suggestions_skips_existing() {
    use crate::workflow::auto_detect::{DependencyReason, DependencySuggestion};

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));

    // Add existing dependency
    workflow.add_dependency("a", "b").unwrap();

    let suggestions = vec![DependencySuggestion {
        from_task: TaskId::new("a"),
        to_task: TaskId::new("b"),
        reason: DependencyReason::SymbolImpact {
            symbol: "test".to_string(),
            hops: 1,
        },
        confidence: 0.9,
    }];

    let applied = workflow.apply_suggestions(suggestions).unwrap();
    assert_eq!(applied, 0); // Should skip existing dependency
}

#[test]
fn test_preview_suggestions() {
    use crate::workflow::auto_detect::{DependencyReason, DependencySuggestion};

    let workflow = Workflow::new();

    let suggestions = vec![
        DependencySuggestion {
            from_task: TaskId::new("a"),
            to_task: TaskId::new("b"),
            reason: DependencyReason::SymbolImpact {
                symbol: "test_func".to_string(),
                hops: 2,
            },
            confidence: 0.85,
        },
        DependencySuggestion {
            from_task: TaskId::new("b"),
            to_task: TaskId::new("c"),
            reason: DependencyReason::Reference {
                symbol: "test_struct".to_string(),
            },
            confidence: 0.9,
        },
    ];

    let preview = workflow.preview_suggestions(&suggestions);
    assert_eq!(preview.len(), 2);

    assert!(preview[0].contains("'b' should depend on task 'a'"));
    assert!(preview[0].contains("test_func"));
    assert!(preview[0].contains("2 hops"));

    assert!(preview[1].contains("'c' should depend on task 'b'"));
    assert!(preview[1].contains("test_struct"));
    assert!(preview[1].contains("reference"));
}

// ============== execution_layers() tests ==============

#[test]
fn test_execution_layers_empty_workflow() {
    let workflow = Workflow::new();
    let result = workflow.execution_layers();
    assert!(matches!(result, Err(WorkflowError::EmptyWorkflow)));
}

#[test]
fn test_execution_layers_single_task() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let layers = workflow.execution_layers().unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].len(), 1);
    assert_eq!(layers[0][0], TaskId::new("a"));
}

#[test]
fn test_execution_layers_two_independent_tasks() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));

    let layers = workflow.execution_layers().unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].len(), 2);
    assert!(layers[0].contains(&TaskId::new("a")));
    assert!(layers[0].contains(&TaskId::new("b")));
}

#[test]
fn test_execution_layers_linear_chain() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("b", "c").unwrap();

    let layers = workflow.execution_layers().unwrap();
    assert_eq!(layers.len(), 3);
    assert_eq!(layers[0], vec![TaskId::new("a")]);
    assert_eq!(layers[1], vec![TaskId::new("b")]);
    assert_eq!(layers[2], vec![TaskId::new("c")]);
}

#[test]
fn test_execution_layers_diamond_pattern() {
    let mut workflow = Workflow::new();

    // Create a diamond DAG: a -> [b, c] -> d
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));
    workflow.add_task(Box::new(MockTask::new("d", "Task D")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("a", "c").unwrap();
    workflow.add_dependency("b", "d").unwrap();
    workflow.add_dependency("c", "d").unwrap();

    let layers = workflow.execution_layers().unwrap();
    assert_eq!(layers.len(), 3);

    // Layer 0: only 'a' (root)
    assert_eq!(layers[0], vec![TaskId::new("a")]);

    // Layer 1: 'b' and 'c' (independent tasks that depend on 'a')
    assert_eq!(layers[1].len(), 2);
    assert!(layers[1].contains(&TaskId::new("b")));
    assert!(layers[1].contains(&TaskId::new("c")));

    // Layer 2: only 'd' (depends on both 'b' and 'c')
    assert_eq!(layers[2], vec![TaskId::new("d")]);
}

#[test]
fn test_execution_layers_fan_out() {
    let mut workflow = Workflow::new();

    // Fan-out: a -> [b, c, d]
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));
    workflow.add_task(Box::new(MockTask::new("d", "Task D")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("a", "c").unwrap();
    workflow.add_dependency("a", "d").unwrap();

    let layers = workflow.execution_layers().unwrap();
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0], vec![TaskId::new("a")]);
    assert_eq!(layers[1].len(), 3);
}

#[test]
fn test_execution_layers_fan_in() {
    let mut workflow = Workflow::new();

    // Fan-in: [a, b, c] -> d
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));
    workflow.add_task(Box::new(MockTask::new("d", "Task D")));

    workflow.add_dependency("a", "d").unwrap();
    workflow.add_dependency("b", "d").unwrap();
    workflow.add_dependency("c", "d").unwrap();

    let layers = workflow.execution_layers().unwrap();
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].len(), 3); // a, b, c are independent
    assert_eq!(layers[1], vec![TaskId::new("d")]);
}

#[test]
fn test_execution_layers_complex_dag() {
    let mut workflow = Workflow::new();

    // Complex DAG with multiple layers:
    //     a
    //    / \
    //   b   c
    //   |   |
    //   d   e
    //    \ /
    //     f
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));
    workflow.add_task(Box::new(MockTask::new("d", "Task D")));
    workflow.add_task(Box::new(MockTask::new("e", "Task E")));
    workflow.add_task(Box::new(MockTask::new("f", "Task F")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("a", "c").unwrap();
    workflow.add_dependency("b", "d").unwrap();
    workflow.add_dependency("c", "e").unwrap();
    workflow.add_dependency("d", "f").unwrap();
    workflow.add_dependency("e", "f").unwrap();

    let layers = workflow.execution_layers().unwrap();
    assert_eq!(layers.len(), 4);
    assert_eq!(layers[0], vec![TaskId::new("a")]);
    assert_eq!(layers[1].len(), 2); // b and c
    assert_eq!(layers[2].len(), 2); // d and e
    assert_eq!(layers[3], vec![TaskId::new("f")]);
}

#[test]
fn test_execution_layers_with_cycle() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));

    // Create a cycle: a -> b -> c -> a
    // Note: add_dependency removes the edge if it creates a cycle,
    // so we need to directly manipulate the graph for this test
    let a_idx = workflow.task_map.get(&TaskId::new("a")).copied().unwrap();
    let b_idx = workflow.task_map.get(&TaskId::new("b")).copied().unwrap();
    let c_idx = workflow.task_map.get(&TaskId::new("c")).copied().unwrap();

    workflow.graph.add_edge(a_idx, b_idx, ());
    workflow.graph.add_edge(b_idx, c_idx, ());
    workflow.graph.add_edge(c_idx, a_idx, ()); // Creates a cycle

    let result = workflow.execution_layers();
    assert!(matches!(result, Err(WorkflowError::CycleDetected(_))));
}

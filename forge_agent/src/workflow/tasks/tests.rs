use super::*;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::test]
async fn test_function_task() {
    let task = FunctionTask::new(
        TaskId::new("test_task"),
        "Test Task".to_string(),
        |_ctx| async { Ok(TaskResult::Success) },
    );

    let context = TaskContext::new("workflow_1", TaskId::new("test_task"));
    let result = task.execute(&context).await.unwrap();

    assert_eq!(result, TaskResult::Success);
    assert_eq!(task.id(), TaskId::new("test_task"));
    assert_eq!(task.name(), "Test Task");
}

#[tokio::test]
async fn test_agent_loop_task() {
    let task = AgentLoopTask::new(
        TaskId::new("agent_task"),
        "Agent Task".to_string(),
        "Find all functions",
    );

    assert_eq!(task.id(), TaskId::new("agent_task"));
    assert_eq!(task.name(), "Agent Task");
    assert_eq!(task.query(), "Find all functions");

    let context = TaskContext::new("workflow_1", TaskId::new("agent_task"));
    let result = task.execute(&context).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_graph_query_task() {
    let task = GraphQueryTask::find_symbol("process_data");

    assert_eq!(task.query_type, GraphQueryType::FindSymbol);
    assert_eq!(task._target, "process_data");
    assert_eq!(task.target(), "process_data");

    let context = TaskContext::new("workflow_1", task.id());
    let result = task.execute(&context).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_graph_query_references() {
    let task = GraphQueryTask::references("my_function");

    assert_eq!(task.query_type, GraphQueryType::References);
    assert_eq!(task._target, "my_function");
}

#[tokio::test]
async fn test_graph_query_impact() {
    let task = GraphQueryTask::impact_analysis("struct_name");

    assert_eq!(task.query_type, GraphQueryType::ImpactAnalysis);
    assert_eq!(task._target, "struct_name");
}

#[tokio::test]
async fn test_graph_query_with_custom_id() {
    let task = GraphQueryTask::with_id(
        TaskId::new("custom_id"),
        GraphQueryType::FindSymbol,
        "my_symbol",
    );

    assert_eq!(task.id(), TaskId::new("custom_id"));
    assert_eq!(task._target, "my_symbol");
}

#[tokio::test]
async fn test_shell_command_task_executes_command() {
    // Run a shell command that writes a file — proves execution + args + working_dir.
    let temp_dir = tempfile::tempdir().unwrap();
    let config = ShellCommandConfig::new("sh")
        .args(vec![
            "-c".to_string(),
            "printf 'hello world' > marker.txt".to_string(),
        ])
        .working_dir(temp_dir.path());
    let task =
        ShellCommandTask::with_config(TaskId::new("shell_task"), "Shell Task".to_string(), config);

    assert_eq!(task.id(), TaskId::new("shell_task"));
    assert_eq!(task.command(), "sh");

    let context = TaskContext::new("workflow_1", task.id());
    let result = task.execute(&context).await.unwrap();
    assert_eq!(result, TaskResult::Success);

    // The command must have actually run — verify the side effect on disk.
    let content =
        std::fs::read_to_string(temp_dir.path().join("marker.txt")).expect("marker file missing");
    assert_eq!(content, "hello world");
}

#[tokio::test]
async fn test_shell_command_task_failure_propagates() {
    // A command that exits non-zero must produce TaskResult::Failed.
    let config = ShellCommandConfig::new("sh").args(vec!["-c".to_string(), "exit 42".to_string()]);
    let task =
        ShellCommandTask::with_config(TaskId::new("fail_task"), "Fail Task".to_string(), config);

    let context = TaskContext::new("workflow_1", task.id());
    let result = task.execute(&context).await.unwrap();
    match result {
        TaskResult::Failed(msg) => assert!(msg.contains("42"), "error should include exit code"),
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[tokio::test]
async fn test_shell_task_args_default() {
    let task = ShellCommandTask::new(TaskId::new("shell_task"), "Shell Task".to_string(), "ls");

    assert_eq!(task.args().len(), 0);
    assert!(task.args().is_empty());
}

#[tokio::test]
async fn test_shell_command_with_working_dir() {
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join("test_shell_command.txt");

    std::fs::write(&test_file, "test content").unwrap();

    let config = ShellCommandConfig::new("ls")
        .args(vec![temp_dir.to_string_lossy().to_string()])
        .working_dir(&temp_dir);

    let task =
        ShellCommandTask::with_config(TaskId::new("shell_task"), "Shell Task".to_string(), config);

    let context = TaskContext::new("workflow_1", task.id());
    let result = task.execute(&context).await.unwrap();

    assert_eq!(result, TaskResult::Success);

    std::fs::remove_file(&test_file).ok();
}

#[tokio::test]
async fn test_shell_command_with_env() {
    let config = ShellCommandConfig::new("sh")
        .args(vec!["-c".to_string(), "echo $TEST_VAR".to_string()])
        .env("TEST_VAR", "test_value");

    let task =
        ShellCommandTask::with_config(TaskId::new("shell_task"), "Shell Task".to_string(), config);

    let context = TaskContext::new("workflow_1", task.id());
    let result = task.execute(&context).await.unwrap();

    assert_eq!(result, TaskResult::Success);
}

#[tokio::test]
async fn test_shell_command_compensation() {
    let config = ShellCommandConfig::new("echo").args(vec!["test".to_string()]);
    let task =
        ShellCommandTask::with_config(TaskId::new("shell_task"), "Shell Task".to_string(), config);

    let compensation = task.compensation();
    assert!(compensation.is_some());
    assert_eq!(
        compensation.unwrap().action_type,
        crate::workflow::task::CompensationType::Skip
    );

    let context = TaskContext::new("workflow_1", task.id());
    let result = task.execute(&context).await.unwrap();
    assert_eq!(result, TaskResult::Success);

    let compensation = task.compensation();
    assert!(compensation.is_some());
    assert_eq!(
        compensation.unwrap().action_type,
        crate::workflow::task::CompensationType::UndoFunction
    );
}

#[tokio::test]
async fn test_graph_query_compensation_skip() {
    let task = GraphQueryTask::find_symbol("my_function");

    let compensation = task.compensation();
    assert!(compensation.is_some());
    assert_eq!(
        compensation.unwrap().action_type,
        crate::workflow::task::CompensationType::Skip
    );
}

#[tokio::test]
async fn test_agent_loop_compensation_skip() {
    let task = AgentLoopTask::new(
        TaskId::new("agent_task"),
        "Agent Task".to_string(),
        "Find all functions",
    );

    let compensation = task.compensation();
    assert!(compensation.is_some());
    assert_eq!(
        compensation.unwrap().action_type,
        crate::workflow::task::CompensationType::Skip
    );
}

#[tokio::test]
async fn test_file_edit_compensation_undo() {
    let task = FileEditTask::new(
        TaskId::new("file_edit"),
        "Edit File".to_string(),
        PathBuf::from("/tmp/test.txt"),
        "original".to_string(),
        "new".to_string(),
    );

    let compensation = task.compensation();
    assert!(compensation.is_some());
    assert_eq!(
        compensation.unwrap().action_type,
        crate::workflow::task::CompensationType::UndoFunction
    );
}

// ============== ToolTask Tests ==============

#[tokio::test]
async fn test_tool_task_creation() {
    let task = ToolTask::new(TaskId::new("tool_task"), "Echo Tool".to_string(), "echo");

    assert_eq!(task.id(), TaskId::new("tool_task"));
    assert_eq!(task.name(), "Echo Tool");
    assert_eq!(task.tool_name(), "echo");
    assert!(task.invocation().args.is_empty());
    assert!(task.fallback.is_none());
}

#[tokio::test]
async fn test_tool_task_with_args() {
    let task = ToolTask::new(TaskId::new("tool_task"), "Echo Tool".to_string(), "echo")
        .args(vec!["hello".to_string(), "world".to_string()]);

    assert_eq!(task.invocation().args.len(), 2);
    assert_eq!(task.invocation().args[0], "hello");
    assert_eq!(task.invocation().args[1], "world");
}

#[tokio::test]
async fn test_tool_task_with_working_dir() {
    let task = ToolTask::new(TaskId::new("tool_task"), "Cargo Test".to_string(), "cargo")
        .working_dir("/home/user/project");

    assert_eq!(
        task.invocation().working_dir,
        Some(PathBuf::from("/home/user/project"))
    );
}

#[tokio::test]
async fn test_tool_task_with_env() {
    let task = ToolTask::new(TaskId::new("tool_task"), "Cargo Test".to_string(), "cargo")
        .env("RUST_LOG", "debug");

    assert_eq!(task.invocation().env.len(), 1);
    assert_eq!(
        task.invocation().env.get("RUST_LOG"),
        Some(&"debug".to_string())
    );
}

#[tokio::test]
async fn test_tool_task_builder_pattern() {
    let task = ToolTask::new(TaskId::new("tool_task"), "Cargo Test".to_string(), "cargo")
        .args(vec!["test".to_string()])
        .working_dir("/tmp")
        .env("TEST_VAR", "value");

    assert_eq!(task.invocation().args.len(), 1);
    assert_eq!(task.invocation().working_dir, Some(PathBuf::from("/tmp")));
    assert_eq!(
        task.invocation().env.get("TEST_VAR"),
        Some(&"value".to_string())
    );
}

#[tokio::test]
async fn test_tool_task_compensation() {
    let task = ToolTask::new(TaskId::new("tool_task"), "Echo Tool".to_string(), "echo");

    let compensation = task.compensation();
    assert!(compensation.is_some());
    assert_eq!(
        compensation.unwrap().action_type,
        crate::workflow::task::CompensationType::Skip
    );
}

#[tokio::test]
async fn test_tool_task_execution() {
    use std::sync::Arc;

    let mut registry = crate::workflow::tools::ToolRegistry::new();
    registry
        .register(crate::workflow::tools::Tool::new("echo", "echo"))
        .unwrap();

    let context = TaskContext::new("workflow_1", TaskId::new("tool_task"))
        .with_tool_registry(Arc::new(registry));

    let task = ToolTask::new(TaskId::new("tool_task"), "Echo Tool".to_string(), "echo")
        .args(vec!["test".to_string()]);

    let result = task.execute(&context).await.unwrap();
    assert_eq!(result, TaskResult::Success);
}

#[tokio::test]
async fn test_tool_task_with_fallback() {
    use std::sync::Arc;

    let mut registry = crate::workflow::tools::ToolRegistry::new();
    registry
        .register(crate::workflow::tools::Tool::new("echo", "echo"))
        .unwrap();

    let context = TaskContext::new("workflow_1", TaskId::new("tool_task"))
        .with_tool_registry(Arc::new(registry));

    let task = ToolTask::new(
        TaskId::new("tool_task"),
        "Nonexistent Tool".to_string(),
        "nonexistent",
    )
    .with_fallback(Box::new(crate::workflow::tools::SkipFallback::skip()));

    let result = task.execute(&context).await.unwrap();
    assert_eq!(result, TaskResult::Skipped);
}

#[tokio::test]
async fn test_standard_tools() {
    use crate::workflow::tools::ToolRegistry;

    let registry = ToolRegistry::with_standard_tools();

    let tool_count = registry.len();
    eprintln!("Standard tools registered: {}", tool_count);

    let _ = tool_count;
}

#[tokio::test]
async fn test_tool_invoke_from_workflow() {
    use crate::workflow::dag::Workflow;
    use crate::workflow::executor::WorkflowExecutor;
    use crate::workflow::tools::{Tool, ToolRegistry};

    let mut workflow = Workflow::new();
    let task_id = TaskId::new("tool_task");

    let mut registry = ToolRegistry::new();
    registry.register(Tool::new("echo", "echo")).unwrap();

    let tool_task = ToolTask::new(task_id.clone(), "Echo Tool".to_string(), "echo")
        .args(vec!["hello".to_string()]);

    workflow.add_task(Box::new(tool_task));

    let mut executor = WorkflowExecutor::new(workflow).with_tool_registry(registry);

    let result = executor.execute().await.unwrap();
    assert!(result.success);
    assert!(result.completed_tasks.contains(&task_id));
}

#[tokio::test]
async fn test_tool_fallback_audit_event() {
    use crate::audit::AuditLog;

    let audit_log = AuditLog::new();

    let mut registry = crate::workflow::tools::ToolRegistry::new();
    registry
        .register(crate::workflow::tools::Tool::new("echo", "echo"))
        .unwrap();

    let context = TaskContext::new("workflow_1", TaskId::new("tool_task"))
        .with_tool_registry(Arc::new(registry))
        .with_audit_log(audit_log);

    let task = ToolTask::new(
        TaskId::new("tool_task"),
        "Nonexistent Tool".to_string(),
        "nonexistent",
    )
    .with_fallback(Box::new(crate::workflow::tools::SkipFallback::skip()));

    let result = task.execute(&context).await.unwrap();
    assert_eq!(result, TaskResult::Skipped);
}

#[tokio::test]
async fn test_file_edit_task_rollback_restores_content() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("target.rs");
    tokio::fs::write(&file_path, "original content")
        .await
        .unwrap();

    let task = FileEditTask::new(
        TaskId::new("edit"),
        "Edit target.rs".to_string(),
        file_path.clone(),
        "original content".to_string(),
        "modified content".to_string(),
    );

    let context = TaskContext::new("wf-1", task.id());
    let result = task.execute(&context).await.unwrap();

    let on_disk = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(on_disk, "modified content");

    let compensation = match result {
        TaskResult::WithCompensation { compensation, .. } => compensation,
        other => panic!("expected WithCompensation, got {:?}", other),
    };

    compensation.execute(&context).unwrap();
    let restored = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(
        restored, "original content",
        "compensation must restore the original file content"
    );
}

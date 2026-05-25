use super::*;
use crate::workflow::rollback::ToolCompensation;
use crate::workflow::task::TaskResult;
use async_trait::async_trait;

#[tokio::test]
async fn test_retry_fallback_retries_transient_errors() {
    let fallback = RetryFallback::new(3, 100);
    let invocation = ToolInvocation::new("test_tool").args(vec!["arg1".to_string()]);

    let error = ToolError::Timeout("Test timeout".to_string());
    let result = fallback.handle(&error, &invocation).await;

    assert!(matches!(result, FallbackResult::Retry(_)));
}

#[tokio::test]
async fn test_retry_fallback_fails_on_tool_not_found() {
    let fallback = RetryFallback::new(3, 100);
    let invocation = ToolInvocation::new("nonexistent_tool");

    let error = ToolError::ToolNotFound("nonexistent_tool".to_string());
    let result = fallback.handle(&error, &invocation).await;

    assert!(matches!(result, FallbackResult::Fail(_)));
}

#[tokio::test]
async fn test_skip_fallback_success() {
    let fallback = SkipFallback::success();
    let invocation = ToolInvocation::new("test_tool");
    let error = ToolError::ToolNotFound("test_tool".to_string());

    let result = fallback.handle(&error, &invocation).await;

    assert!(matches!(result, FallbackResult::Skip(TaskResult::Success)));
}

#[tokio::test]
async fn test_skip_fallback_skip() {
    let fallback = SkipFallback::skip();
    let invocation = ToolInvocation::new("test_tool");
    let error = ToolError::ToolNotFound("test_tool".to_string());

    let result = fallback.handle(&error, &invocation).await;

    assert!(matches!(result, FallbackResult::Skip(TaskResult::Skipped)));
}

#[tokio::test]
async fn test_skip_fallback_custom_result() {
    let fallback = SkipFallback::new(TaskResult::Failed("Custom failure".to_string()));
    let invocation = ToolInvocation::new("test_tool");
    let error = ToolError::ToolNotFound("test_tool".to_string());

    let result = fallback.handle(&error, &invocation).await;

    assert!(matches!(
        result,
        FallbackResult::Skip(TaskResult::Failed(_))
    ));
    if let FallbackResult::Skip(TaskResult::Failed(msg)) = result {
        assert_eq!(msg, "Custom failure");
    }
}

#[tokio::test]
async fn test_chain_fallback_tries_handlers_in_sequence() {
    let invocation = ToolInvocation::new("test_tool");
    let error = ToolError::Timeout("Test timeout".to_string());

    let fallback = ChainFallback::new()
        .with_handler(SkipFallback::skip())
        .with_handler(SkipFallback::success());

    let result = fallback.handle(&error, &invocation).await;

    assert!(matches!(result, FallbackResult::Skip(TaskResult::Skipped)));
}

#[tokio::test]
async fn test_chain_fallback_all_handlers_fail() {
    let invocation = ToolInvocation::new("test_tool");
    let error = ToolError::Timeout("Test timeout".to_string());

    #[derive(Clone)]
    struct AlwaysFail;
    #[async_trait]
    impl FallbackHandler for AlwaysFail {
        async fn handle(&self, error: &ToolError, _invocation: &ToolInvocation) -> FallbackResult {
            FallbackResult::Fail(error.clone())
        }
    }

    let fallback = ChainFallback::new()
        .with_handler(AlwaysFail)
        .with_handler(AlwaysFail);

    let result = fallback.handle(&error, &invocation).await;

    assert!(matches!(result, FallbackResult::Fail(_)));
}

#[tokio::test]
async fn test_chain_fallback_empty_chain() {
    let invocation = ToolInvocation::new("test_tool");
    let error = ToolError::Timeout("Test timeout".to_string());

    let fallback = ChainFallback::new();
    let result = fallback.handle(&error, &invocation).await;

    assert!(matches!(result, FallbackResult::Fail(_)));
}

#[test]
fn test_tool_creation() {
    let tool = Tool::new("magellan", "/usr/bin/magellan");

    assert_eq!(tool.name, "magellan");
    assert_eq!(tool.executable, PathBuf::from("/usr/bin/magellan"));
    assert!(tool.default_args.is_empty());
    assert!(tool.description.is_empty());
}

#[test]
fn test_tool_with_default_args() {
    let tool = Tool::new("magellan", "/usr/bin/magellan")
        .default_args(vec!["--db".to_string(), ".forge/graph.db".to_string()]);

    assert_eq!(tool.default_args.len(), 2);
    assert_eq!(tool.default_args[0], "--db");
    assert_eq!(tool.default_args[1], ".forge/graph.db");
}

#[test]
fn test_tool_with_description() {
    let tool = Tool::new("magellan", "/usr/bin/magellan").description("Graph-based code indexer");

    assert_eq!(tool.description, "Graph-based code indexer");
}

#[test]
fn test_tool_builder_pattern() {
    let tool = Tool::new("magellan", "/usr/bin/magellan")
        .default_args(vec!["--db".to_string(), ".forge/graph.db".to_string()])
        .description("Graph-based code indexer");

    assert_eq!(tool.name, "magellan");
    assert_eq!(tool.default_args.len(), 2);
    assert_eq!(tool.description, "Graph-based code indexer");
}

#[test]
fn test_tool_invocation_creation() {
    let invocation = ToolInvocation::new("magellan");

    assert_eq!(invocation.tool_name, "magellan");
    assert!(invocation.args.is_empty());
    assert!(invocation.working_dir.is_none());
    assert!(invocation.env.is_empty());
}

#[test]
fn test_tool_invocation_with_args() {
    let invocation = ToolInvocation::new("magellan").args(vec![
        "find".to_string(),
        "--name".to_string(),
        "symbol".to_string(),
    ]);

    assert_eq!(invocation.args.len(), 3);
    assert_eq!(invocation.args[0], "find");
}

#[test]
fn test_tool_invocation_with_working_dir() {
    let invocation = ToolInvocation::new("magellan").working_dir("/home/user/project");

    assert_eq!(
        invocation.working_dir,
        Some(PathBuf::from("/home/user/project"))
    );
}

#[test]
fn test_tool_invocation_with_env() {
    let invocation = ToolInvocation::new("magellan").env("RUST_LOG", "debug");

    assert_eq!(invocation.env.len(), 1);
    assert_eq!(invocation.env.get("RUST_LOG"), Some(&"debug".to_string()));
}

#[test]
fn test_tool_invocation_display() {
    let invocation =
        ToolInvocation::new("magellan").args(vec!["find".to_string(), "--name".to_string()]);

    let display = format!("{}", invocation);
    assert!(display.contains("magellan"));
    assert!(display.contains("find"));
}

#[test]
fn test_tool_result_success() {
    let result = ToolResult::success("output".to_string());

    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stdout, "output");
    assert!(result.stderr.is_empty());
    assert!(result.success);
}

#[test]
fn test_tool_result_failure() {
    let result = ToolResult::failure(1, "error".to_string());

    assert_eq!(result.exit_code, Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(result.stderr, "error");
    assert!(!result.success);
}

#[test]
fn test_tool_result_new() {
    let result = ToolResult::new(Some(0), "stdout".to_string(), "stderr".to_string());

    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stdout, "stdout");
    assert_eq!(result.stderr, "stderr");
    assert!(result.success);
}

#[test]
fn test_tool_result_none_exit_code() {
    let result = ToolResult::new(None, "stdout".to_string(), "stderr".to_string());

    assert_eq!(result.exit_code, None);
    assert!(!result.success);
}

#[test]
fn test_tool_registry_new() {
    let registry = ToolRegistry::new();

    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_register_tool() {
    let mut registry = ToolRegistry::new();
    let tool = Tool::new("magellan", "/usr/bin/magellan");

    registry.register(tool).unwrap();

    assert_eq!(registry.len(), 1);
    assert!(registry.is_registered("magellan"));
}

#[test]
fn test_register_duplicate_tool() {
    let mut registry = ToolRegistry::new();

    registry
        .register(Tool::new("magellan", "/usr/bin/magellan"))
        .unwrap();

    let result = registry.register(Tool::new("magellan", "/usr/bin/magellan"));

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        ToolError::AlreadyRegistered("magellan".to_string())
    );
}

#[test]
fn test_get_tool() {
    let mut registry = ToolRegistry::new();

    registry
        .register(Tool::new("magellan", "/usr/bin/magellan"))
        .unwrap();

    let tool = registry.get("magellan");
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().name, "magellan");
}

#[test]
fn test_get_nonexistent_tool() {
    let registry = ToolRegistry::new();

    let tool = registry.get("magellan");
    assert!(tool.is_none());
}

#[test]
fn test_list_tools() {
    let mut registry = ToolRegistry::new();

    registry
        .register(Tool::new("magellan", "/usr/bin/magellan"))
        .unwrap();
    registry
        .register(Tool::new("cargo", "/usr/bin/cargo"))
        .unwrap();

    let tools = registry.list_tools();
    assert_eq!(tools.len(), 2);
    assert!(tools.contains(&"magellan"));
    assert!(tools.contains(&"cargo"));
}

#[test]
fn test_is_registered() {
    let mut registry = ToolRegistry::new();

    registry
        .register(Tool::new("magellan", "/usr/bin/magellan"))
        .unwrap();

    assert!(registry.is_registered("magellan"));
    assert!(!registry.is_registered("cargo"));
}

#[test]
fn test_tool_registry_default() {
    let registry = ToolRegistry::default();

    let _ = registry.len();
}

#[tokio::test]
async fn test_invoke_basic_tool() {
    let mut registry = ToolRegistry::new();

    registry.register(Tool::new("echo", "echo")).unwrap();

    let invocation = ToolInvocation::new("echo").args(vec!["hello".to_string()]);

    let result = registry.invoke(&invocation).await.unwrap();

    assert!(result.result.success);
    let trimmed = result.result.stdout.trim();
    assert_eq!(trimmed, "hello", "Expected 'hello', got '{}'", trimmed);
}

#[tokio::test]
async fn test_invoke_with_default_args() {
    let mut registry = ToolRegistry::new();

    registry
        .register(Tool::new("echo", "/bin/echo").default_args(vec!["-n".to_string()]))
        .unwrap();

    let invocation = ToolInvocation::new("echo").args(vec!["test".to_string()]);

    let result = registry.invoke(&invocation).await.unwrap();

    assert!(result.result.success);
}

#[tokio::test]
async fn test_invoke_nonexistent_tool() {
    let registry = ToolRegistry::new();

    let invocation = ToolInvocation::new("nonexistent").args(vec!["arg".to_string()]);

    let result = registry.invoke(&invocation).await;

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        ToolError::ToolNotFound("nonexistent".to_string())
    );
}

#[test]
fn test_process_guard_creation() {
    let guard = ProcessGuard::new(12345, "test_tool");

    assert_eq!(guard.pid(), 12345);
    assert!(!guard.is_terminated());
}

#[test]
fn test_process_guard_display() {
    let guard = ProcessGuard::new(12345, "test_tool");

    let display = format!("{}", guard);
    assert!(display.contains("12345"));
    assert!(display.contains("test_tool"));
}

#[test]
fn test_process_guard_clone() {
    let guard1 = ProcessGuard::new(12345, "test_tool");
    let guard2 = guard1.clone();

    assert_eq!(guard1.pid(), guard2.pid());
    assert_eq!(guard1.tool_name, guard2.tool_name);

    assert_eq!(guard1.is_terminated(), guard2.is_terminated());
}

#[test]
fn test_process_guard_into_tool_compensation() {
    let guard = ProcessGuard::new(12345, "test_tool");

    let compensation: ToolCompensation = guard.into();

    assert!(compensation.description.contains("12345"));
    assert!(compensation.description.contains("test_tool"));
}

#[tokio::test]
async fn test_tool_invocation_result_completed() {
    let result = ToolResult::success("output".to_string());
    let invocation_result = ToolInvocationResult::completed(result);

    assert!(invocation_result.guard.is_none());
    assert!(invocation_result.result.success);
}

#[tokio::test]
async fn test_tool_invocation_result_with_guard() {
    let result = ToolResult::failure(1, "error".to_string());
    let guard = ProcessGuard::new(12345, "test_tool");
    let invocation_result = ToolInvocationResult::new(result, Some(guard));

    assert!(invocation_result.guard.is_some());
    assert!(!invocation_result.result.success);
}

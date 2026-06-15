use super::*;

#[tokio::test]
async fn test_executor_with_forge_passes_context() {
    use crate::workflow::task::TaskError;
    use forgekit_core::Forge;
    use tempfile::TempDir;

    struct ForgeCheckTask;

    #[async_trait]
    impl WorkflowTask for ForgeCheckTask {
        async fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
            if context.forge.is_some() {
                Ok(TaskResult::Success)
            } else {
                Err(TaskError::ExecutionFailed(
                    "no forge in context".to_string(),
                ))
            }
        }

        fn id(&self) -> TaskId {
            TaskId::new("forge-check")
        }

        fn name(&self) -> &str {
            "ForgeCheckTask"
        }
    }

    let temp_dir = TempDir::new().unwrap();
    let forge = Forge::open(temp_dir.path()).await.unwrap();

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(ForgeCheckTask));

    let mut executor = WorkflowExecutor::new(workflow).with_forge(Arc::new(forge));
    let result = executor.execute().await.unwrap();
    assert!(
        result.success,
        "task should succeed when forge is in context"
    );
}

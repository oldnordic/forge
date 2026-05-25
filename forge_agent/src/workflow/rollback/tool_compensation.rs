use crate::workflow::task::{
    CompensationAction, CompensationType, ExecutableCompensation, TaskContext, TaskError,
    TaskResult,
};
use std::fs;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct ToolCompensation {
    pub description: String,
    #[allow(clippy::type_complexity)]
    compensate: Arc<dyn Fn(&TaskContext) -> Result<TaskResult, TaskError> + Send + Sync>,
}

impl ToolCompensation {
    pub fn new<F>(description: impl Into<String>, compensate_fn: F) -> Self
    where
        F: Fn(&TaskContext) -> Result<TaskResult, TaskError> + Send + Sync + 'static,
    {
        Self {
            description: description.into(),
            compensate: Arc::new(compensate_fn),
        }
    }

    pub fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
        (self.compensate)(context)
    }

    pub fn file_compensation(file_path: impl Into<String>) -> Self {
        let path = file_path.into();
        Self::new(format!("Delete file: {}", path), move |_context| {
            if Path::new(&path).exists() {
                fs::remove_file(&path).map_err(|e| {
                    TaskError::ExecutionFailed(format!("Failed to delete file {}: {}", path, e))
                })?;
            }
            Ok(TaskResult::Success)
        })
    }

    pub fn process_compensation(pid: u32) -> Self {
        Self::new(format!("Terminate process: {}", pid), move |_context| {
            #[cfg(unix)]
            {
                use std::process::Command;
                let result = Command::new("kill")
                    .arg("-TERM")
                    .arg(pid.to_string())
                    .output();

                match result {
                    Ok(_) => Ok(TaskResult::Success),
                    Err(e) => Ok(TaskResult::Failed(format!(
                        "Failed to terminate process {}: {}",
                        pid, e
                    ))),
                }
            }

            #[cfg(not(unix))]
            {
                Ok(TaskResult::Failed(format!(
                    "Process termination not supported on this platform"
                )))
            }
        })
    }

    pub fn skip(description: impl Into<String>) -> Self {
        Self::new(description, |_context| Ok(TaskResult::Skipped))
    }

    pub fn retry(description: impl Into<String>) -> Self {
        Self::new(description, |_context| Ok(TaskResult::Skipped))
    }
}

impl From<CompensationAction> for ToolCompensation {
    fn from(action: CompensationAction) -> Self {
        match action.action_type {
            CompensationType::Skip => ToolCompensation::skip(action.description),
            CompensationType::Retry => ToolCompensation::retry(action.description),
            CompensationType::UndoFunction => ToolCompensation::skip(format!(
                "{} (no undo function available)",
                action.description
            )),
        }
    }
}

impl From<ExecutableCompensation> for ToolCompensation {
    fn from(exec: ExecutableCompensation) -> Self {
        let description = exec.action.description.clone();
        match exec.action.action_type {
            CompensationType::Skip => ToolCompensation::skip(description),
            CompensationType::Retry => ToolCompensation::retry(description),
            CompensationType::UndoFunction => {
                if let Some(undo_fn) = exec.into_undo_fn() {
                    ToolCompensation {
                        description,
                        compensate: undo_fn,
                    }
                } else {
                    ToolCompensation::skip(format!("{} (no undo function)", description))
                }
            }
        }
    }
}

use crate::workflow::task::{
    CompensationAction, ExecutableCompensation, TaskContext, TaskError, TaskId, TaskResult,
    WorkflowTask,
};
use std::path::PathBuf;

/// Task that edits a file (stub for Phase 11).
///
/// Demonstrates the Saga compensation pattern with undo functionality.
/// In Phase 11, this will be implemented with actual file editing.
pub struct FileEditTask {
    id: TaskId,
    name: String,
    file_path: PathBuf,
    original_content: String,
    new_content: String,
}

impl FileEditTask {
    /// Creates a new FileEditTask.
    ///
    /// # Arguments
    ///
    /// * `id` - Task identifier
    /// * `name` - Human-readable task name
    /// * `file_path` - Path to the file to edit
    /// * `original_content` - Original content (for rollback)
    /// * `new_content` - New content to write
    pub fn new(
        id: TaskId,
        name: String,
        file_path: PathBuf,
        original_content: String,
        new_content: String,
    ) -> Self {
        Self {
            id,
            name,
            file_path,
            original_content,
            new_content,
        }
    }

    /// Gets the file path.
    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    /// Gets the original content.
    pub fn original_content(&self) -> &str {
        &self.original_content
    }

    /// Gets the new content.
    pub fn new_content(&self) -> &str {
        &self.new_content
    }
}

#[async_trait::async_trait]
impl WorkflowTask for FileEditTask {
    async fn execute(&self, _context: &TaskContext) -> Result<TaskResult, TaskError> {
        if let Some(parent) = self.file_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(TaskError::Io)?;
        }
        tokio::fs::write(&self.file_path, &self.new_content)
            .await
            .map_err(TaskError::Io)?;

        let path = self.file_path.clone();
        let original = self.original_content.clone();
        let compensation = ExecutableCompensation::with_undo(
            format!("Restore original content of {}", path.display()),
            move |_ctx| {
                std::fs::write(&path, &original).map_err(TaskError::Io)?;
                Ok(TaskResult::Success)
            },
        );

        Ok(TaskResult::WithCompensation {
            result: Box::new(TaskResult::Success),
            compensation,
        })
    }

    fn id(&self) -> TaskId {
        self.id.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn compensation(&self) -> Option<CompensationAction> {
        Some(CompensationAction::undo(format!(
            "Restore original content of {}",
            self.file_path.display()
        )))
    }
}

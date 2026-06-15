use std::path::PathBuf;

use crate::error::Result;

pub(crate) enum UndoableOp {
    CreateFile {
        path: PathBuf,
    },
    WriteFile {
        path: PathBuf,
        previous: Option<String>,
    },
    CreateDirectory {
        path: PathBuf,
    },
}

pub struct PendingUndo {
    pub(crate) operation: UndoableOp,
}

pub enum UndoResult {
    Undone { operation: String },
    Empty,
}

impl super::EditModule {
    pub fn with_undo_capacity(mut self, capacity: usize) -> Self {
        self.undo_capacity = capacity;
        self
    }

    pub async fn undo(&self) -> Result<UndoResult> {
        let op = {
            let mut stack = self.undo_stack.lock();
            if stack.is_empty() {
                return Ok(UndoResult::Empty);
            }
            stack
                .pop()
                .expect("invariant: stack non-empty after is_empty check")
        };

        match op.operation {
            UndoableOp::CreateFile { path } => {
                let full = self.store.codebase_path.join(&path);
                if full.exists() {
                    tokio::fs::remove_file(&full).await?;
                }
                Ok(UndoResult::Undone {
                    operation: format!("create_file({})", path.display()),
                })
            }
            UndoableOp::WriteFile { path, previous } => {
                let full = self.store.codebase_path.join(&path);
                match previous {
                    Some(content) => {
                        tokio::fs::write(&full, content).await?;
                    }
                    None => {
                        if full.exists() {
                            tokio::fs::remove_file(&full).await?;
                        }
                    }
                }
                Ok(UndoResult::Undone {
                    operation: format!("write_file({})", path.display()),
                })
            }
            UndoableOp::CreateDirectory { path } => {
                let full = self.store.codebase_path.join(&path);
                if full.exists() {
                    tokio::fs::remove_dir(&full).await?;
                }
                Ok(UndoResult::Undone {
                    operation: format!("create_directory({})", path.display()),
                })
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.lock().is_empty()
    }

    pub fn undo_depth(&self) -> usize {
        self.undo_stack.lock().len()
    }

    pub fn clear_undo_stack(&self) {
        self.undo_stack.lock().clear();
    }

    pub(crate) fn push_undo(&self, op: UndoableOp) {
        let mut stack = self.undo_stack.lock();
        if stack.len() >= self.undo_capacity {
            stack.remove(0);
        }
        stack.push(PendingUndo { operation: op });
    }
}

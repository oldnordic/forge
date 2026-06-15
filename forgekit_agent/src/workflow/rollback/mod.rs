//! Rollback engine for workflow failure recovery using Saga compensation pattern.
//!
//! The rollback engine provides selective rollback of dependent tasks using
//! DAG backward traversal. When a task fails, only its dependents are rolled back,
//! leaving independent tasks in their completed state.
//!
//! # Saga Compensation Pattern
//!
//! Rollback uses the Saga pattern where each task optionally provides a
//! compensation action that undoes its side effects:
//! - `UndoFunction`: Executes a compensating transaction (e.g., delete created file)
//! - `Skip`: No compensation needed (read-only operations like queries)
//! - `Retry`: Recommends retry instead of compensation (transient failures)
//!
//! # Rollback Strategies
//!
//! - `AllDependent`: Roll back all tasks reachable from failed task (default)
//! - `FailedOnly`: Roll back only the failed task
//! - `Custom`: Use provided filter function for selective rollback

mod compensation_registry;
mod engine;
mod tool_compensation;

pub use compensation_registry::CompensationRegistry;
pub use engine::RollbackEngine;
pub use tool_compensation::ToolCompensation;

use crate::workflow::task::TaskId;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollbackStrategy {
    AllDependent,
    FailedOnly,
    Custom,
}

#[derive(Clone, Debug)]
pub struct RollbackReport {
    pub rolled_back_tasks: Vec<TaskId>,
    pub skipped_tasks: Vec<TaskId>,
    pub failed_compensations: Vec<(TaskId, String)>,
}

impl RollbackReport {
    pub(super) fn new() -> Self {
        Self {
            rolled_back_tasks: Vec::new(),
            skipped_tasks: Vec::new(),
            failed_compensations: Vec::new(),
        }
    }

    pub fn total_processed(&self) -> usize {
        self.rolled_back_tasks.len() + self.skipped_tasks.len() + self.failed_compensations.len()
    }
}

#[derive(Clone, Debug)]
pub struct CompensationReport {
    pub tasks_with_compensation: Vec<TaskId>,
    pub tasks_without_compensation: Vec<TaskId>,
    pub coverage_percentage: f64,
}

impl CompensationReport {
    pub(super) fn calculate(with_compensation: usize, total: usize) -> f64 {
        if total == 0 {
            1.0
        } else {
            with_compensation as f64 / total as f64
        }
    }
}

#[derive(Error, Debug)]
pub enum RollbackError {
    #[error("Failed to determine rollback set: {0}")]
    RollbackSetFailed(String),

    #[error("Task not found during rollback: {0}")]
    TaskNotFound(TaskId),

    #[error("Compensation failed for task {0}: {1}")]
    CompensationFailed(TaskId, String),

    #[error("Graph traversal error: {0}")]
    TraversalError(String),
}

#[cfg(test)]
mod tests;

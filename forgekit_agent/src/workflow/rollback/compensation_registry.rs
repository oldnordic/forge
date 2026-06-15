use super::{CompensationReport, ToolCompensation};
use crate::workflow::task::TaskId;
use std::collections::HashMap;

pub struct CompensationRegistry {
    compensations: HashMap<TaskId, ToolCompensation>,
}

impl CompensationRegistry {
    pub fn new() -> Self {
        Self {
            compensations: HashMap::new(),
        }
    }

    pub fn register(&mut self, task_id: TaskId, compensation: ToolCompensation) {
        self.compensations.insert(task_id, compensation);
    }

    pub fn get(&self, task_id: &TaskId) -> Option<&ToolCompensation> {
        self.compensations.get(task_id)
    }

    pub fn has_compensation(&self, task_id: &TaskId) -> bool {
        self.compensations.contains_key(task_id)
    }

    pub fn remove(&mut self, task_id: &TaskId) -> Option<ToolCompensation> {
        self.compensations.remove(task_id)
    }

    pub fn validate_coverage(&self, task_ids: &[TaskId]) -> CompensationReport {
        let mut with_compensation = Vec::new();
        let mut without_compensation = Vec::new();

        for task_id in task_ids {
            if self.has_compensation(task_id) {
                with_compensation.push(task_id.clone());
            } else {
                without_compensation.push(task_id.clone());
            }
        }

        let total = task_ids.len();
        let coverage = CompensationReport::calculate(with_compensation.len(), total);

        CompensationReport {
            tasks_with_compensation: with_compensation,
            tasks_without_compensation: without_compensation,
            coverage_percentage: coverage,
        }
    }

    pub fn register_file_creation(&mut self, task_id: TaskId, file_path: impl Into<String>) {
        self.register(task_id, ToolCompensation::file_compensation(file_path));
    }

    pub fn register_process_spawn(&mut self, task_id: TaskId, pid: u32) {
        self.register(task_id, ToolCompensation::process_compensation(pid));
    }

    pub fn len(&self) -> usize {
        self.compensations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.compensations.is_empty()
    }

    pub fn task_ids(&self) -> Vec<TaskId> {
        self.compensations.keys().cloned().collect()
    }
}

impl Default for CompensationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

use super::{
    CompensationRegistry, CompensationReport, RollbackError, RollbackReport, RollbackStrategy,
};
use crate::audit::AuditLog;
use crate::workflow::dag::Workflow;
use crate::workflow::task::{TaskContext, TaskId};
use chrono::Utc;
use sqlitegraph::typed_digraph::{Direction, NodeIndex};
use std::collections::{HashMap, HashSet, VecDeque};

pub struct RollbackEngine {
    _private: (),
}

impl RollbackEngine {
    pub fn new() -> Self {
        Self { _private: () }
    }

    pub fn find_rollback_set(
        &self,
        workflow: &Workflow,
        failed_task: &TaskId,
        strategy: RollbackStrategy,
    ) -> Result<Vec<TaskId>, RollbackError> {
        let failed_idx = *workflow
            .task_map
            .get(failed_task)
            .ok_or_else(|| RollbackError::TaskNotFound(failed_task.clone()))?;

        match strategy {
            RollbackStrategy::FailedOnly => Ok(vec![failed_task.clone()]),
            RollbackStrategy::AllDependent => {
                let predecessor_set = self.find_prerequisite_tasks(workflow, failed_idx)?;
                self.reverse_execution_order(workflow, predecessor_set)
            }
            RollbackStrategy::Custom => {
                let predecessor_set = self.find_prerequisite_tasks(workflow, failed_idx)?;
                self.reverse_execution_order(workflow, predecessor_set)
            }
        }
    }

    pub(crate) fn find_prerequisite_tasks(
        &self,
        workflow: &Workflow,
        failed_idx: NodeIndex,
    ) -> Result<HashSet<TaskId>, RollbackError> {
        let mut predecessor_set = HashSet::new();
        let mut visited = HashSet::new();
        let mut stack = VecDeque::new();

        stack.push_back(failed_idx);
        visited.insert(failed_idx);

        while let Some(current_idx) = stack.pop_front() {
            if let Some(node) = workflow.graph.node_weight(current_idx) {
                let task_id = node.id().clone();
                predecessor_set.insert(task_id);
            }

            for neighbor in workflow
                .graph
                .neighbors_directed(current_idx, Direction::Incoming)
            {
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor);
                    stack.push_back(neighbor);
                }
            }
        }

        Ok(predecessor_set)
    }

    pub(super) fn reverse_execution_order(
        &self,
        workflow: &Workflow,
        tasks: HashSet<TaskId>,
    ) -> Result<Vec<TaskId>, RollbackError> {
        let execution_order = workflow
            .execution_order()
            .map_err(|e| RollbackError::TraversalError(e.to_string()))?;

        let position_map: HashMap<TaskId, usize> = execution_order
            .iter()
            .enumerate()
            .map(|(pos, task_id)| (task_id.clone(), pos))
            .collect();

        let mut rollback_tasks: Vec<TaskId> = tasks.into_iter().collect();

        rollback_tasks.sort_by(|a, b| {
            let pos_a = position_map.get(a).copied().unwrap_or(0);
            let pos_b = position_map.get(b).copied().unwrap_or(0);
            pos_b.cmp(&pos_a)
        });

        Ok(rollback_tasks)
    }

    pub async fn execute_rollback(
        &self,
        workflow: &Workflow,
        tasks: Vec<TaskId>,
        workflow_id: &str,
        audit_log: &mut AuditLog,
        compensation_registry: &CompensationRegistry,
    ) -> Result<RollbackReport, RollbackError> {
        let mut report = RollbackReport::new();

        for task_id in &tasks {
            let node_idx = workflow
                .task_map
                .get(task_id)
                .ok_or_else(|| RollbackError::TaskNotFound(task_id.clone()))?;

            let _node = workflow
                .graph
                .node_weight(*node_idx)
                .expect("Node index should be valid");

            if let Some(compensation) = compensation_registry.get(task_id) {
                let context = TaskContext::new(workflow_id, task_id.clone());

                match compensation.execute(&context) {
                    Ok(_) => {
                        let _ = audit_log
                            .record(crate::audit::AuditEvent::WorkflowTaskRolledBack {
                                timestamp: Utc::now(),
                                workflow_id: workflow_id.to_string(),
                                task_id: task_id.to_string(),
                                compensation: compensation.description.clone(),
                            })
                            .await;

                        report.rolled_back_tasks.push(task_id.clone());
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        let _ = audit_log
                            .record(crate::audit::AuditEvent::WorkflowTaskRolledBack {
                                timestamp: Utc::now(),
                                workflow_id: workflow_id.to_string(),
                                task_id: task_id.to_string(),
                                compensation: format!("Failed: {}", error_msg),
                            })
                            .await;

                        report
                            .failed_compensations
                            .push((task_id.clone(), error_msg));
                    }
                }
            } else {
                report.skipped_tasks.push(task_id.clone());

                let _ = audit_log
                    .record(crate::audit::AuditEvent::WorkflowTaskRolledBack {
                        timestamp: Utc::now(),
                        workflow_id: workflow_id.to_string(),
                        task_id: task_id.to_string(),
                        compensation: "No compensation registered".to_string(),
                    })
                    .await;
            }
        }

        let _ = audit_log
            .record(crate::audit::AuditEvent::WorkflowRolledBack {
                timestamp: Utc::now(),
                workflow_id: workflow_id.to_string(),
                reason: "Task failure triggered rollback".to_string(),
                rolled_back_tasks: tasks.iter().map(|id| id.to_string()).collect(),
            })
            .await;

        Ok(report)
    }

    pub fn validate_compensation_coverage(
        &self,
        workflow: &Workflow,
        registry: &CompensationRegistry,
    ) -> CompensationReport {
        let task_ids = workflow.task_ids();
        registry.validate_coverage(&task_ids)
    }
}

impl Default for RollbackEngine {
    fn default() -> Self {
        Self::new()
    }
}

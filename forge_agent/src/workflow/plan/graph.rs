use chrono::{DateTime, Utc};

use super::{
    CustodyChain, CustodyNode, CustodyRun, CustodySection, CustodyTask, DeliverableData,
    DiscoveredKnowledge, GateResult, LogEntryData, PlanEdgeKind, PlanNodeKind, PlanSectionData,
    ReasoningStepData, SemgrepFinding, SubagentRunData, SubagentStatus, ToolCallData,
};

#[cfg(feature = "sqlite")]
pub struct PlanGraph {
    graph: sqlitegraph::SqliteGraph,
}

#[cfg(feature = "sqlite")]
impl PlanGraph {
    pub fn open(path: &std::path::Path) -> anyhow::Result<Self> {
        let graph = sqlitegraph::SqliteGraph::open(path)?;
        Ok(Self { graph })
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        let graph = sqlitegraph::SqliteGraph::open_in_memory()?;
        Ok(Self { graph })
    }

    pub fn add_requirement(&mut self, title: &str, description: &str) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::Requirement.as_str().to_string(),
            name: title.to_string(),
            file_path: None,
            data: serde_json::json!({ "description": description }),
        };
        let id = self.graph.insert_entity(&entity)?;
        Ok(id)
    }

    pub fn add_plan(&mut self, name: &str, requirement_ids: &[i64]) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::Plan.as_str().to_string(),
            name: name.to_string(),
            file_path: None,
            data: serde_json::json!({}),
        };
        let plan_id = self.graph.insert_entity(&entity)?;

        for req_id in requirement_ids {
            let edge = sqlitegraph::GraphEdge {
                id: 0,
                from_id: plan_id,
                to_id: *req_id,
                edge_type: PlanEdgeKind::HasRequirement.as_str().to_string(),
                data: serde_json::json!({}),
            };
            self.graph.insert_edge(&edge)?;
        }

        Ok(plan_id)
    }

    pub fn link_knowledge(
        &mut self,
        plan_id: i64,
        knowledge: &DiscoveredKnowledge,
    ) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::DiscoveredKnowledge.as_str().to_string(),
            name: knowledge.title.clone(),
            file_path: None,
            data: serde_json::to_value(knowledge)?,
        };
        let knowledge_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: plan_id,
            to_id: knowledge_id,
            edge_type: PlanEdgeKind::InformedBy.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(knowledge_id)
    }

    pub fn get_plan_knowledge(&self, plan_id: i64) -> anyhow::Result<Vec<DiscoveredKnowledge>> {
        let pattern = sqlitegraph::PatternTriple::new(PlanEdgeKind::InformedBy.as_str());
        let matches = self.graph.match_triples(&pattern)?;

        let mut results = Vec::new();
        for triple in matches {
            if triple.start_id == plan_id {
                if let Ok(entity) = self.graph.get_entity(triple.end_id) {
                    if entity.kind == PlanNodeKind::DiscoveredKnowledge.as_str() {
                        if let Ok(dk) = serde_json::from_value::<DiscoveredKnowledge>(entity.data) {
                            results.push(dk);
                        }
                    }
                }
            }
        }
        Ok(results)
    }

    pub fn add_gate_result(&mut self, task_id: i64, result: &GateResult) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::GateResult.as_str().to_string(),
            name: result.gate_name.clone(),
            file_path: None,
            data: serde_json::to_value(result)?,
        };
        let result_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: task_id,
            to_id: result_id,
            edge_type: PlanEdgeKind::ValidatedBy.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(result_id)
    }

    pub fn add_semgrep_finding(
        &mut self,
        gate_result_id: i64,
        finding: &SemgrepFinding,
    ) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::SemgrepFinding.as_str().to_string(),
            name: format!("{}:{}", finding.check_id, finding.file),
            file_path: Some(finding.file.clone()),
            data: serde_json::to_value(finding)?,
        };
        let finding_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: gate_result_id,
            to_id: finding_id,
            edge_type: PlanEdgeKind::FoundIn.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(finding_id)
    }

    pub fn approve(&mut self, plan_id: i64, approver: &str) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::Approval.as_str().to_string(),
            name: format!("approved_by_{}", approver),
            file_path: None,
            data: serde_json::json!({ "approver": approver }),
        };
        let approval_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: plan_id,
            to_id: approval_id,
            edge_type: PlanEdgeKind::Approved.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(approval_id)
    }

    pub fn reject(&mut self, plan_id: i64, reason: &str) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::Rejection.as_str().to_string(),
            name: "rejected".to_string(),
            file_path: None,
            data: serde_json::json!({ "reason": reason }),
        };
        let rejection_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: plan_id,
            to_id: rejection_id,
            edge_type: PlanEdgeKind::Rejected.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(rejection_id)
    }

    pub fn graph(&self) -> &sqlitegraph::SqliteGraph {
        &self.graph
    }

    pub fn count_outgoing(&self, from_id: i64, edge_type: &str) -> anyhow::Result<usize> {
        let pattern = sqlitegraph::PatternTriple::new(edge_type);
        let matches = self.graph.match_triples(&pattern)?;
        let count = matches.iter().filter(|t| t.start_id == from_id).count();
        Ok(count)
    }

    pub fn add_section(
        &mut self,
        plan_id: i64,
        requirement_id: i64,
        data: &PlanSectionData,
    ) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::PlanSection.as_str().to_string(),
            name: data.title.clone(),
            file_path: None,
            data: serde_json::to_value(data)?,
        };
        let section_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: section_id,
            to_id: requirement_id,
            edge_type: PlanEdgeKind::AddressesIn.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        let edge2 = sqlitegraph::GraphEdge {
            id: 0,
            from_id: plan_id,
            to_id: section_id,
            edge_type: PlanEdgeKind::DecomposesInto.as_str().to_string(),
            data: serde_json::json!({ "order": data.order }),
        };
        self.graph.insert_edge(&edge2)?;

        Ok(section_id)
    }

    pub fn sections_in_order(&self, plan_id: i64) -> anyhow::Result<Vec<(i64, PlanSectionData)>> {
        let pattern = sqlitegraph::PatternTriple::new(PlanEdgeKind::DecomposesInto.as_str());
        let matches = self.graph.match_triples(&pattern)?;

        let mut sections = Vec::new();
        for triple in matches {
            if triple.start_id == plan_id {
                if let Ok(entity) = self.graph.get_entity(triple.end_id) {
                    if entity.kind == PlanNodeKind::PlanSection.as_str() {
                        if let Ok(data) = serde_json::from_value::<PlanSectionData>(entity.data) {
                            sections.push((triple.end_id, data));
                        }
                    }
                }
            }
        }
        sections.sort_by_key(|(_, d)| d.order);
        Ok(sections)
    }

    pub fn begin_subagent_run(
        &mut self,
        task_id: i64,
        data: &SubagentRunData,
    ) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::SubagentRun.as_str().to_string(),
            name: format!("{}_{}", data.agent_name, data.run_id),
            file_path: None,
            data: serde_json::to_value(data)?,
        };
        let run_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: task_id,
            to_id: run_id,
            edge_type: PlanEdgeKind::ExecutedBy.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(run_id)
    }

    pub fn complete_subagent_run(
        &mut self,
        run_node_id: i64,
        status: SubagentStatus,
        summary: &str,
    ) -> anyhow::Result<()> {
        let mut entity = self.graph.get_entity(run_node_id)?;
        let mut data: SubagentRunData = serde_json::from_value(entity.data)?;
        data.status = status;
        data.completed_at = Some(Utc::now());
        data.output_summary = Some(summary.to_string());
        entity.data = serde_json::to_value(data)?;
        self.graph.update_entity(&entity)?;
        Ok(())
    }

    pub fn record_log(&mut self, run_node_id: i64, data: &LogEntryData) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::LogEntry.as_str().to_string(),
            name: format!("{:?}_{}", data.level, data.timestamp.timestamp()),
            file_path: None,
            data: serde_json::to_value(data)?,
        };
        let node_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: run_node_id,
            to_id: node_id,
            edge_type: PlanEdgeKind::Logged.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(node_id)
    }

    pub fn record_tool_call(
        &mut self,
        run_node_id: i64,
        data: &ToolCallData,
    ) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::ToolCall.as_str().to_string(),
            name: data.tool.clone(),
            file_path: None,
            data: serde_json::to_value(data)?,
        };
        let node_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: run_node_id,
            to_id: node_id,
            edge_type: PlanEdgeKind::Called.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(node_id)
    }

    pub fn record_reasoning(
        &mut self,
        run_node_id: i64,
        data: &ReasoningStepData,
    ) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::ReasoningStep.as_str().to_string(),
            name: data.decision.clone(),
            file_path: None,
            data: serde_json::to_value(data)?,
        };
        let node_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: run_node_id,
            to_id: node_id,
            edge_type: PlanEdgeKind::Reasoned.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(node_id)
    }

    pub fn record_deliverable(
        &mut self,
        run_node_id: i64,
        data: &DeliverableData,
    ) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::Deliverable.as_str().to_string(),
            name: data.file_path.clone(),
            file_path: Some(data.file_path.clone()),
            data: serde_json::to_value(data)?,
        };
        let node_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: run_node_id,
            to_id: node_id,
            edge_type: PlanEdgeKind::Produced.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(node_id)
    }

    pub(crate) fn children(
        &self,
        parent_id: i64,
        edge_type: &str,
        node_kind: &str,
    ) -> anyhow::Result<Vec<CustodyNode>> {
        let pattern = sqlitegraph::PatternTriple::new(edge_type);
        let matches = self.graph.match_triples(&pattern)?;
        let mut results = Vec::new();
        for triple in matches {
            if triple.start_id == parent_id {
                if let Ok(entity) = self.graph.get_entity(triple.end_id) {
                    if entity.kind == node_kind {
                        let ts = entity
                            .data
                            .get("timestamp")
                            .and_then(|v| serde_json::from_value::<DateTime<Utc>>(v.clone()).ok());
                        let ts = ts.or_else(|| {
                            entity.data.get("started_at").and_then(|v| {
                                serde_json::from_value::<DateTime<Utc>>(v.clone()).ok()
                            })
                        });
                        results.push(CustodyNode {
                            id: triple.end_id,
                            kind: entity.kind,
                            data: entity.data,
                            timestamp: ts,
                        });
                    }
                }
            }
        }
        Ok(results)
    }

    fn parent(
        &self,
        child_id: i64,
        edge_type: &str,
        node_kind: &str,
    ) -> anyhow::Result<Option<CustodyNode>> {
        let pattern = sqlitegraph::PatternTriple::new(edge_type);
        let matches = self.graph.match_triples(&pattern)?;
        for triple in matches {
            if triple.end_id == child_id {
                if let Ok(entity) = self.graph.get_entity(triple.start_id) {
                    if entity.kind == node_kind {
                        let ts = entity
                            .data
                            .get("timestamp")
                            .and_then(|v| serde_json::from_value::<DateTime<Utc>>(v.clone()).ok());
                        return Ok(Some(CustodyNode {
                            id: triple.start_id,
                            kind: entity.kind,
                            data: entity.data,
                            timestamp: ts,
                        }));
                    }
                }
            }
        }
        Ok(None)
    }

    pub fn trace_backward(&self, node_id: i64) -> anyhow::Result<Vec<CustodyNode>> {
        let mut path = Vec::new();
        let mut current = node_id;

        loop {
            let parent_node = self
                .parent(current, PlanEdgeKind::ExecutedBy.as_str(), "Task")
                .ok()
                .flatten()
                .or_else(|| {
                    self.parent(
                        current,
                        PlanEdgeKind::DecomposesInto.as_str(),
                        "PlanSection",
                    )
                    .ok()
                    .flatten()
                })
                .or_else(|| {
                    self.parent(current, PlanEdgeKind::HasRequirement.as_str(), "Plan")
                        .ok()
                        .flatten()
                })
                .or_else(|| {
                    self.parent(current, PlanEdgeKind::AddressesIn.as_str(), "Requirement")
                        .ok()
                        .flatten()
                })
                .or_else(|| {
                    self.parent(current, PlanEdgeKind::Logged.as_str(), "SubagentRun")
                        .ok()
                        .flatten()
                })
                .or_else(|| {
                    self.parent(current, PlanEdgeKind::Called.as_str(), "SubagentRun")
                        .ok()
                        .flatten()
                })
                .or_else(|| {
                    self.parent(current, PlanEdgeKind::Reasoned.as_str(), "SubagentRun")
                        .ok()
                        .flatten()
                })
                .or_else(|| {
                    self.parent(current, PlanEdgeKind::Produced.as_str(), "SubagentRun")
                        .ok()
                        .flatten()
                });

            if let Some(p) = parent_node {
                current = p.id;
                path.push(p);
            } else {
                break;
            }
        }

        path.reverse();
        Ok(path)
    }

    pub fn timeline(&self, root_id: i64) -> anyhow::Result<Vec<CustodyNode>> {
        let edge_types: &[&str] = &[
            PlanEdgeKind::HasRequirement.as_str(),
            PlanEdgeKind::DecomposesInto.as_str(),
            PlanEdgeKind::Implements.as_str(),
            PlanEdgeKind::DependsOn.as_str(),
            PlanEdgeKind::ValidatedBy.as_str(),
            PlanEdgeKind::AssignedTo.as_str(),
            PlanEdgeKind::Approved.as_str(),
            PlanEdgeKind::Rejected.as_str(),
            PlanEdgeKind::FoundIn.as_str(),
            PlanEdgeKind::DetectedBy.as_str(),
            PlanEdgeKind::Checks.as_str(),
            PlanEdgeKind::InformedBy.as_str(),
            PlanEdgeKind::RelatedTo.as_str(),
            PlanEdgeKind::ExecutedBy.as_str(),
            PlanEdgeKind::Logged.as_str(),
            PlanEdgeKind::Called.as_str(),
            PlanEdgeKind::Reasoned.as_str(),
            PlanEdgeKind::Produced.as_str(),
            PlanEdgeKind::AddressesIn.as_str(),
        ];

        let mut visited = std::collections::HashSet::new();
        let mut queue = vec![root_id];
        let mut results = Vec::new();

        while let Some(id) = queue.pop() {
            if !visited.insert(id) {
                continue;
            }

            if let Ok(entity) = self.graph.get_entity(id) {
                let ts = entity
                    .data
                    .get("timestamp")
                    .and_then(|v| serde_json::from_value::<DateTime<Utc>>(v.clone()).ok());
                let ts = ts.or_else(|| {
                    entity
                        .data
                        .get("started_at")
                        .and_then(|v| serde_json::from_value::<DateTime<Utc>>(v.clone()).ok())
                });
                results.push(CustodyNode {
                    id,
                    kind: entity.kind.clone(),
                    data: entity.data,
                    timestamp: ts,
                });

                for et in edge_types {
                    let pattern = sqlitegraph::PatternTriple::new(*et);
                    let matches = self.graph.match_triples(&pattern)?;
                    for triple in matches {
                        if triple.start_id == id && !visited.contains(&triple.end_id) {
                            queue.push(triple.end_id);
                        }
                    }
                }
            }
        }

        results.sort_by(|a, b| match (a.timestamp, b.timestamp) {
            (Some(ta), Some(tb)) => ta.cmp(&tb),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
        Ok(results)
    }

    pub fn find_gaps(&self, plan_id: i64) -> anyhow::Result<Vec<i64>> {
        let pattern = sqlitegraph::PatternTriple::new(PlanEdgeKind::DecomposesInto.as_str());
        let all = self.graph.match_triples(&pattern)?;
        let mut gaps = Vec::new();

        for triple in &all {
            if triple.start_id == plan_id {
                let task_pattern =
                    sqlitegraph::PatternTriple::new(PlanEdgeKind::DecomposesInto.as_str());
                let task_matches = self.graph.match_triples(&task_pattern)?;
                for task_triple in task_matches {
                    if task_triple.start_id == triple.end_id {
                        if let Ok(entity) = self.graph.get_entity(task_triple.end_id) {
                            if entity.kind == PlanNodeKind::Task.as_str() {
                                let exec = self.count_outgoing(
                                    task_triple.end_id,
                                    PlanEdgeKind::ExecutedBy.as_str(),
                                )?;
                                if exec == 0 {
                                    gaps.push(task_triple.end_id);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(gaps)
    }

    pub fn trace_forward(&self, requirement_id: i64) -> anyhow::Result<CustodyChain> {
        let mut chain = CustodyChain {
            requirement_id,
            sections: Vec::new(),
        };

        let plan_pattern = sqlitegraph::PatternTriple::new(PlanEdgeKind::HasRequirement.as_str());
        let plans = self.graph.match_triples(&plan_pattern)?;
        let plan_ids: Vec<i64> = plans
            .iter()
            .filter(|t| t.end_id == requirement_id)
            .map(|t| t.start_id)
            .collect();

        for plan_id in plan_ids {
            let section_nodes = self.children(
                plan_id,
                PlanEdgeKind::DecomposesInto.as_str(),
                PlanNodeKind::PlanSection.as_str(),
            )?;
            for section_node in section_nodes {
                let section_data: PlanSectionData = serde_json::from_value(section_node.data)?;

                let mut section = CustodySection {
                    section_id: section_node.id,
                    section: section_data,
                    tasks: Vec::new(),
                };

                let task_nodes = self.children(
                    section_node.id,
                    PlanEdgeKind::DecomposesInto.as_str(),
                    PlanNodeKind::Task.as_str(),
                )?;
                for task_node in task_nodes {
                    let mut task = CustodyTask {
                        task_id: task_node.id,
                        runs: Vec::new(),
                    };

                    let run_nodes = self.children(
                        task_node.id,
                        PlanEdgeKind::ExecutedBy.as_str(),
                        PlanNodeKind::SubagentRun.as_str(),
                    )?;
                    for run_node in run_nodes {
                        let run_data: SubagentRunData = serde_json::from_value(run_node.data)?;

                        let logs: Vec<LogEntryData> = self
                            .children(
                                run_node.id,
                                PlanEdgeKind::Logged.as_str(),
                                PlanNodeKind::LogEntry.as_str(),
                            )?
                            .into_iter()
                            .filter_map(|n| serde_json::from_value(n.data).ok())
                            .collect();

                        let calls: Vec<ToolCallData> = self
                            .children(
                                run_node.id,
                                PlanEdgeKind::Called.as_str(),
                                PlanNodeKind::ToolCall.as_str(),
                            )?
                            .into_iter()
                            .filter_map(|n| serde_json::from_value(n.data).ok())
                            .collect();

                        let reasoning: Vec<ReasoningStepData> = self
                            .children(
                                run_node.id,
                                PlanEdgeKind::Reasoned.as_str(),
                                PlanNodeKind::ReasoningStep.as_str(),
                            )?
                            .into_iter()
                            .filter_map(|n| serde_json::from_value(n.data).ok())
                            .collect();

                        let deliverables: Vec<DeliverableData> = self
                            .children(
                                run_node.id,
                                PlanEdgeKind::Produced.as_str(),
                                PlanNodeKind::Deliverable.as_str(),
                            )?
                            .into_iter()
                            .filter_map(|n| serde_json::from_value(n.data).ok())
                            .collect();

                        task.runs.push(CustodyRun {
                            run_id: run_node.id,
                            run: run_data,
                            logs,
                            calls,
                            reasoning,
                            deliverables,
                        });
                    }

                    section.tasks.push(task);
                }

                chain.sections.push(section);
            }
        }

        chain.sections.sort_by_key(|s| s.section.order);
        Ok(chain)
    }
}

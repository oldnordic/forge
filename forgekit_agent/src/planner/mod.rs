//! Planning engine - Execution plan generation from observations.
//!
//! This module implements the planning phase of the agent loop, generating
//! execution steps from observations and constraints.

mod parsing;
mod types;

#[cfg(test)]
mod tests;

pub use types::*;

use parsing::{detect_intent, parse_llm_steps, should_precede, FileRegion};

use crate::Result;
use std::sync::Arc;

/// Planner for generating execution plans.
///
/// The Planner creates ordered steps from observations, ensuring
/// dependencies are satisfied and conflicts are detected.
/// When an LLM provider is available, it uses it for intelligent
/// step generation. Otherwise falls back to regex intent detection.
#[derive(Clone)]
pub struct Planner {
    llm: Option<Arc<dyn crate::llm::LlmProvider>>,
    context_prefix: Option<String>,
    /// Steps already attempted this session — used by `fix_once` to avoid repeats.
    attempted: Vec<PlanStep>,
    /// Optional generator for enriching `Create` step content with real code.
    generator: Option<Arc<crate::generate::Generator>>,
    /// Knowledge gap hints from KnowledgeGapAnalyzer, appended to LLM prompts.
    gap_hints: Vec<String>,
}

impl Default for Planner {
    fn default() -> Self {
        Self::new()
    }
}

impl Planner {
    /// Creates a new planner.
    pub fn new() -> Self {
        Self {
            llm: None,
            context_prefix: None,
            attempted: Vec::new(),
            generator: None,
            gap_hints: Vec::new(),
        }
    }

    /// Sets the LLM provider for intelligent step generation.
    pub fn with_llm(mut self, provider: Arc<dyn crate::llm::LlmProvider>) -> Self {
        self.llm = Some(provider);
        self
    }

    /// Sets the codebase context prefix injected into LLM prompts.
    pub fn with_context(mut self, ctx: &crate::context::AgentContext) -> Self {
        self.context_prefix = Some(ctx.context_prefix());
        self
    }

    /// Sets the code generator used to enrich `Create` step content.
    pub fn with_generator(mut self, gen: Arc<crate::generate::Generator>) -> Self {
        self.generator = Some(gen);
        self
    }

    /// Appends knowledge gap hints to LLM prompts so the planner is aware of
    /// missing information flagged by `KnowledgeGapAnalyzer`.
    pub fn with_gap_hints(mut self, hints: &[String]) -> Self {
        self.gap_hints = hints.to_vec();
        self
    }

    /// Generates execution steps from an observation.
    ///
    /// When an LLM is configured, sends the observation to the LLM for
    /// intelligent step generation as JSON. Falls back to regex intent
    /// detection when no LLM is available or LLM response is unparseable.
    pub async fn generate_steps(
        &self,
        observation: &super::observe::Observation,
    ) -> Result<Vec<PlanStep>> {
        // Try LLM first if available
        let mut steps = if let Some(ref llm) = self.llm {
            match self
                .generate_steps_with_llm(llm.as_ref(), observation)
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("LLM step generation failed, falling back to regex: {e}");
                    self.generate_steps_regex(observation).await?
                }
            }
        } else {
            self.generate_steps_regex(observation).await?
        };

        // Enrich Create steps with real generated code when a Generator is configured
        if let Some(ref gen) = self.generator {
            for step in &mut steps {
                if let PlanOperation::Create {
                    ref mut content, ..
                } = step.operation
                {
                    match gen.generate(&step.description).await {
                        Ok(gc) => *content = gc.content,
                        Err(e) => {
                            tracing::warn!(
                                "Generator failed for Create step, keeping original content: {e}"
                            );
                        }
                    }
                }
            }
        }

        Ok(steps)
    }

    /// Generate fix steps using error context from a failed verification.
    /// Falls back to empty plan if LLM unavailable or response unparseable.
    pub async fn generate_fix_steps(
        &self,
        observation: &super::observe::Observation,
        errors: &[String],
        previous_steps: &[PlanStep],
    ) -> Result<Vec<PlanStep>> {
        let Some(ref llm) = self.llm else {
            return Ok(Vec::new());
        };

        let error_text = errors.join("\n");
        let symbol_list: Vec<String> = observation
            .symbols
            .iter()
            .map(|s| format!("{} (id:{})", s.name, s.id.0))
            .collect();

        let prev_text = if previous_steps.is_empty() {
            String::new()
        } else {
            let ops: Vec<String> = previous_steps
                .iter()
                .map(|s| format!("{:?}", s.operation))
                .collect();
            format!("\nAlready tried (do not repeat): {}", ops.join("; "))
        };

        let prefix = self
            .context_prefix
            .as_deref()
            .map(|p| format!("{}\n", p))
            .unwrap_or_default();
        let gap_text = if self.gap_hints.is_empty() {
            String::new()
        } else {
            format!("Knowledge gaps:\n{}\n", self.gap_hints.join("\n"))
        };
        let prompt = format!(
            "{}{}Query: {}\nSymbols: [{}]\nCompilation/verification errors:\n{}{}",
            prefix,
            gap_text,
            observation.query,
            symbol_list.join(", "),
            error_text,
            prev_text
        );

        let system = "You are a Rust fix planner. Given a code query, relevant symbols, \
and compilation/verification errors, generate fix steps as a JSON array.\n\n\
Available operations:\n\
- {\"operation\":\"inspect\",\"symbol_name\":\"...\",\"symbol_id\":N}\n\
- {\"operation\":\"rename\",\"old\":\"...\",\"new\":\"...\",\"file\":\"...\"}\n\
- {\"operation\":\"delete\",\"name\":\"...\",\"file\":\"...\"}\n\
- {\"operation\":\"create\",\"path\":\"...\",\"content\":\"...\"}\n\
- {\"operation\":\"modify\",\"file\":\"...\",\"start\":N,\"end\":N,\"replacement\":\"...\"}\n\n\
Output ONLY a JSON array. No explanation.";

        match llm.complete(&prompt, Some(system)).await {
            Ok(resp) => {
                let steps = parse_llm_steps(&resp).unwrap_or_default();
                let deduped: Vec<PlanStep> = steps
                    .into_iter()
                    .filter(|s| !previous_steps.iter().any(|p| p.operation == s.operation))
                    .collect();
                Ok(deduped)
            }
            Err(e) => {
                tracing::warn!("LLM fix generation failed: {e}");
                Ok(Vec::new())
            }
        }
    }

    /// Generate fix steps for one retry attempt, using internal history to deduplicate.
    ///
    /// Unlike `generate_fix_steps`, the caller does not manage the attempt history —
    /// the Planner tracks it internally and filters duplicates automatically.
    pub async fn fix_once(
        &mut self,
        observation: &super::observe::Observation,
        errors: &[String],
    ) -> Result<Vec<PlanStep>> {
        let steps = self
            .generate_fix_steps(observation, errors, &self.attempted.clone())
            .await?;
        self.attempted.extend(steps.clone());
        Ok(steps)
    }

    /// Generate steps using LLM. Returns Err if LLM call fails or response
    /// is unparseable.
    async fn generate_steps_with_llm(
        &self,
        llm: &dyn crate::llm::LlmProvider,
        observation: &super::observe::Observation,
    ) -> Result<Vec<PlanStep>> {
        let summary_text = observation
            .summary
            .as_deref()
            .unwrap_or("No summary available");

        let symbol_list: Vec<String> = observation
            .symbols
            .iter()
            .map(|s| format!("{} (id:{})", s.name, s.id.0))
            .collect();

        let prefix = self
            .context_prefix
            .as_deref()
            .map(|p| format!("{}\n", p))
            .unwrap_or_default();
        let gap_text = if self.gap_hints.is_empty() {
            String::new()
        } else {
            format!("Knowledge gaps:\n{}\n", self.gap_hints.join("\n"))
        };
        let prompt = format!(
            "{}{}Query: {}\nSummary: {}\nSymbols: [{}]",
            prefix,
            gap_text,
            observation.query,
            summary_text,
            symbol_list.join(", ")
        );

        let system = "You are a code operation planner. Given a code query, generate execution steps as a JSON array.\n\nAvailable operations:\n\
        - {\"operation\":\"inspect\",\"symbol_name\":\"...\",\"symbol_id\":N}\n\
        - {\"operation\":\"rename\",\"old\":\"...\",\"new\":\"...\",\"file\":\"...\"}\n\
        - {\"operation\":\"delete\",\"name\":\"...\",\"file\":\"...\"}\n\
        - {\"operation\":\"create\",\"path\":\"...\",\"content\":\"...\"}\n\
        - {\"operation\":\"modify\",\"file\":\"...\",\"start\":N,\"end\":N,\"replacement\":\"...\"}\n\n\
        Output ONLY a JSON array. No explanation.";

        let response = llm.complete(&prompt, Some(system)).await.map_err(|e| {
            crate::AgentError::PlanningFailed(format!("LLM step generation failed: {}", e))
        })?;

        parse_llm_steps(&response)
    }

    /// Regex-based step generation (original logic).
    async fn generate_steps_regex(
        &self,
        observation: &super::observe::Observation,
    ) -> Result<Vec<PlanStep>> {
        let query_lower = observation.query.to_lowercase();
        let mut steps = Vec::new();

        // Detect intent from query
        let intent = detect_intent(&query_lower);

        for symbol in &observation.symbols {
            let file = symbol.location.file_path.to_str().map(|s| s.to_string());

            match &intent {
                PlanIntent::Rename { new_name } => {
                    steps.push(PlanStep {
                        description: format!("Rename {} to {}", symbol.name, new_name),
                        operation: PlanOperation::Rename {
                            old: symbol.name.clone(),
                            new: new_name.clone(),
                            file: file.clone(),
                        },
                    });
                }
                PlanIntent::Delete => {
                    steps.push(PlanStep {
                        description: format!("Delete {}", symbol.name),
                        operation: PlanOperation::Delete {
                            name: symbol.name.to_string(),
                            file: file.clone(),
                        },
                    });
                }
                PlanIntent::Create { content } => {
                    let file_path = symbol.location.file_path.to_string_lossy().to_string();
                    steps.push(PlanStep {
                        description: format!("Create {} in {}", symbol.name, file_path),
                        operation: PlanOperation::Create {
                            path: file_path,
                            content: content.clone(),
                        },
                    });
                }
                PlanIntent::Inspect => {
                    steps.push(PlanStep {
                        description: format!(
                            "Inspect {} ({:?} at {}:{})",
                            symbol.name,
                            symbol.kind,
                            symbol.location.file_path.display(),
                            symbol.location.line_number,
                        ),
                        operation: PlanOperation::Inspect {
                            symbol_id: symbol.id,
                            symbol_name: symbol.name.to_string(),
                        },
                    });
                }
            }
        }

        Ok(steps)
    }

    /// Estimates impact of a plan.
    ///
    /// # Arguments
    ///
    /// * `steps` - The planned steps
    pub async fn estimate_impact(&self, steps: &[PlanStep]) -> Result<ImpactEstimate> {
        let mut affected_files = std::collections::HashSet::new();

        // Collect all affected files
        for step in steps {
            match &step.operation {
                PlanOperation::Rename { file, .. } => {
                    if let Some(f) = file {
                        affected_files.insert(f.clone());
                    }
                }
                PlanOperation::Delete { file, .. } => {
                    if let Some(f) = file {
                        affected_files.insert(f.clone());
                    }
                }
                PlanOperation::Create { path, .. } => {
                    affected_files.insert(path.clone());
                }
                PlanOperation::Inspect { .. } => {
                    // Inspect doesn't modify files
                }
                PlanOperation::Modify { file, .. } => {
                    affected_files.insert(file.clone());
                }
            }
        }

        Ok(ImpactEstimate {
            affected_files: affected_files.into_iter().collect(),
            complexity: steps.len(),
        })
    }

    /// Detects conflicts between steps.
    ///
    /// # Arguments
    ///
    /// * `steps` - The planned steps
    pub fn detect_conflicts(&self, steps: &[PlanStep]) -> Result<Vec<Conflict>> {
        let mut conflicts = Vec::new();
        let mut file_regions: std::collections::HashMap<String, Vec<(usize, usize, usize)>> =
            std::collections::HashMap::new();

        // Track regions in each file
        for (idx, step) in steps.iter().enumerate() {
            if let Some(region) = self.get_step_region(step) {
                file_regions.entry(region.file.clone()).or_default().push((
                    idx,
                    region.start,
                    region.end,
                ));
            }
        }

        // Check for overlaps
        for (file, regions) in &file_regions {
            for i in 0..regions.len() {
                for j in (i + 1)..regions.len() {
                    let (_, start1, end1) = regions[i];
                    let (_, start2, end2) = regions[j];

                    // Two intervals [start1,end1) and [start2,end2) overlap
                    // when start1 < end2 && start2 < end1
                    if start1 < end2 && start2 < end1 {
                        conflicts.push(Conflict {
                            file: file.clone(),
                            reason: ConflictReason::OverlappingRegion {
                                start: start1.min(start2),
                                end: end1.max(end2),
                            },
                        });
                    }
                }
            }
        }

        Ok(conflicts)
    }

    /// Orders steps using topological sort based on dependencies.
    ///
    /// Rules:
    /// - Inspect of symbol X must come before Rename/Delete/Modify of X
    /// - Rename of symbol X must come before Delete of X
    /// - Create of file F must come before Modify of file F
    pub fn order_steps(&self, steps: &mut [PlanStep]) -> Result<()> {
        if steps.len() <= 1 {
            return Ok(());
        }

        let n = steps.len();
        // Build adjacency: edge i->j means step i must happen before step j
        let mut edges: Vec<(usize, usize)> = Vec::new();

        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }
                if should_precede(&steps[i].operation, &steps[j].operation) {
                    edges.push((i, j));
                }
            }
        }

        // Kahn's algorithm for topological sort
        let mut in_degree = vec![0usize; n];
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for &(from, to) in &edges {
            adj[from].push(to);
            in_degree[to] += 1;
        }

        let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        for (i, &deg) in in_degree.iter().enumerate() {
            if deg == 0 {
                queue.push_back(i);
            }
        }

        let mut sorted_indices = Vec::with_capacity(n);
        while let Some(node) = queue.pop_front() {
            sorted_indices.push(node);
            for &neighbor in &adj[node] {
                in_degree[neighbor] -= 1;
                if in_degree[neighbor] == 0 {
                    queue.push_back(neighbor);
                }
            }
        }

        if sorted_indices.len() != n {
            // Cycle detected — fall back to original order
            return Ok(());
        }

        // Reorder steps in-place
        let original: Vec<PlanStep> = steps.to_vec();
        for (target_pos, &source_idx) in sorted_indices.iter().enumerate() {
            steps[target_pos] = original[source_idx].clone();
        }

        Ok(())
    }

    /// Generates rollback plan.
    ///
    /// # Arguments
    ///
    /// * `steps` - The planned steps
    pub fn generate_rollback(&self, steps: &[PlanStep]) -> Vec<RollbackStep> {
        steps
            .iter()
            .rev()
            .map(|step| RollbackStep {
                description: format!("Rollback: {}", step.description),
                operation: match &step.operation {
                    PlanOperation::Rename { old, .. } => RollbackOperation::Rename {
                        new_name: old.clone(),
                    },
                    PlanOperation::Delete { name, .. } => {
                        RollbackOperation::Restore { name: name.clone() }
                    }
                    PlanOperation::Create { path, .. } => {
                        RollbackOperation::Delete { path: path.clone() }
                    }
                    PlanOperation::Inspect { .. } => RollbackOperation::None,
                    PlanOperation::Modify { file, .. } => {
                        RollbackOperation::Restore { name: file.clone() }
                    }
                },
            })
            .collect()
    }

    /// Gets the file region affected by a step.
    fn get_step_region(&self, step: &PlanStep) -> Option<FileRegion> {
        match &step.operation {
            PlanOperation::Rename { file, .. } | PlanOperation::Delete { file, .. } => {
                file.as_ref().map(|f| FileRegion {
                    file: f.clone(),
                    start: 0,
                    end: usize::MAX,
                })
            }
            PlanOperation::Create { path, .. } => Some(FileRegion {
                file: path.clone(),
                start: 0,
                end: usize::MAX,
            }),
            PlanOperation::Inspect { .. } => None,
            PlanOperation::Modify {
                file, start, end, ..
            } => Some(FileRegion {
                file: file.clone(),
                start: *start,
                end: *end,
            }),
        }
    }
}

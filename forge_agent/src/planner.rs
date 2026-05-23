//! Planning engine - Execution plan generation from observations.
//!
//! This module implements the planning phase of the agent loop, generating
//! execution steps from observations and constraints.

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
        let prompt = format!(
            "{}Query: {}\nSymbols: [{}]\nCompilation/verification errors:\n{}{}",
            prefix,
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
        let prompt = format!(
            "{}Query: {}\nSummary: {}\nSymbols: [{}]",
            prefix,
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
                    let (idx1, start1, end1) = regions[i];
                    let (idx2, start2, end2) = regions[j];

                    // Two intervals [start1,end1) and [start2,end2) overlap
                    // when start1 < end2 && start2 < end1
                    if start1 < end2 && start2 < end1 {
                        conflicts.push(Conflict {
                            step_indices: vec![idx1, idx2],
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

/// A step in the execution plan.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanStep {
    /// Step description
    pub description: String,
    /// Operation to perform
    pub operation: PlanOperation,
}

/// Operation to perform in a plan step.
#[derive(Clone, Debug, PartialEq)]
pub enum PlanOperation {
    /// Rename a symbol
    Rename {
        old: String,
        new: String,
        file: Option<String>,
    },
    /// Delete a symbol
    Delete { name: String, file: Option<String> },
    /// Create new code
    Create { path: String, content: String },
    /// Inspect a symbol (read-only)
    Inspect {
        symbol_id: forge_core::types::SymbolId,
        symbol_name: String,
    },
    /// Modify existing code
    Modify {
        file: String,
        start: usize,
        end: usize,
        replacement: String,
    },
}

/// Estimated impact of a plan.
#[derive(Clone, Debug)]
pub struct ImpactEstimate {
    /// Files to be modified
    pub affected_files: Vec<String>,
    /// Estimated complexity
    pub complexity: usize,
}

/// A conflict detected between steps.
#[derive(Clone, Debug)]
pub struct Conflict {
    /// Indices of conflicting steps
    pub step_indices: Vec<usize>,
    /// File where conflict occurs
    pub file: String,
    /// Reason for conflict
    pub reason: ConflictReason,
}

/// Reason for a conflict.
#[derive(Clone, Debug)]
pub enum ConflictReason {
    /// Overlapping regions in the same file
    OverlappingRegion { start: usize, end: usize },
    /// Circular dependency
    CircularDependency,
    /// Missing dependency
    MissingDependency,
}

/// A rollback step.
#[derive(Clone, Debug)]
pub struct RollbackStep {
    /// Step description
    pub description: String,
    /// Rollback operation
    pub operation: RollbackOperation,
}

/// Rollback operation.
#[derive(Clone, Debug)]
pub enum RollbackOperation {
    /// Rollback by renaming back
    Rename { new_name: String },
    /// Rollback by restoring deleted content
    Restore { name: String },
    /// Rollback by deleting created content
    Delete { path: String },
    /// No rollback needed
    None,
}

/// Intent detected from an observation query.
#[derive(Clone, Debug)]
enum PlanIntent {
    /// Rename symbols to a new name
    Rename { new_name: String },
    /// Delete symbols
    Delete,
    /// Create new code
    Create { content: String },
    /// Inspect symbols (default)
    Inspect,
}

/// Parse LLM JSON response into plan steps.
fn parse_llm_steps(response: &str) -> Result<Vec<PlanStep>> {
    let trimmed = response.trim();

    // Strip markdown code fences if present
    let json_str = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .strip_suffix("```")
        .unwrap_or(trimmed)
        .trim();

    let items: Vec<serde_json::Value> = serde_json::from_str(json_str).map_err(|_| {
        crate::AgentError::PlanningFailed("Failed to parse LLM response as JSON array".to_string())
    })?;

    let mut steps = Vec::new();
    for item in &items {
        match json_value_to_step(item) {
            Some(step) => steps.push(step),
            None => {
                tracing::warn!(
                    "LLM plan: skipping unparseable step: {}",
                    item.to_string().chars().take(200).collect::<String>()
                );
            }
        }
    }

    let skipped = items.len() - steps.len();
    if skipped > 0 {
        tracing::warn!(
            "LLM plan: {skipped} of {} steps failed to parse",
            items.len()
        );
    }

    Ok(steps)
}

/// Convert a JSON object to a PlanStep.
fn json_value_to_step(val: &serde_json::Value) -> Option<PlanStep> {
    let obj = val.as_object()?;
    let op = obj.get("operation")?.as_str()?;

    let operation = match op {
        "inspect" => {
            let name = obj.get("symbol_name")?.as_str()?.to_string();
            let id = obj.get("symbol_id").and_then(|v| v.as_u64())?;
            PlanOperation::Inspect {
                symbol_id: forge_core::types::SymbolId(id as i64),
                symbol_name: name,
            }
        }
        "rename" => PlanOperation::Rename {
            old: obj.get("old")?.as_str()?.to_string(),
            new: obj.get("new")?.as_str()?.to_string(),
            file: obj.get("file").and_then(|v| v.as_str()).map(String::from),
        },
        "delete" => PlanOperation::Delete {
            name: obj.get("name")?.as_str()?.to_string(),
            file: obj.get("file").and_then(|v| v.as_str()).map(String::from),
        },
        "create" => PlanOperation::Create {
            path: obj.get("path")?.as_str()?.to_string(),
            content: obj.get("content")?.as_str()?.to_string(),
        },
        "modify" => PlanOperation::Modify {
            file: obj.get("file")?.as_str()?.to_string(),
            start: obj.get("start")?.as_u64()? as usize,
            end: obj.get("end")?.as_u64()? as usize,
            replacement: obj.get("replacement")?.as_str()?.to_string(),
        },
        _ => return None,
    };

    let description = describe_operation(&operation);
    Some(PlanStep {
        description,
        operation,
    })
}

/// Human-readable description for a plan operation.
fn describe_operation(op: &PlanOperation) -> String {
    match op {
        PlanOperation::Rename { old, new, .. } => format!("Rename {old} to {new}"),
        PlanOperation::Delete { name, .. } => format!("Delete {name}"),
        PlanOperation::Create { path, .. } => format!("Create {path}"),
        PlanOperation::Inspect { symbol_name, .. } => format!("Inspect {symbol_name}"),
        PlanOperation::Modify {
            file, start, end, ..
        } => {
            format!("Modify {file}:{start}-{end}")
        }
    }
}

/// Detect plan intent from the observation query.
fn detect_intent(query: &str) -> PlanIntent {
    // "rename X to Y" or "rename X -> Y"
    if let Some(rest) = query.strip_prefix("rename ") {
        if let Some((_, new)) = rest.split_once(" to ") {
            return PlanIntent::Rename {
                new_name: new.trim().to_string(),
            };
        }
        if let Some((_, new)) = rest.split_once(" -> ") {
            return PlanIntent::Rename {
                new_name: new.trim().to_string(),
            };
        }
    }

    if query.contains("delete ") || query.contains("remove ") {
        return PlanIntent::Delete;
    }

    if query.contains("create ") || query.contains("add ") {
        return PlanIntent::Create {
            content: String::new(),
        };
    }

    PlanIntent::Inspect
}

/// Returns true if operation `a` must happen before operation `b`.
fn should_precede(a: &PlanOperation, b: &PlanOperation) -> bool {
    match (a, b) {
        // Inspect before Rename of same symbol
        (PlanOperation::Inspect { symbol_name, .. }, PlanOperation::Rename { old, .. }) => {
            symbol_name == old
        }
        // Inspect before Delete of same symbol
        (PlanOperation::Inspect { symbol_name, .. }, PlanOperation::Delete { name, .. }) => {
            symbol_name == name
        }
        // Rename before Delete for same symbol
        (PlanOperation::Rename { old, .. }, PlanOperation::Delete { name, .. }) => old == name,
        // Create before Modify for same file
        (PlanOperation::Create { path, .. }, PlanOperation::Modify { file, .. }) => path == file,
        _ => false,
    }
}

/// A file region.
#[derive(Clone, Debug)]
struct FileRegion {
    /// File path
    file: String,
    /// Region start
    start: usize,
    /// Region end
    end: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_planner_creation() {
        let _planner = Planner::new();
    }

    #[tokio::test]
    async fn test_generate_steps() {
        let planner = Planner::new();

        let observation = crate::observe::Observation {
            query: "test".to_string(),
            symbols: vec![],
            summary: None,
        };

        let steps = planner.generate_steps(&observation).await.unwrap();
        // Should succeed (even if empty)
        assert!(steps.is_empty());
    }

    #[tokio::test]
    async fn test_detect_conflicts_empty() {
        let planner = Planner::new();

        let steps = vec![];
        let conflicts = planner.detect_conflicts(&steps).unwrap();
        assert!(conflicts.is_empty());
    }

    #[tokio::test]
    async fn test_order_steps() {
        let planner = Planner::new();

        let mut steps = vec![
            PlanStep {
                description: "Delete foo".to_string(),
                operation: PlanOperation::Delete {
                    name: "foo".to_string(),
                    file: None,
                },
            },
            PlanStep {
                description: "Rename foo to bar".to_string(),
                operation: PlanOperation::Rename {
                    old: "foo".to_string(),
                    new: "bar".to_string(),
                    file: None,
                },
            },
        ];

        planner.order_steps(&mut steps).unwrap();

        // Rename should now come before Delete
        assert!(matches!(steps[0].operation, PlanOperation::Rename { .. }));
        assert!(matches!(steps[1].operation, PlanOperation::Delete { .. }));
    }

    #[tokio::test]
    async fn test_generate_rollback() {
        let planner = Planner::new();

        let steps = vec![PlanStep {
            description: "Create file".to_string(),
            operation: PlanOperation::Create {
                path: "/tmp/test.rs".to_string(),
                content: "fn test() {}".to_string(),
            },
        }];

        let rollback = planner.generate_rollback(&steps);

        assert_eq!(rollback.len(), 1);
        assert_eq!(rollback[0].description, "Rollback: Create file");
        assert!(matches!(
            rollback[0].operation,
            RollbackOperation::Delete { .. }
        ));
    }

    #[tokio::test]
    async fn test_estimate_impact() {
        let planner = Planner::new();

        let steps = vec![PlanStep {
            description: "Create test.rs".to_string(),
            operation: PlanOperation::Create {
                path: "/tmp/test.rs".to_string(),
                content: "fn test() {}".to_string(),
            },
        }];

        let _impact = planner.estimate_impact(&steps).await.unwrap();
    }

    #[tokio::test]
    async fn test_planner_no_llm_uses_regex() {
        let planner = Planner::new();
        assert!(planner.llm.is_none());

        let observation = crate::observe::Observation {
            query: "rename old_func to new_func".to_string(),
            symbols: vec![],
            summary: None,
        };

        let steps = planner.generate_steps(&observation).await.unwrap();
        // No symbols → regex path produces empty steps, but doesn't error
        assert!(steps.is_empty());
    }

    #[tokio::test]
    async fn test_planner_llm_generates_steps() {
        use std::sync::Arc;

        // MockProvider returns valid JSON steps
        let json_steps =
            r#"[{"operation":"inspect","symbol_name":"auth_middleware","symbol_id":42}]"#;
        let mock = Arc::new(crate::llm::MockProvider::new(json_steps));

        let planner = Planner::new().with_llm(mock);
        assert!(planner.llm.is_some());

        let observation = crate::observe::Observation {
            query: "where is the auth middleware?".to_string(),
            symbols: vec![],
            summary: None,
        };

        let steps = planner.generate_steps(&observation).await.unwrap();
        assert_eq!(steps.len(), 1);
        assert!(matches!(
            &steps[0].operation,
            PlanOperation::Inspect { symbol_name, .. } if symbol_name == "auth_middleware"
        ));
    }

    #[tokio::test]
    async fn test_planner_llm_fallback_on_parse_error() {
        use std::sync::Arc;

        // MockProvider returns garbage — should fall back to regex
        let mock = Arc::new(crate::llm::MockProvider::new("not valid json at all"));

        let planner = Planner::new().with_llm(mock);

        let observation = crate::observe::Observation {
            query: "inspect test query".to_string(),
            symbols: vec![],
            summary: None,
        };

        // Should NOT error — falls back to regex detect_intent
        let steps = planner.generate_steps(&observation).await.unwrap();
        // No symbols matched, regex produces Inspect with no targets → empty
        assert!(steps.is_empty());
    }

    // ── Task 2: attempt history ──────────────────────────────────────────

    #[tokio::test]
    async fn test_generate_fix_steps_accepts_previous_steps() {
        use crate::llm::MockProvider;
        let llm = Arc::new(MockProvider::new(
            r#"[{"operation":"inspect","symbol_name":"foo","symbol_id":1}]"#,
        ));
        let planner = Planner::new().with_llm(llm);
        let obs = crate::observe::Observation {
            query: "fix the error".to_string(),
            symbols: vec![],
            summary: None,
        };
        let prev = vec![PlanStep {
            description: "Previous attempt".to_string(),
            operation: PlanOperation::Create {
                path: "src/foo.rs".to_string(),
                content: "fn foo() {}".to_string(),
            },
        }];
        // Should compile and run — previous_steps accepted without error
        let result = planner
            .generate_fix_steps(&obs, &["compile error".to_string()], &prev)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_generate_fix_steps_empty_previous_no_error() {
        use crate::llm::MockProvider;
        let llm = Arc::new(MockProvider::new("[]"));
        let planner = Planner::new().with_llm(llm);
        let obs = crate::observe::Observation {
            query: "fix".to_string(),
            symbols: vec![],
            summary: None,
        };
        let result = planner
            .generate_fix_steps(&obs, &["error".to_string()], &[])
            .await;
        assert!(result.is_ok());
    }

    // ── Gap 7: retry memoization ─────────────────────────────────────────

    #[tokio::test]
    async fn test_dedup_filters_repeated_fix_step() {
        use crate::llm::MockProvider;
        let llm = Arc::new(MockProvider::new(
            r#"[{"operation":"modify","file":"src/lib.rs","start":10,"end":20,"replacement":"fixed"}]"#,
        ));
        let planner = Planner::new().with_llm(llm);
        let obs = crate::observe::Observation {
            query: "fix error".to_string(),
            symbols: vec![],
            summary: None,
        };
        let prev = vec![PlanStep {
            description: "Modify src/lib.rs:10-20".to_string(),
            operation: PlanOperation::Modify {
                file: "src/lib.rs".to_string(),
                start: 10,
                end: 20,
                replacement: "fixed".to_string(),
            },
        }];
        let result = planner
            .generate_fix_steps(&obs, &["error".to_string()], &prev)
            .await
            .unwrap();
        assert!(
            result.is_empty(),
            "repeated operation must be filtered out, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_dedup_keeps_new_fix_step() {
        use crate::llm::MockProvider;
        let llm = Arc::new(MockProvider::new(
            r#"[{"operation":"modify","file":"src/lib.rs","start":30,"end":40,"replacement":"new_fix"}]"#,
        ));
        let planner = Planner::new().with_llm(llm);
        let obs = crate::observe::Observation {
            query: "fix error".to_string(),
            symbols: vec![],
            summary: None,
        };
        let prev = vec![PlanStep {
            description: "Modify src/lib.rs:10-20".to_string(),
            operation: PlanOperation::Modify {
                file: "src/lib.rs".to_string(),
                start: 10,
                end: 20,
                replacement: "old_fix".to_string(),
            },
        }];
        let result = planner
            .generate_fix_steps(&obs, &["error".to_string()], &prev)
            .await
            .unwrap();
        assert_eq!(result.len(), 1, "new operation must pass through dedup");
    }

    #[tokio::test]
    async fn test_dedup_mixed_new_and_repeated() {
        use crate::llm::MockProvider;
        let llm = Arc::new(MockProvider::new(
            r#"[
                {"operation":"modify","file":"src/lib.rs","start":10,"end":20,"replacement":"already_tried"},
                {"operation":"create","path":"src/new.rs","content":"fn new_fix() {}"}
            ]"#,
        ));
        let planner = Planner::new().with_llm(llm);
        let obs = crate::observe::Observation {
            query: "fix error".to_string(),
            symbols: vec![],
            summary: None,
        };
        let prev = vec![PlanStep {
            description: "Modify src/lib.rs:10-20".to_string(),
            operation: PlanOperation::Modify {
                file: "src/lib.rs".to_string(),
                start: 10,
                end: 20,
                replacement: "already_tried".to_string(),
            },
        }];
        let result = planner
            .generate_fix_steps(&obs, &["error".to_string()], &prev)
            .await
            .unwrap();
        assert_eq!(result.len(), 1, "only the new operation should remain");
        assert!(
            matches!(&result[0].operation, PlanOperation::Create { path, .. } if path == "src/new.rs"),
            "remaining step should be the Create, got: {:?}",
            result[0].operation
        );
    }

    // ── INT-11: Planner retry memoization ───────────────────────────────

    #[tokio::test]
    async fn test_planner_memoizes_attempted_operations() {
        use crate::llm::MockProvider;
        // LLM returns an Inspect step
        let mock = Arc::new(MockProvider::new(
            r#"[{"operation":"inspect","symbol_name":"foo","symbol_id":1}]"#,
        ));
        let mut planner = Planner::new().with_llm(mock);
        let obs = crate::observe::Observation {
            query: "fix it".to_string(),
            symbols: vec![],
            summary: None,
        };

        // First call: should return the step
        let first = planner.fix_once(&obs, &["err".to_string()]).await.unwrap();
        assert_eq!(first.len(), 1);

        // Second call: LLM returns same step — planner should filter it as already tried
        let second = planner.fix_once(&obs, &["err".to_string()]).await.unwrap();
        assert!(
            second.is_empty(),
            "second call should deduplicate already-attempted operations"
        );
    }

    // ── INT-3: AgentContext prefix in LLM prompts ────────────────────────

    #[tokio::test]
    async fn test_planner_context_prefix_in_generate_steps() {
        use crate::llm::CapturingMockProvider;
        let mock = Arc::new(CapturingMockProvider::new(
            r#"[{"operation":"inspect","symbol_name":"foo","symbol_id":1}]"#,
        ));
        let ctx = crate::context::AgentContext::from_path(std::path::Path::new("/tmp/test-proj"));
        let planner = Planner::new().with_context(&ctx).with_llm(mock.clone());

        let obs = crate::observe::Observation {
            query: "find the bug".to_string(),
            symbols: vec![],
            summary: None,
        };
        let _ = planner.generate_steps(&obs).await.unwrap();

        let captured = mock.last_prompt.lock().unwrap().clone().unwrap_or_default();
        assert!(
            captured.contains("[Project:"),
            "prompt should contain context prefix, got: {captured:?}"
        );
    }

    #[tokio::test]
    async fn test_planner_context_prefix_in_generate_fix_steps() {
        use crate::llm::CapturingMockProvider;
        let mock = Arc::new(CapturingMockProvider::new(
            r#"[{"operation":"inspect","symbol_name":"bar","symbol_id":2}]"#,
        ));
        let ctx = crate::context::AgentContext::from_path(std::path::Path::new("/tmp/test-proj"));
        let planner = Planner::new().with_context(&ctx).with_llm(mock.clone());

        let obs = crate::observe::Observation {
            query: "fix compile error".to_string(),
            symbols: vec![],
            summary: None,
        };
        let _ = planner
            .generate_fix_steps(&obs, &["error: mismatched types".to_string()], &[])
            .await
            .unwrap();

        let captured = mock.last_prompt.lock().unwrap().clone().unwrap_or_default();
        assert!(
            captured.contains("[Project:"),
            "fix prompt should contain context prefix, got: {captured:?}"
        );
    }

    // ── INT-10: CodeGenerator enriches Create steps ──────────────────────

    #[tokio::test]
    async fn test_planner_enriches_create_step_via_generator() {
        use crate::generate::Generator;
        use crate::llm::MockProvider;

        // Planning LLM returns a Create step with bare description as content
        let planning_llm = Arc::new(MockProvider::new(
            r#"[{"operation":"create","path":"src/auth.rs","content":"authentication handler"}]"#,
        ));
        // Generator LLM returns actual Rust code
        let gen_llm = Arc::new(MockProvider::new(
            "fn authenticate(token: &str) -> bool { true }",
        ));

        let temp_dir = tempfile::TempDir::new().unwrap();
        let forge = forge_core::Forge::open(temp_dir.path()).await.unwrap();
        let generator = Arc::new(Generator::new(Arc::new(forge), gen_llm));

        let planner = Planner::new()
            .with_llm(planning_llm)
            .with_generator(generator);

        let obs = crate::observe::Observation {
            query: "add authentication".to_string(),
            symbols: vec![],
            summary: None,
        };

        let steps = planner.generate_steps(&obs).await.unwrap();
        assert_eq!(steps.len(), 1);

        if let PlanOperation::Create { content, .. } = &steps[0].operation {
            assert!(
                content.contains("fn authenticate"),
                "content should be generated code, got: {content:?}"
            );
        } else {
            panic!("expected Create step, got: {:?}", steps[0].operation);
        }
    }
}

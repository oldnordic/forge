//! Planning engine - Execution plan generation from observations.
//!
//! This module implements the planning phase of the agent loop, generating
//! execution steps from observations and constraints.

use crate::Result;

/// Planner for generating execution plans.
///
/// The Planner creates ordered steps from observations, ensuring
/// dependencies are satisfied and conflicts are detected.
#[derive(Clone, Default)]
pub struct Planner {}

impl Planner {
    /// Creates a new planner.
    pub fn new() -> Self {
        Self::default()
    }

    /// Generates execution steps from an observation.
    ///
    /// Parses the observation query for intent keywords to decide what
    /// operations to generate. Falls back to Inspect for unrecognized queries.
    pub async fn generate_steps(
        &self,
        observation: &super::observe::Observation,
    ) -> Result<Vec<PlanStep>> {
        let query_lower = observation.query.to_lowercase();
        let mut steps = Vec::new();

        // Detect intent from query
        let intent = detect_intent(&query_lower);

        for symbol in &observation.symbols {
            match &intent {
                PlanIntent::Rename { new_name } => {
                    steps.push(PlanStep {
                        description: format!("Rename {} to {}", symbol.name, new_name),
                        operation: PlanOperation::Rename {
                            old: symbol.name.clone(),
                            new: new_name.clone(),
                        },
                    });
                }
                PlanIntent::Delete => {
                    steps.push(PlanStep {
                        description: format!("Delete {}", symbol.name),
                        operation: PlanOperation::Delete {
                            name: symbol.name.clone(),
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
                            symbol_name: symbol.name.clone(),
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
                PlanOperation::Rename { old, .. } => {
                    // Extract file from symbol name (simplified)
                    if let Some(file) = self.extract_file_from_symbol(old) {
                        affected_files.insert(file);
                    }
                }
                PlanOperation::Delete { name } => {
                    if let Some(file) = self.extract_file_from_symbol(name) {
                        affected_files.insert(file);
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
                    PlanOperation::Delete { name } => {
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

    /// Extracts file path from symbol name (simplified).
    fn extract_file_from_symbol(&self, _symbol: &str) -> Option<String> {
        // In production, this would query the graph for symbol location
        // For now, return None
        None
    }

    /// Gets the file region affected by a step.
    fn get_step_region(&self, step: &PlanStep) -> Option<FileRegion> {
        match &step.operation {
            PlanOperation::Rename { .. } | PlanOperation::Delete { .. } => None,
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
#[derive(Clone, Debug)]
pub struct PlanStep {
    /// Step description
    pub description: String,
    /// Operation to perform
    pub operation: PlanOperation,
}

/// Operation to perform in a plan step.
#[derive(Clone, Debug)]
pub enum PlanOperation {
    /// Rename a symbol
    Rename { old: String, new: String },
    /// Delete a symbol
    Delete { name: String },
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
        (PlanOperation::Inspect { symbol_name, .. }, PlanOperation::Delete { name }) => {
            symbol_name == name
        }
        // Rename before Delete for same symbol
        (PlanOperation::Rename { old, .. }, PlanOperation::Delete { name }) => old == name,
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
                },
            },
            PlanStep {
                description: "Rename foo to bar".to_string(),
                operation: PlanOperation::Rename {
                    old: "foo".to_string(),
                    new: "bar".to_string(),
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
}

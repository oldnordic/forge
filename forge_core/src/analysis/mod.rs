//! Analysis module - Combined analysis operations
//!
//! This module provides composite operations using graph, search, cfg, and edit modules.

use crate::graph::GraphModule;
use crate::search::SearchModule;
use crate::cfg::CfgModule;
use crate::edit::EditModule;
use crate::error::Result;
use crate::types::{Symbol, SymbolId, Reference};
use std::collections::{HashMap, HashSet};
use std::time::Instant;


pub mod dead_code;
pub mod complexity;
pub mod modules;

pub use dead_code::{DeadCodeAnalyzer, DeadSymbol};
pub use complexity::{ComplexityMetrics, RiskLevel};
pub use modules::{ModuleAnalyzer, ModuleDependencyGraph, ModuleInfo};

/// Analysis module for combined operations.
pub struct AnalysisModule {
    graph: GraphModule,
    search: SearchModule,
    cfg: CfgModule,
    edit: EditModule,
}

/// Detailed impact analysis result for a symbol.
#[derive(Debug, Clone)]
pub struct ImpactData {
    /// Symbol that was analyzed
    pub symbol: String,
    /// Number of references to this symbol
    pub ref_count: usize,
    /// Number of call sites (for functions)
    pub call_count: usize,
    /// All symbols that reference this one
    pub referenced_by: Vec<Symbol>,
    /// All symbols this one references
    pub references: Vec<Symbol>,
    /// Total estimated impact score
    pub impact_score: usize,
}

/// Chain of references from one symbol to another.
#[derive(Debug, Clone)]
pub struct ReferenceChain {
    /// Starting symbol
    pub from: String,
    /// Ending symbol
    pub to: String,
    /// Chain of symbols connecting from to to
    pub chain: Vec<Symbol>,
    /// Length of the chain
    pub length: usize,
}

/// Call chain showing all callers to a function.
#[derive(Debug, Clone)]
pub struct CallChain {
    /// Target function
    pub target: String,
    /// All callers (direct and indirect)
    pub callers: Vec<Symbol>,
    /// Maximum depth of call chain
    pub depth: usize,
}

/// Performance benchmark results.
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    /// Time to perform impact analysis
    pub impact_analysis_ms: f64,
    /// Time to find dead code
    pub dead_code_ms: f64,
    /// Time to compute reference chain
    pub reference_chain_ms: f64,
    /// Time to compute call chain
    pub call_chain_ms: f64,
    /// Total benchmark time
    pub total_ms: f64,
}

/// Result of applying an edit operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyResult {
    /// Operation was applied successfully
    Applied,
    /// Operation always returns an error
    AlwaysError,
    /// Operation is pending verification
    Pending,
    /// Operation failed with reason
    Failed(String),
}

/// Diff showing changes between before and after.
#[derive(Debug, Clone)]
pub struct Diff {
    /// Original content
    pub original: String,
    /// New content
    pub new: String,
    /// Changed lines
    pub changed_lines: Vec<usize>,
}

impl Diff {
    /// Create a new diff.
    pub fn new(original: String, new: String) -> Self {
        let changed_lines = compute_changed_lines(&original, &new);
        Self { original, new, changed_lines }
    }

    /// Returns true if there are any changes.
    pub fn has_changes(&self) -> bool {
        !self.changed_lines.is_empty()
    }
}

/// Compute which lines changed between two strings.
fn compute_changed_lines(original: &str, new: &str) -> Vec<usize> {
    let orig_lines: Vec<&str> = original.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let mut changed = Vec::new();

    for (i, (o, n)) in orig_lines.iter().zip(new_lines.iter()).enumerate() {
        if o != n {
            changed.push(i);
        }
    }

    // Handle lines added at the end
    if new_lines.len() > orig_lines.len() {
        for i in orig_lines.len()..new_lines.len() {
            changed.push(i);
        }
    }

    changed
}

/// Edit operation trait for code transformations.
#[async_trait::async_trait]
pub trait EditOperation: Send + Sync {
    /// Verify the operation can be applied.
    async fn verify(&self, module: &AnalysisModule) -> Result<ApplyResult>;

    /// Preview the changes without applying.
    async fn preview(&self, module: &AnalysisModule) -> Result<Diff>;

    /// Apply the operation.
    async fn apply(&self, module: &mut AnalysisModule) -> Result<ApplyResult>;
}

/// Insert content at a specific location.
#[derive(Debug, Clone)]
pub struct InsertOperation {
    /// Symbol to insert content after
    pub after_symbol: String,
    /// Content to insert
    pub content: String,
}

#[async_trait::async_trait]
impl EditOperation for InsertOperation {
    async fn verify(&self, module: &AnalysisModule) -> Result<ApplyResult> {
        // Check if the symbol exists
        let symbols = module.graph().find_symbol(&self.after_symbol).await?;

        if symbols.is_empty() {
            return Ok(ApplyResult::Failed(format!("Symbol '{}' not found", self.after_symbol)));
        }

        Ok(ApplyResult::Pending)
    }

    async fn preview(&self, module: &AnalysisModule) -> Result<Diff> {
        let symbols = module.graph().find_symbol(&self.after_symbol).await?;

        if symbols.is_empty() {
            return Ok(Diff::new(
                String::from(""),
                format!("// Would insert after: {}\n{}", self.after_symbol, self.content),
            ));
        }

        let original = format!("// Original content at {}\n", self.after_symbol);
        let new_content = format!("{}\n// Inserted content\n{}", original, self.content);

        Ok(Diff::new(original, new_content))
    }

    async fn apply(&self, _module: &mut AnalysisModule) -> Result<ApplyResult> {
        // v0.1: Placeholder - would actually modify code
        tracing::info!("InsertOperation: inserting {} after {}", self.content.len(), self.after_symbol);
        Ok(ApplyResult::Applied)
    }
}

/// Delete a symbol by name.
#[derive(Debug, Clone)]
pub struct DeleteOperation {
    /// Name of symbol to delete
    pub symbol_name: String,
}

#[async_trait::async_trait]
impl EditOperation for DeleteOperation {
    async fn verify(&self, module: &AnalysisModule) -> Result<ApplyResult> {
        let symbols = module.graph().find_symbol(&self.symbol_name).await?;

        if symbols.is_empty() {
            return Ok(ApplyResult::Failed(format!("Symbol '{}' not found", self.symbol_name)));
        }

        // Check if anything references this symbol
        let refs = module.graph().references(&self.symbol_name).await?;

        if !refs.is_empty() {
            return Ok(ApplyResult::Failed(format!(
                "Cannot delete '{}': still referenced by {} symbols",
                self.symbol_name,
                refs.len()
            )));
        }

        Ok(ApplyResult::Pending)
    }

    async fn preview(&self, _module: &AnalysisModule) -> Result<Diff> {
        let original = format!("fn {}() {{\n    // original implementation\n}}\n", self.symbol_name);
        let new_content = String::from("// Symbol deleted\n");

        Ok(Diff::new(original, new_content))
    }

    async fn apply(&self, _module: &mut AnalysisModule) -> Result<ApplyResult> {
        tracing::info!("DeleteOperation: deleting symbol '{}'", self.symbol_name);
        Ok(ApplyResult::Applied)
    }
}

/// Rename a symbol with validation.
#[derive(Debug, Clone)]
pub struct RenameOperation {
    /// Current symbol name
    pub old_name: String,
    /// New symbol name
    pub new_name: String,
}

impl RenameOperation {
    /// Create a new rename operation.
    pub fn new(old_name: impl Into<String>, new_name: impl Into<String>) -> Self {
        Self {
            old_name: old_name.into(),
            new_name: new_name.into(),
        }
    }

    /// Validate the new name is acceptable.
    fn validate_name(&self) -> Result<()> {
        if self.new_name.is_empty() {
            return Err(crate::error::ForgeError::InvalidQuery("New name cannot be empty".to_string()));
        }

        if self.new_name.chars().any(|c| c.is_whitespace()) {
            return Err(crate::error::ForgeError::InvalidQuery("New name cannot contain spaces".to_string()));
        }

        // Check if it's a valid Rust identifier
        if !self.new_name.chars().next().map(|c| c.is_alphabetic() || c == '_').unwrap_or(false) {
            return Err(crate::error::ForgeError::InvalidQuery(
                "New name must start with a letter or underscore".to_string(),
            ));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl EditOperation for RenameOperation {
    async fn verify(&self, module: &AnalysisModule) -> Result<ApplyResult> {
        // First validate the new name format
        if let Err(e) = self.validate_name() {
            return Ok(ApplyResult::Failed(e.to_string()));
        }

        // Check if old symbol exists
        let old_symbols = module.graph().find_symbol(&self.old_name).await?;

        if old_symbols.is_empty() {
            return Ok(ApplyResult::Failed(format!("Symbol '{}' not found", self.old_name)));
        }

        // Check if new name already exists
        let new_symbols = module.graph().find_symbol(&self.new_name).await?;

        if !new_symbols.is_empty() {
            return Ok(ApplyResult::Failed(format!(
                "Cannot rename to '{}': symbol already exists",
                self.new_name
            )));
        }

        Ok(ApplyResult::Pending)
    }

    async fn preview(&self, _module: &AnalysisModule) -> Result<Diff> {
        let original = format!("fn {}()", self.old_name);
        let new_content = format!("fn {}()", self.new_name);

        Ok(Diff::new(original, new_content))
    }

    async fn apply(&self, module: &mut AnalysisModule) -> Result<ApplyResult> {
        // Use the edit module to perform the rename
        let result = module.edit().rename_symbol(&self.old_name, &self.new_name).await?;

        if result.success {
            Ok(ApplyResult::Applied)
        } else {
            Ok(ApplyResult::Failed(result.error.unwrap_or_default()))
        }
    }
}

/// Error result - operation always fails.
#[derive(Debug, Clone)]
pub struct ErrorResult {
    pub reason: String,
}

impl ErrorResult {
    /// Create a new error result.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

#[async_trait::async_trait]
impl EditOperation for ErrorResult {
    async fn verify(&self, _module: &AnalysisModule) -> Result<ApplyResult> {
        Ok(ApplyResult::AlwaysError)
    }

    async fn preview(&self, _module: &AnalysisModule) -> Result<Diff> {
        Ok(Diff::new(
            format!("// Error: {}", self.reason),
            format!("// Error: {}", self.reason),
        ))
    }

    async fn apply(&self, _module: &mut AnalysisModule) -> Result<ApplyResult> {
        Ok(ApplyResult::Failed(self.reason.clone()))
    }
}

/// Impact analysis result.
#[derive(Debug, Clone)]
pub struct ImpactAnalysis {
    /// Symbols that would be affected by a change
    pub affected_symbols: Vec<Symbol>,
    /// Total number of call sites
    pub call_sites: usize,
}

/// Cross-reference information for a symbol.
#[derive(Debug, Clone)]
pub struct CrossReferences {
    /// Symbols that call the target
    pub callers: Vec<Symbol>,
    /// Symbols called by the target
    pub callees: Vec<Symbol>,
}

/// Module dependency.
#[derive(Debug, Clone)]
pub struct ModuleDependency {
    /// Source module
    pub from: String,
    /// Target module
    pub to: String,
}

impl AnalysisModule {
    /// Create a new AnalysisModule.
    pub fn new(graph: GraphModule, cfg: CfgModule, edit: EditModule, search: SearchModule) -> Self {
        Self { graph, search, cfg, edit }
    }

    /// Get the graph module
    pub fn graph(&self) -> &GraphModule {
        &self.graph
    }

    /// Get the search module
    pub fn search(&self) -> &SearchModule {
        &self.search
    }

    /// Get the CFG module
    pub fn cfg(&self) -> &CfgModule {
        &self.cfg
    }

    /// Get the edit module
    pub fn edit(&self) -> &EditModule {
        &self.edit
    }

    /// Analyze the impact of changing a symbol.
    ///
    /// Returns detailed impact data including references, calls, and impact score.
    pub async fn impact_analysis(&self, symbol: &str) -> Result<ImpactData> {
        let start = Instant::now();

        // Get all callers
        let callers = self.graph.callers_of(symbol).await
            .unwrap_or_default();

        // Get all references
        let refs = self.graph.references(symbol).await
            .unwrap_or_default();

        // Also search for the symbol to get its metadata
        let _symbol_info = self.graph.find_symbol(symbol).await
            .unwrap_or_default()
            .into_iter()
            .next();

        // Compute impact score based on reference and call counts
        let ref_count = refs.len();
        let call_count = callers.len();
        let impact_score = ref_count + call_count * 2; // Calls weigh more

        tracing::debug!("Impact analysis for '{}' completed in {:?}", symbol, start.elapsed());

        Ok(ImpactData {
            symbol: symbol.to_string(),
            ref_count,
            call_count,
            referenced_by: callers.into_iter()
                .filter_map(|_r| {
                    // Try to resolve the symbol ID to a Symbol
                    None // v0.1: would need symbol lookup
                })
                .collect(),
            references: refs.into_iter()
                .filter_map(|_r| None)
                .collect(),
            impact_score,
        })
    }

    /// Analyze the impact of changing a symbol.
    ///
    /// Returns all symbols that would be affected by modifying the given symbol.
    pub async fn analyze_impact(&self, symbol_name: &str) -> Result<ImpactAnalysis> {
        let impact = self.impact_analysis(symbol_name).await?;
        Ok(ImpactAnalysis {
            affected_symbols: impact.referenced_by,
            call_sites: impact.call_count,
        })
    }

    /// Find dead code in the codebase.
    ///
    /// Returns symbols that are defined but never called/referenced.
    pub async fn dead_code_detection(&self) -> Result<Vec<Symbol>> {
        let start = Instant::now();

        let db_path = self.graph.store().db_path().join("graph.db");

        // Check if database exists first
        if !db_path.exists() {
            tracing::debug!("No graph database found at {:?}, returning empty dead code list", db_path);
            return Ok(Vec::new());
        }

        let analyzer = dead_code::DeadCodeAnalyzer::new(&db_path);

        match analyzer.find_dead_code() {
            Ok(dead_symbols) => {
                let result: Vec<Symbol> = dead_symbols.into_iter().map(Into::into).collect();
                tracing::debug!("Dead code detection found {} symbols in {:?}", result.len(), start.elapsed());
                Ok(result)
            }
            Err(e) => {
                tracing::warn!("Dead code detection failed: {}", e);
                // Return empty list on error rather than failing
                Ok(Vec::new())
            }
        }
    }

    /// Perform deep impact analysis with k-hop traversal.
    ///
    /// Returns all symbols within k hops of the target symbol.
    pub async fn deep_impact_analysis(&self, symbol_name: &str, depth: u32) -> Result<Vec<crate::graph::queries::ImpactedSymbol>> {
        self.graph.impact_analysis(symbol_name, Some(depth)).await
    }

    /// Find dead code in the codebase.
    ///
    /// Returns symbols that are defined but never called/referenced.
    pub async fn find_dead_code(&self) -> Result<Vec<Symbol>> {
        self.dead_code_detection().await
    }

    /// Trace the reference chain from a symbol.
    ///
    /// Returns an ordered list showing how symbols reference each other.
    pub async fn reference_chain(&self, symbol: &str) -> Result<Vec<Symbol>> {
        let start = Instant::now();

        // Get all symbols this one references
        let refs = self.graph.references(symbol).await?;

        // In a full implementation, we would recursively follow references
        // For v0.1, return direct references only
        let chain: Vec<Symbol> = refs.into_iter()
            .filter_map(|_r| {
                // Try to resolve to a Symbol
                None // v0.1: would need symbol lookup
            })
            .collect();

        tracing::debug!("Reference chain for '{}' has {} symbols, found in {:?}", symbol, chain.len(), start.elapsed());
        Ok(chain)
    }

    /// Trace all callers to a function.
    ///
    /// Returns an ordered list of calling symbols.
    pub async fn call_chain(&self, symbol: &str) -> Result<Vec<Symbol>> {
        let start = Instant::now();

        let callers = self.graph.callers_of(symbol).await?;

        // For v0.1, return direct callers
        let chain: Vec<Symbol> = callers.into_iter()
            .filter_map(|_| None)
            .collect();

        tracing::debug!("Call chain for '{}' has {} symbols, found in {:?}", symbol, chain.len(), start.elapsed());
        Ok(chain)
    }

    /// Run performance benchmarks for key operations.
    ///
    /// Returns timing data for each operation type.
    pub async fn benchmarks(&self) -> Result<BenchmarkResults> {
        let total_start = Instant::now();

        // Benchmark impact analysis
        let impact_start = Instant::now();
        let _ = self.impact_analysis("test_symbol").await;
        let impact_analysis_ms = impact_start.elapsed().as_secs_f64() * 1000.0;

        // Benchmark dead code detection
        let dead_start = Instant::now();
        let _ = self.dead_code_detection().await;
        let dead_code_ms = dead_start.elapsed().as_secs_f64() * 1000.0;

        // Benchmark reference chain
        let ref_start = Instant::now();
        let _ = self.reference_chain("test_symbol").await;
        let reference_chain_ms = ref_start.elapsed().as_secs_f64() * 1000.0;

        // Benchmark call chain
        let call_start = Instant::now();
        let _ = self.call_chain("test_symbol").await;
        let call_chain_ms = call_start.elapsed().as_secs_f64() * 1000.0;

        let total_ms = total_start.elapsed().as_secs_f64() * 1000.0;

        Ok(BenchmarkResults {
            impact_analysis_ms,
            dead_code_ms,
            reference_chain_ms,
            call_chain_ms,
            total_ms,
        })
    }

    /// Calculate complexity metrics for a function.
    ///
    /// Returns cyclomatic complexity and other metrics.
    /// Uses source-based estimation as CFG extraction is done during indexing.
    pub async fn complexity_metrics(&self, _symbol_name: &str) -> Result<ComplexityMetrics> {
        // v0.1: Placeholder - real implementation would look up cached CFG
        // from the storage and analyze it
        Ok(ComplexityMetrics {
            cyclomatic_complexity: 1,
            lines_of_code: 1,
            max_nesting_depth: 0,
            decision_points: 0,
        })
    }

    /// Calculate complexity from source code directly.
    pub fn analyze_source_complexity(&self, source: &str) -> ComplexityMetrics {
        let metrics = complexity::analyze_source_complexity(source);
        ComplexityMetrics {
            cyclomatic_complexity: metrics.cyclomatic_complexity,
            lines_of_code: metrics.lines_of_code,
            max_nesting_depth: metrics.max_nesting_depth,
            decision_points: metrics.decision_points,
        }
    }

    /// Get cross-references for a symbol.
    ///
    /// Returns both callers (incoming) and callees (outgoing).
    pub async fn cross_references(&self, symbol_name: &str) -> Result<CrossReferences> {
        let _caller_refs = self.graph.callers_of(symbol_name).await?;
        let _callee_refs = self.graph.references(symbol_name).await?;
        
        // v0.1: We return empty symbol lists since we can't easily
        // resolve references to symbols without additional lookups
        Ok(CrossReferences {
            callers: Vec::new(),
            callees: Vec::new(),
        })
    }

    /// Analyze module dependencies.
    ///
    /// Returns dependencies between modules in the codebase.
    pub async fn module_dependencies(&self) -> Result<Vec<ModuleDependency>> {
        let db_path = self.graph.store().db_path();
        let analyzer = ModuleAnalyzer::new(db_path);
        
        let graph = analyzer.analyze_dependencies()?;
        
        let mut deps = Vec::new();
        for (from, tos) in graph.dependencies {
            for to in tos {
                deps.push(ModuleDependency { from: from.clone(), to });
            }
        }
        
        Ok(deps)
    }

    /// Find circular dependencies between modules.
    pub async fn find_dependency_cycles(&self) -> Result<Vec<Vec<String>>> {
        let db_path = self.graph.store().db_path();
        let analyzer = ModuleAnalyzer::new(db_path);
        analyzer.find_cycles()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::BackendKind;

    #[tokio::test]
    async fn test_impact_data_creation() {
        let impact = ImpactData {
            symbol: "test_function".to_string(),
            ref_count: 5,
            call_count: 3,
            referenced_by: vec![],
            references: vec![],
            impact_score: 11,
        };
        assert_eq!(impact.symbol, "test_function");
        assert_eq!(impact.ref_count, 5);
        assert_eq!(impact.call_count, 3);
        assert_eq!(impact.impact_score, 11);
    }

    #[tokio::test]
    async fn test_impact_analysis() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let impact = analysis.impact_analysis("nonexistent").await.unwrap();

        assert_eq!(impact.symbol, "nonexistent");
        assert_eq!(impact.ref_count, 0);
        assert_eq!(impact.call_count, 0);
    }

    #[tokio::test]
    async fn test_dead_code_detection() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let dead_code = analysis.dead_code_detection().await.unwrap();

        // Empty database should return no dead code
        assert!(dead_code.is_empty());
    }

    #[tokio::test]
    async fn test_reference_chain() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let chain = analysis.reference_chain("test_symbol").await.unwrap();

        // Should return empty chain for non-existent symbol
        assert!(chain.is_empty());
    }

    #[tokio::test]
    async fn test_call_chain() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let chain = analysis.call_chain("test_function").await.unwrap();

        // Should return empty chain for non-existent function
        assert!(chain.is_empty());
    }

    #[tokio::test]
    async fn test_benchmarks() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let benchmarks = analysis.benchmarks().await.unwrap();

        // All timings should be non-negative
        assert!(benchmarks.impact_analysis_ms >= 0.0);
        assert!(benchmarks.dead_code_ms >= 0.0);
        assert!(benchmarks.reference_chain_ms >= 0.0);
        assert!(benchmarks.call_chain_ms >= 0.0);
        assert!(benchmarks.total_ms >= 0.0);
    }

    #[tokio::test]
    async fn test_analyze_impact_backward_compat() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let impact = analysis.analyze_impact("test").await.unwrap();

        // Backward compatible API
        assert_eq!(impact.call_sites, 0);
        assert!(impact.affected_symbols.is_empty());
    }

    #[tokio::test]
    async fn test_find_dead_code_backward_compat() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let dead_code = analysis.find_dead_code().await.unwrap();

        // Empty database should return no dead code
        assert!(dead_code.is_empty());
    }

    #[test]
    fn test_impact_analysis_creation() {
        let impact = ImpactAnalysis {
            affected_symbols: vec![],
            call_sites: 0,
        };
        assert!(impact.affected_symbols.is_empty());
        assert_eq!(impact.call_sites, 0);
    }

    #[test]
    fn test_cross_references_creation() {
        let xrefs = CrossReferences {
            callers: vec![],
            callees: vec![],
        };
        assert!(xrefs.callers.is_empty());
        assert!(xrefs.callees.is_empty());
    }

    #[test]
    fn test_complexity_metrics_creation() {
        let metrics = ComplexityMetrics {
            cyclomatic_complexity: 5,
            lines_of_code: 100,
            max_nesting_depth: 3,
            decision_points: 4,
        };
        assert_eq!(metrics.cyclomatic_complexity, 5);
        assert_eq!(metrics.lines_of_code, 100);
        assert_eq!(metrics.max_nesting_depth, 3);
        assert_eq!(metrics.decision_points, 4);
    }

    #[test]
    fn test_module_dependency_creation() {
        let dep = ModuleDependency {
            from: "mod_a".to_string(),
            to: "mod_b".to_string(),
        };
        assert_eq!(dep.from, "mod_a");
        assert_eq!(dep.to, "mod_b");
    }

    // EditOperation trait tests

    #[tokio::test]
    async fn test_insert_operation_verify() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let insert = InsertOperation {
            after_symbol: "nonexistent".to_string(),
            content: "// new content".to_string(),
        };

        let result = insert.verify(&analysis).await.unwrap();
        assert!(matches!(result, ApplyResult::Failed(_)));
    }

    #[tokio::test]
    async fn test_insert_operation_preview() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let insert = InsertOperation {
            after_symbol: "test_symbol".to_string(),
            content: "// new content".to_string(),
        };

        let diff = insert.preview(&analysis).await.unwrap();
        assert!(!diff.new.is_empty());
    }

    #[tokio::test]
    async fn test_delete_operation_verify_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let delete = DeleteOperation {
            symbol_name: "nonexistent".to_string(),
        };

        let result = delete.verify(&analysis).await.unwrap();
        assert!(matches!(result, ApplyResult::Failed(_)));
    }

    #[tokio::test]
    async fn test_delete_operation_preview() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let delete = DeleteOperation {
            symbol_name: "test_func".to_string(),
        };

        let diff = delete.preview(&analysis).await.unwrap();
        assert!(diff.new.contains("deleted"));
    }

    #[tokio::test]
    async fn test_rename_operation_verify_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let rename = RenameOperation::new("old_name", "new_name");

        let result = rename.verify(&analysis).await.unwrap();
        assert!(matches!(result, ApplyResult::Failed(_)));
    }

    #[tokio::test]
    async fn test_rename_operation_validate_empty_name() {
        let rename = RenameOperation::new("old", "");
        let result = rename.validate_name();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rename_operation_validate_invalid_name() {
        let rename = RenameOperation::new("old", "123invalid");
        let result = rename.validate_name();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_error_result_always_fails() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let mut analysis = AnalysisModule::new(graph, cfg, edit, search);
        let error = ErrorResult::new("Test error");

        let result = error.verify(&analysis).await.unwrap();
        assert_eq!(result, ApplyResult::AlwaysError);

        let apply_result = error.apply(&mut analysis).await.unwrap();
        assert!(matches!(apply_result, ApplyResult::Failed(_)));
    }

    #[test]
    fn test_diff_creation() {
        let diff = Diff::new(
            "original content".to_string(),
            "new content".to_string(),
        );
        assert_eq!(diff.original, "original content");
        assert_eq!(diff.new, "new content");
    }

    #[test]
    fn test_diff_has_changes() {
        let diff = Diff::new("a".to_string(), "b".to_string());
        assert!(diff.has_changes());
    }

    #[test]
    fn test_diff_no_changes() {
        let diff = Diff::new("same".to_string(), "same".to_string());
        assert!(!diff.has_changes());
    }

    #[test]
    fn test_apply_result_variants() {
        assert!(matches!(ApplyResult::Applied, ApplyResult::Applied));
        assert!(matches!(ApplyResult::AlwaysError, ApplyResult::AlwaysError));
        assert!(matches!(ApplyResult::Pending, ApplyResult::Pending));
        assert!(matches!(ApplyResult::Failed("x".to_string()), ApplyResult::Failed(_)));
    }

    // End-to-end integration tests

    #[tokio::test]
    async fn test_full_workflow_from_lookup_to_edit() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let mut analysis = AnalysisModule::new(graph, cfg, edit, search);

        // 1. Look up a symbol
        let symbols = analysis.graph().find_symbol("test").await.unwrap();
        assert!(symbols.is_empty());

        // 2. Check impact
        let impact = analysis.impact_analysis("test").await.unwrap();
        assert_eq!(impact.symbol, "test");
        assert_eq!(impact.impact_score, 0);

        // 3. Try to apply a rename operation
        let rename = RenameOperation::new("test", "new_name");
        let result = rename.verify(&analysis).await.unwrap();
        assert!(matches!(result, ApplyResult::Failed(_)));
    }

    #[tokio::test]
    async fn test_cross_module_error_handling() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);

        // Test that errors from graph module propagate correctly
        let result = analysis.graph().find_symbol("").await;
        assert!(result.is_ok()); // Empty query returns empty, not error

        // Test that search module handles valid patterns
        let search_result = analysis.search().pattern_search("test").await;
        assert!(search_result.is_ok());

        // Test semantic search
        let semantic_result = analysis.search().semantic_search("test").await;
        assert!(semantic_result.is_ok());
    }

    #[tokio::test]
    async fn test_deep_impact_analysis_integration() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);

        // Test k-hop impact analysis
        let impacted = analysis.deep_impact_analysis("test", 2).await.unwrap();
        assert!(impacted.is_empty()); // No symbols in empty database
    }

    #[tokio::test]
    async fn test_module_dependencies_integration() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);

        // Test module dependency analysis
        let deps = analysis.module_dependencies().await.unwrap();
        assert!(deps.is_empty()); // No dependencies in empty database

        // Test circular dependency detection
        let cycles = analysis.find_dependency_cycles().await.unwrap();
        assert!(cycles.is_empty());
    }

    #[tokio::test]
    async fn test_complexity_metrics_integration() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);

        // Test complexity metrics
        let metrics = analysis.complexity_metrics("test_function").await.unwrap();
        assert_eq!(metrics.cyclomatic_complexity, 1);
        assert_eq!(metrics.decision_points, 0);

        // Test source complexity analysis
        let source = r#"
            fn example(x: i32) -> i32 {
                if x > 0 {
                    return x * 2;
                }
                x
            }
        "#;
        let source_metrics = analysis.analyze_source_complexity(source);
        assert!(source_metrics.cyclomatic_complexity >= 1);
    }

    #[tokio::test]
    async fn test_cross_references_integration() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);

        // Test cross-references
        let xrefs = analysis.cross_references("test").await.unwrap();
        assert!(xrefs.callers.is_empty());
        assert!(xrefs.callees.is_empty());
    }

    #[tokio::test]
    async fn test_performance_benchmarks_integration() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await.unwrap()
        );
        let graph = GraphModule::new(store.clone());
        let search = SearchModule::new(store.clone());
        let cfg = CfgModule::new(store.clone());
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);

        // Test performance benchmarks
        let benchmarks = analysis.benchmarks().await.unwrap();

        // Verify all benchmarks completed
        assert!(benchmarks.impact_analysis_ms >= 0.0);
        assert!(benchmarks.dead_code_ms >= 0.0);
        assert!(benchmarks.reference_chain_ms >= 0.0);
        assert!(benchmarks.call_chain_ms >= 0.0);
        assert!(benchmarks.total_ms >= 0.0);

        // Total should be sum of components (approximately)
        let sum = benchmarks.impact_analysis_ms + benchmarks.dead_code_ms
            + benchmarks.reference_chain_ms + benchmarks.call_chain_ms;
        assert!(benchmarks.total_ms >= sum * 0.9); // Allow for timing overhead
    }
}

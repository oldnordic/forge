//! Analysis module - Combined analysis operations
//!
//! This module provides composite operations using graph, search, cfg, and edit modules.

use crate::cfg::CfgModule;
use crate::edit::EditModule;
use crate::error::Result;
use crate::graph::GraphModule;
use crate::search::SearchModule;
use crate::types::Symbol;
use std::sync::Arc;
use std::time::Instant;

pub mod complexity;
pub mod dead_code;
pub mod modules;
pub mod operations;

pub use complexity::{ComplexityMetrics, RiskLevel};
pub use dead_code::{DeadCodeAnalyzer, DeadSymbol};
pub use modules::{ModuleAnalyzer, ModuleDependencyGraph, ModuleInfo};
pub use operations::{
    DeleteOperation, Diff, EditOperation, ErrorResult, InsertOperation, RenameOperation,
};

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
        Self {
            graph,
            search,
            cfg,
            edit,
        }
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
        let callers = self.graph.callers_of(symbol).await.unwrap_or_default();

        // Get all references
        let refs = self.graph.references(symbol).await.unwrap_or_default();

        // Also search for the symbol to get its metadata
        let _symbol_info = self
            .graph
            .find_symbol(symbol)
            .await
            .unwrap_or_default()
            .into_iter()
            .next();

        // Compute impact score based on reference and call counts
        let ref_count = refs.len();
        let call_count = callers.len();
        let impact_score = ref_count + call_count * 2; // Calls weigh more

        tracing::debug!(
            "Impact analysis for '{}' completed in {:?}",
            symbol,
            start.elapsed()
        );

        Ok(ImpactData {
            symbol: symbol.to_string(),
            ref_count,
            call_count,
            referenced_by: callers
                .into_iter()
                .map(|r| {
                    let name: Arc<str> = Arc::from(r.from_name.unwrap_or_default());
                    Symbol {
                        id: r.from,
                        name: name.clone(),
                        fully_qualified_name: name,
                        kind: crate::types::SymbolKind::Function,
                        language: crate::types::Language::Unknown("unknown".to_string()),
                        location: r.location,
                        parent_id: None,
                        metadata: serde_json::Value::Null,
                    }
                })
                .collect(),
            references: refs
                .into_iter()
                .map(|r| {
                    let name: Arc<str> = Arc::from(r.to_name.unwrap_or_default());
                    Symbol {
                        id: r.to,
                        name: name.clone(),
                        fully_qualified_name: name,
                        kind: crate::types::SymbolKind::Function,
                        language: crate::types::Language::Unknown("unknown".to_string()),
                        location: r.location,
                        parent_id: None,
                        metadata: serde_json::Value::Null,
                    }
                })
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
            tracing::debug!(
                "No graph database found at {:?}, returning empty dead code list",
                db_path
            );
            return Ok(Vec::new());
        }

        let analyzer = dead_code::DeadCodeAnalyzer::new(&db_path);

        match analyzer.find_dead_code() {
            Ok(dead_symbols) => {
                let result: Vec<Symbol> = dead_symbols.into_iter().map(Into::into).collect();
                tracing::debug!(
                    "Dead code detection found {} symbols in {:?}",
                    result.len(),
                    start.elapsed()
                );
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
    pub async fn deep_impact_analysis(
        &self,
        symbol_name: &str,
        depth: u32,
    ) -> Result<Vec<crate::graph::ImpactedSymbol>> {
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

        let refs = self.graph.references(symbol).await?;

        let chain: Vec<Symbol> = refs
            .into_iter()
            .map(|r| {
                let name: Arc<str> = Arc::from(r.to_name.unwrap_or_default());
                Symbol {
                    id: r.from,
                    name: name.clone(),
                    fully_qualified_name: name,
                    kind: crate::types::SymbolKind::Function,
                    language: crate::types::Language::Unknown("unknown".to_string()),
                    location: r.location,
                    parent_id: None,
                    metadata: serde_json::Value::Null,
                }
            })
            .collect();

        tracing::debug!(
            "Reference chain for '{}' has {} symbols, found in {:?}",
            symbol,
            chain.len(),
            start.elapsed()
        );
        Ok(chain)
    }

    /// Trace all callers to a function.
    ///
    /// Returns an ordered list of calling symbols.
    pub async fn call_chain(&self, symbol: &str) -> Result<Vec<Symbol>> {
        let start = Instant::now();

        let callers = self.graph.callers_of(symbol).await?;

        let chain: Vec<Symbol> = callers
            .into_iter()
            .map(|r| {
                let name: Arc<str> = Arc::from(r.from_name.unwrap_or_default());
                Symbol {
                    id: r.from,
                    name: name.clone(),
                    fully_qualified_name: name,
                    kind: crate::types::SymbolKind::Function,
                    language: crate::types::Language::Unknown("unknown".to_string()),
                    location: r.location,
                    parent_id: None,
                    metadata: serde_json::Value::Null,
                }
            })
            .collect();

        tracing::debug!(
            "Call chain for '{}' has {} symbols, found in {:?}",
            symbol,
            chain.len(),
            start.elapsed()
        );
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
    /// Looks up the symbol's source code and analyzes it for complexity.
    pub async fn complexity_metrics(&self, symbol_name: &str) -> Result<ComplexityMetrics> {
        // Try to find the symbol's source and analyze it
        let symbols = self
            .graph
            .find_symbol(symbol_name)
            .await
            .unwrap_or_default();
        if let Some(sym) = symbols.first() {
            let full_path = self
                .graph
                .store()
                .codebase_path
                .join(&sym.location.file_path);
            if let Ok(content) = tokio::fs::read_to_string(&full_path).await {
                // Extract the function body from byte span
                let start = sym.location.byte_start as usize;
                let end = sym.location.byte_end as usize;
                if end <= content.len() {
                    let source = &content[start..end];
                    return Ok(self.analyze_source_complexity(source));
                }
            }
        }

        // Fallback: minimal metrics
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
        let caller_refs = self.graph.callers_of(symbol_name).await?;
        let callee_refs = self.graph.references(symbol_name).await?;

        let callers = caller_refs
            .iter()
            .map(|r| {
                let name: Arc<str> = Arc::from(r.from_name.clone().unwrap_or_default());
                Symbol {
                    id: r.from,
                    name: name.clone(),
                    fully_qualified_name: name,
                    kind: crate::types::SymbolKind::Function,
                    language: crate::types::Language::Unknown(String::new()),
                    location: r.location.clone(),
                    parent_id: None,
                    metadata: serde_json::Value::Null,
                }
            })
            .collect();

        let callees = callee_refs
            .iter()
            .map(|r| {
                let name: Arc<str> = Arc::from(r.to_name.clone().unwrap_or_default());
                Symbol {
                    id: r.to,
                    name: name.clone(),
                    fully_qualified_name: name,
                    kind: crate::types::SymbolKind::Function,
                    language: crate::types::Language::Unknown(String::new()),
                    location: r.location.clone(),
                    parent_id: None,
                    metadata: serde_json::Value::Null,
                }
            })
            .collect();

        Ok(CrossReferences { callers, callees })
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
                deps.push(ModuleDependency {
                    from: from.clone(),
                    to,
                });
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
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
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
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
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
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
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
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
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
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
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
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
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
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
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

    // End-to-end integration tests

    #[tokio::test]
    async fn test_cross_module_error_handling() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
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
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
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
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
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
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
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
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
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
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
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
        let sum = benchmarks.impact_analysis_ms
            + benchmarks.dead_code_ms
            + benchmarks.reference_chain_ms
            + benchmarks.call_chain_ms;
        assert!(benchmarks.total_ms >= sum * 0.9); // Allow for timing overhead
    }
}

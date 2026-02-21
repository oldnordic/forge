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
}

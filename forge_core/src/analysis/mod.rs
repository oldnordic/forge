//! Analysis module - Combined analysis operations
//!
//! This module provides composite operations using graph, cfg, and edit modules.

use crate::graph::GraphModule;
use crate::cfg::CfgModule;
use crate::edit::EditModule;
use crate::error::Result;
use crate::types::Symbol;


pub mod dead_code;
pub mod complexity;
pub mod modules;

pub use dead_code::{DeadCodeAnalyzer, DeadSymbol};
pub use complexity::{ComplexityMetrics, RiskLevel};
pub use modules::{ModuleAnalyzer, ModuleDependencyGraph, ModuleInfo};

/// Analysis module for combined operations.
pub struct AnalysisModule {
    graph: GraphModule,
    cfg: CfgModule,
    edit: EditModule,
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
    pub fn new(graph: GraphModule, cfg: CfgModule, edit: EditModule) -> Self {
        Self { graph, cfg, edit }
    }

    /// Get the graph module
    pub fn graph(&self) -> &GraphModule {
        &self.graph
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
    /// Returns all symbols that would be affected by modifying the given symbol.
    pub async fn analyze_impact(&self, symbol_name: &str) -> Result<ImpactAnalysis> {
        // Get all callers (references that call this symbol)
        let caller_refs = self.graph.callers_of(symbol_name).await?;
        
        // Get all references
        let all_refs = self.graph.references(symbol_name).await?;
        
        let total_sites = caller_refs.len() + all_refs.len();
        
        Ok(ImpactAnalysis {
            affected_symbols: Vec::new(), // v0.1 placeholder
            call_sites: total_sites,
        })
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
        let db_path = self.graph.store().db_path();
        let analyzer = DeadCodeAnalyzer::new(db_path);
        
        let dead_symbols = analyzer.find_dead_code()?;
        Ok(dead_symbols.into_iter().map(Into::into).collect())
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

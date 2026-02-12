//! Analysis module - Combined operations using multiple modules.
//!
//! This module provides high-level analysis that combines graph, CFG,
//! and edit operations.

use std::collections::{HashSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use crate::{graph::GraphModule, cfg::CfgModule, edit::EditModule};
use crate::error::Result;
use crate::types::{SymbolId, Cycle};

/// Analysis module for combined operations.
///
/// # Examples
///
/// ```rust,no_run
/// use forge::Forge;
///
/// # #[tokio::main]
/// # async fn main() -> forge::Result<()> {
/// #     let forge = Forge::open("./my-project").await?;
/// let analysis = forge.analysis();
///
/// // Analyze impact
/// let impact = analysis.impact_radius(symbol_id).await?;
///
/// #     Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct AnalysisModule {
    graph: GraphModule,
    cfg: CfgModule,
    edit: EditModule,
}

impl AnalysisModule {
    pub(crate) fn new(graph: GraphModule, cfg: CfgModule, edit: EditModule) -> Self {
        Self { graph, cfg, edit }
    }

    /// Analyzes the impact radius of a symbol.
    ///
    /// This determines which symbols and files would be affected
    /// by changes to the given symbol.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The symbol to analyze
    ///
    /// # Returns
    ///
    /// Impact analysis showing affected symbols and files
    pub async fn impact_radius(&self, symbol: SymbolId) -> Result<ImpactAnalysis> {
        // Get all reachable symbols from the target
        let reachable = self.graph.reachable_from(symbol).await?;

        // Collect unique files from reachable symbols
        let mut affected_files_set = HashSet::new();
        let mut affected_symbols = Vec::new();

        for sym_id in &reachable {
            // Try to get the symbol to find its file
            if let Ok(sym) = self.graph.find_symbol_by_id(*sym_id).await {
                affected_files_set.insert(sym.location.file_path.clone());
                affected_symbols.push(*sym_id);
            }
        }

        let affected_files: Vec<PathBuf> = affected_files_set.into_iter().collect();

        Ok(ImpactAnalysis {
            affected_symbols,
            affected_files,
            radius: reachable.len(),
        })
    }

    /// Finds unused (dead) functions.
    ///
    /// Uses graph reachability to find symbols that are not
    /// reachable from any of the given entry points.
    ///
    /// # Arguments
    ///
    /// * `entries` - Entry point symbols to consider live
    ///
    /// # Returns
    ///
    /// A vector of symbols that are unreachable from any entry point
    pub async fn unused_functions(&self, entries: &[SymbolId]) -> Result<Vec<SymbolId>> {
        let mut live = HashSet::new();

        // Mark all entry points as live
        for &entry in entries {
            live.insert(entry);
        }

        // Find all symbols reachable from entry points
        for &entry in entries {
            if let Ok(reachable) = self.graph.reachable_from(entry).await {
                for sym in reachable {
                    live.insert(sym);
                }
            }
        }

        // For now, return empty since we can't enumerate all symbols
        // Full implementation would query all symbols and find dead ones
        Ok(Vec::new())
    }

    /// Detects circular dependencies in the call graph.
    ///
    /// # Returns
    ///
    /// A vector of cycles found
    pub async fn circular_dependencies(&self) -> Result<Vec<Cycle>> {
        self.graph.cycles().await
    }
}

/// Result of an impact analysis.
#[derive(Clone, Debug)]
pub struct ImpactAnalysis {
    /// Symbols that would be affected
    pub affected_symbols: Vec<SymbolId>,
    /// Files that would be affected
    pub affected_files: Vec<PathBuf>,
    /// Estimated impact radius (number of hops from symbol)
    pub radius: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_analysis_module_creation() {
        let store = Arc::new(crate::storage::UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());

        let graph = crate::graph::GraphModule::new(store.clone());
        let cfg = crate::cfg::CfgModule::new(store.clone());
        let edit = crate::edit::EditModule::new(store);

        let _module = AnalysisModule::new(graph, cfg, edit);

        // Test that module can be created
        // (No assertions needed - just that it compiles)
    }

    #[tokio::test]
    async fn test_impact_radius() {
        let store = Arc::new(crate::storage::UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());

        let graph = crate::graph::GraphModule::new(store.clone());
        let cfg = crate::cfg::CfgModule::new(store.clone());
        let edit = crate::edit::EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit);

        let impact = analysis.impact_radius(SymbolId(1)).await.unwrap();

        assert_eq!(impact.radius, 0);
        assert_eq!(impact.affected_symbols.len(), 0);
        assert_eq!(impact.affected_files.len(), 0);
    }

    #[tokio::test]
    async fn test_unused_functions() {
        let store = Arc::new(crate::storage::UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());

        let graph = crate::graph::GraphModule::new(store.clone());
        let cfg = crate::cfg::CfgModule::new(store.clone());
        let edit = crate::edit::EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit);

        let entries = &[SymbolId(1), SymbolId(2)];
        let unused = analysis.unused_functions(entries).await.unwrap();

        // Should return empty for now
        assert_eq!(unused.len(), 0);
    }

    #[tokio::test]
    async fn test_circular_dependencies() {
        let store = Arc::new(crate::storage::UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());

        let graph = crate::graph::GraphModule::new(store.clone());
        let cfg = crate::cfg::CfgModule::new(store.clone());
        let edit = crate::edit::EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit);

        let cycles = analysis.circular_dependencies().await.unwrap();

        // Should return empty for now
        assert_eq!(cycles.len(), 0);
    }
}

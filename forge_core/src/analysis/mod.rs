//! Analysis module - Combined operations using multiple modules.
//!
//! This module provides high-level analysis that combines graph, CFG,
//! and edit operations.

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
    /// * `_symbol` - The symbol to analyze
    ///
    /// # Returns
    ///
    /// Impact analysis showing affected symbols and files
    pub async fn impact_radius(&self, _symbol: SymbolId) -> Result<ImpactAnalysis> {
        // TODO: Implement using graph reachability
        Err(crate::error::ForgeError::BackendNotAvailable(
            "Impact analysis not yet implemented".to_string()
        ))
    }

    /// Finds unused (dead) functions.
    ///
    /// # Arguments
    ///
    /// * `_entries` - Entry point symbols to consider live
    ///
    /// # Returns
    ///
    /// A vector of symbols that are unreachable from any entry point
    pub async fn unused_functions(&self, _entries: &[SymbolId]) -> Result<Vec<SymbolId>> {
        // TODO: Implement using graph dead code detection
        Err(crate::error::ForgeError::BackendNotAvailable(
            "Unused function detection not yet implemented".to_string()
        ))
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
    pub affected_files: Vec<std::path::PathBuf>,
    /// Estimated impact radius (number of hops from the symbol)
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

        let module = AnalysisModule::new(graph, cfg, edit);

        // Test that module can be created
        // (No assertions needed - just that it compiles)
    }
}

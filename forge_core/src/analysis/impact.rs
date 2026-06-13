//! Impact-analysis data types.
//!
//! Extracted from `mod.rs` (SPLIT-27). Pure data structs consumed by
//! `AnalysisModule` impact/cross-reference/call-chain methods.

use crate::types::Symbol;

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

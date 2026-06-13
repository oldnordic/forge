#[derive(Clone, Debug, PartialEq)]
pub struct PlanStep {
    pub description: String,
    pub operation: PlanOperation,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlanOperation {
    Rename {
        old: String,
        new: String,
        file: Option<String>,
    },
    Delete {
        name: String,
        file: Option<String>,
    },
    Create {
        path: String,
        content: String,
    },
    Inspect {
        symbol_id: forge_core::types::SymbolId,
        symbol_name: String,
    },
    Modify {
        file: String,
        start: usize,
        end: usize,
        replacement: String,
    },
}

#[derive(Clone, Debug)]
pub struct ImpactEstimate {
    pub affected_files: Vec<String>,
    pub complexity: usize,
}

#[derive(Clone, Debug)]
pub struct Conflict {
    pub file: String,
    pub reason: ConflictReason,
}

#[derive(Clone, Debug)]
pub enum ConflictReason {
    OverlappingRegion { start: usize, end: usize },
}

#[derive(Clone, Debug)]
pub struct RollbackStep {
    pub description: String,
    pub operation: RollbackOperation,
}

#[derive(Clone, Debug)]
pub enum RollbackOperation {
    Rename { new_name: String },
    Restore { name: String },
    Delete { path: String },
    None,
}

#[derive(Clone, Debug)]
pub(crate) enum PlanIntent {
    Rename { new_name: String },
    Delete,
    Create { content: String },
    Inspect,
}

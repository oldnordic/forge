use crate::workflow::task::{
    CompensationAction, TaskContext, TaskError, TaskId, TaskResult, WorkflowTask,
};
use serde::{Deserialize, Serialize};

/// Types of graph queries supported by GraphQueryTask.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GraphQueryType {
    /// Find a symbol by name
    FindSymbol,
    /// Find references to a symbol
    References,
    /// Analyze impact of changes to a symbol
    ImpactAnalysis,
}

/// Task that executes graph queries using the Forge SDK.
///
/// Queries the code graph for symbols, references, or impact analysis.
pub struct GraphQueryTask {
    pub(super) id: TaskId,
    name: String,
    pub(super) query_type: GraphQueryType,
    pub(super) _target: String,
}

impl GraphQueryTask {
    /// Creates a new GraphQueryTask for finding a symbol.
    pub fn find_symbol(target: impl Into<String>) -> Self {
        Self::new(GraphQueryType::FindSymbol, target)
    }

    /// Creates a new GraphQueryTask for finding references.
    pub fn references(target: impl Into<String>) -> Self {
        Self::new(GraphQueryType::References, target)
    }

    /// Creates a new GraphQueryTask for impact analysis.
    pub fn impact_analysis(target: impl Into<String>) -> Self {
        Self::new(GraphQueryType::ImpactAnalysis, target)
    }

    fn new(query_type: GraphQueryType, target: impl Into<String>) -> Self {
        let target_str = target.into();
        Self {
            id: TaskId::new(format!("graph_query_{:?}", query_type)),
            name: format!("Graph Query: {:?}", query_type),
            query_type,
            _target: target_str,
        }
    }

    /// Gets the query target symbol name.
    pub fn target(&self) -> &str {
        &self._target
    }

    /// Creates a GraphQueryTask with a custom ID.
    pub fn with_id(id: TaskId, query_type: GraphQueryType, target: impl Into<String>) -> Self {
        Self {
            id,
            name: format!("Graph Query: {:?}", query_type),
            query_type,
            _target: target.into(),
        }
    }
}

#[async_trait::async_trait]
impl WorkflowTask for GraphQueryTask {
    async fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
        let forge = context.forge.as_ref().ok_or_else(|| {
            TaskError::ExecutionFailed("Forge SDK not available in TaskContext".to_string())
        })?;

        match self.query_type {
            GraphQueryType::FindSymbol => {
                let symbols = forge
                    .graph()
                    .find_symbol(&self._target)
                    .await
                    .map_err(|e| {
                        TaskError::ExecutionFailed(format!("Find symbol failed: {}", e))
                    })?;
                if symbols.is_empty() {
                    Ok(TaskResult::Failed(format!(
                        "Symbol '{}' not found",
                        self._target
                    )))
                } else {
                    Ok(TaskResult::Success)
                }
            }
            GraphQueryType::References => {
                let refs = forge.graph().references(&self._target).await.map_err(|e| {
                    TaskError::ExecutionFailed(format!("References query failed: {}", e))
                })?;
                Ok(if refs.is_empty() {
                    TaskResult::Failed(format!("No references found for '{}'", self._target))
                } else {
                    TaskResult::Success
                })
            }
            GraphQueryType::ImpactAnalysis => {
                let result = forge
                    .analysis()
                    .impact_analysis(&self._target)
                    .await
                    .map_err(|e| {
                        TaskError::ExecutionFailed(format!("Impact analysis failed: {}", e))
                    })?;
                Ok(
                    if result.referenced_by.is_empty() && result.references.is_empty() {
                        TaskResult::Failed(format!("No impact found for '{}'", self._target))
                    } else {
                        TaskResult::Success
                    },
                )
            }
        }
    }

    fn id(&self) -> TaskId {
        self.id.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn compensation(&self) -> Option<CompensationAction> {
        Some(CompensationAction::skip(
            "Read-only graph query - no undo needed",
        ))
    }
}

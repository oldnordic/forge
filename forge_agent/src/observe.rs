//! Observation phase - Graph-based context gathering.
//!
//! This module implements the observation phase of the agent loop, gathering
//! relevant context from the code graph to inform intelligent operations.

// AgentError is used via the `?` operator and `From` trait
use crate::{AgentError, Result};
use forge_core::{
    types::{Symbol, SymbolId, SymbolKind},
    Forge,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Observer for gathering context from the code graph.
///
/// The Observer uses the Forge SDK to query symbols, references, and control
/// flow information relevant to a given query.
///
/// # Examples
///
/// ```no_run
/// use forge_agent::observe::Observer;
/// use forge_core::Forge;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let forge = Forge::open("./my-project").await?;
/// let observer = Observer::new(forge);
///
/// let observation = observer.gather("find functions that call parse").await?;
/// println!("Found {} relevant symbols", observation.symbols.len());
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Observer {
    /// The Forge SDK instance for graph queries
    forge: Arc<Forge>,
    /// Cache for observation results (query -> observation)
    cache: Arc<tokio::sync::RwLock<HashMap<String, Observation>>>,
}

impl Observer {
    /// Creates a new Observer with the given Forge instance.
    ///
    /// # Arguments
    ///
    /// * `forge` - The Forge SDK instance
    pub fn new(forge: Forge) -> Self {
        Self {
            forge: Arc::new(forge),
            cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Gathers observation data for a natural language query.
    ///
    /// This method parses the query to extract intent, then performs
    /// graph queries to gather relevant symbols, references, and CFG data.
    ///
    /// # Arguments
    ///
    /// * `query` - Natural language query describing what to observe
    ///
    /// # Returns
    ///
    /// An `Observation` containing relevant context from the graph.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use forge_agent::observe::Observer;
    /// # use forge_core::Forge;
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// # let forge = Forge::open("./").await?;
    /// # let observer = Observer::new(forge);
    /// let observation = observer.gather("find functions named process").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn gather(&self, query: &str) -> Result<Observation> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(query) {
                return Ok(cached.clone());
            }
        }

        // Parse the query to extract intent
        let parsed_query = self.parse_query(query)?;

        // Gather symbols based on the parsed query
        let symbols = self.gather_symbols(&parsed_query).await?;

        // Gather references for the found symbols
        let references = self.gather_references(&symbols).await?;

        // Gather CFG data for functions
        let cfg_data = self.gather_cfg(&symbols).await?;

        let observation = Observation {
            query: query.to_string(),
            symbols,
            references,
            cfg_data,
        };

        // Cache the result
        {
            let mut cache = self.cache.write().await;
            cache.insert(query.to_string(), observation.clone());
        }

        Ok(observation)
    }

    /// Parses a natural language query into structured intent.
    ///
    /// This is a simple heuristic-based parser. For production, this would
    /// use LLM integration for true semantic understanding.
    ///
    /// # Arguments
    ///
    /// * `query` - The natural language query
    fn parse_query(&self, query: &str) -> Result<ParsedQuery> {
        let query_lower = query.to_lowercase();

        // Detect query type
        let query_type = if query_lower.contains("functions that call") {
            QueryType::FunctionsCalling
        } else if query_lower.contains("functions called by") {
            QueryType::FunctionsCalledBy
        } else if query_lower.contains("find") && query_lower.contains("named") {
            QueryType::FindByName
        } else if query_lower.contains("all functions") {
            QueryType::AllFunctions
        } else if query_lower.contains("all structs") {
            QueryType::AllStructs
        } else {
            QueryType::SemanticSearch
        };

        // Extract filters
        let mut filters = QueryFilters::default();
        if query_lower.contains("in ") || query_lower.contains("from ") {
            // Simple path extraction (would be more sophisticated in production)
            if let Some(path_start) = query_lower.find("in ") {
                let remaining = &query_lower[path_start + 3..];
                filters.file_path = remaining.split_whitespace().next().map(String::from);
            } else if let Some(path_start) = query_lower.find("from ") {
                let remaining = &query_lower[path_start + 5..];
                filters.file_path = remaining.split_whitespace().next().map(String::from);
            }
        }

        let target_name = self.extract_target_name(query, &query_type)?;

        Ok(ParsedQuery {
            original: query.to_string(),
            query_type,
            target_name,
            filters,
        })
    }

    /// Extracts the target name from a query.
    fn extract_target_name(&self, query: &str, query_type: &QueryType) -> Result<Option<String>> {
        match query_type {
            QueryType::FunctionsCalling | QueryType::FunctionsCalledBy => {
                // Look for a name pattern after keywords
                let keywords = ["call", "calling", "called by"];
                for keyword in keywords {
                    if let Some(pos) = query.to_lowercase().find(keyword) {
                        let remaining = &query[pos + keyword.len()..];
                        let name = remaining.trim().trim_end_matches('?').trim().to_string();
                        if !name.is_empty() {
                            return Ok(Some(name));
                        }
                    }
                }
                Ok(None)
            }
            QueryType::FindByName => {
                // Extract name after "named"
                if let Some(pos) = query.to_lowercase().find("named") {
                    let remaining = &query[pos + 6..];
                    let name = remaining.trim().trim_end_matches('?').trim().to_string();
                    if !name.is_empty() {
                        return Ok(Some(name));
                    }
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Gathers symbols matching the parsed query.
    async fn gather_symbols(&self, query: &ParsedQuery) -> Result<Vec<ObservedSymbol>> {
        let graph = self.forge.graph();
        let search = self.forge.search();

        let mut symbols = Vec::new();

        match &query.query_type {
            QueryType::FindByName => {
                if let Some(name) = &query.target_name {
                    // Find symbols by exact name
                    let found = graph.find_symbol(name).await?;
                    for symbol in found {
                        symbols.push(ObservedSymbol::from_symbol(symbol)?);
                    }
                }
            }
            QueryType::AllFunctions => {
                // Use pattern search (empty name with kind filter)
                // Note: semantic search returns empty for v0.1
                let results = search.pattern("").await?;
                for symbol in results {
                    if symbol.kind == SymbolKind::Function {
                        symbols.push(ObservedSymbol::from_symbol(symbol.clone())?);
                    }
                }
            }
            QueryType::AllStructs => {
                let results = search.pattern("").await?;
                for symbol in results {
                    if symbol.kind == SymbolKind::Struct {
                        symbols.push(ObservedSymbol::from_symbol(symbol.clone())?);
                    }
                }
            }
            QueryType::FunctionsCalling => {
                // Find functions that call a specific symbol
                if let Some(target_name) = &query.target_name {
                    // Find what calls this symbol (callers_of returns Reference items)
                    let callers = graph.callers_of(target_name).await?;
                    for caller_ref in callers {
                        // Get the calling symbol
                        if let Ok(caller_symbol) = graph.find_symbol_by_id(caller_ref.from).await {
                            symbols.push(ObservedSymbol::from_symbol(caller_symbol)?);
                        }
                    }
                }
            }
            QueryType::FunctionsCalledBy => {
                // Find functions called by a specific symbol
                if let Some(target_name) = &query.target_name {
                    let refs = graph.references(target_name).await?;
                    for reference in refs {
                        if let Ok(symbol) = graph.find_symbol_by_id(reference.to).await {
                            symbols.push(ObservedSymbol::from_symbol(symbol)?);
                        }
                    }
                }
            }
            QueryType::SemanticSearch => {
                // Use the full query for semantic search
                let results = search.pattern(&query.original).await?;
                for symbol in results {
                    symbols.push(ObservedSymbol::from_symbol(symbol.clone())?);
                }
            }
        }

        // Apply file filter if specified
        if let Some(file_path) = &query.filters.file_path {
            symbols.retain(|s| {
                s.location
                    .file_path
                    .to_string_lossy()
                    .contains(file_path)
            });
        }

        Ok(symbols)
    }

    /// Gathers reference information for the given symbols.
    async fn gather_references(&self, symbols: &[ObservedSymbol]) -> Result<Vec<ObservedReference>> {
        let graph = self.forge.graph();
        let mut references = Vec::new();

        for symbol in symbols {
            // Get incoming references (what calls this)
            let symbol_name = &symbol.name;
            let callers = graph.callers_of(symbol_name).await?;
            for caller_ref in callers {
                references.push(ObservedReference {
                    from: caller_ref.from,
                    to: symbol.id,
                    kind: "call".to_string(),
                });
            }

            // Get outgoing references (what this calls)
            let refs = graph.references(symbol_name).await?;
            for reference in refs {
                references.push(ObservedReference {
                    from: symbol.id,
                    to: reference.to,
                    kind: format!("{:?}", reference.kind),
                });
            }
        }

        Ok(references)
    }

    /// Gathers CFG data for functions in the given symbols.
    async fn gather_cfg(&self, symbols: &[ObservedSymbol]) -> Result<Vec<CfgInfo>> {
        let cfg = self.forge.cfg();
        let mut cfg_infos = Vec::new();

        for symbol in symbols {
            if symbol.kind == SymbolKind::Function {
                // Try to get path information
                let paths = cfg.paths(symbol.id).execute().await?;
                let path_count = paths.len();
                let complexity = self.calculate_complexity(&paths);

                cfg_infos.push(CfgInfo {
                    symbol_id: symbol.id,
                    path_count,
                    complexity,
                });
            }
        }

        Ok(cfg_infos)
    }

    /// Calculates cyclomatic complexity from CFG paths.
    fn calculate_complexity(&self, paths: &[forge_core::cfg::Path]) -> usize {
        // Cyclomatic complexity = number of paths (simplified)
        // More accurate: E - N + 2*P where E=edges, N=nodes, P=components
        std::cmp::max(1, paths.len())
    }

    /// Clears the observation cache.
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

/// Parsed query representation.
#[derive(Debug, Clone)]
struct ParsedQuery {
    /// Original query string
    original: String,
    /// Type of query
    query_type: QueryType,
    /// Target name if specified
    target_name: Option<String>,
    /// Query filters
    filters: QueryFilters,
}

/// Type of query extracted from natural language.
#[derive(Debug, Clone, PartialEq, Eq)]
enum QueryType {
    /// Find functions that call a specific symbol
    FunctionsCalling,
    /// Find functions called by a specific symbol
    FunctionsCalledBy,
    /// Find symbols by exact name
    FindByName,
    /// Find all functions
    AllFunctions,
    /// Find all structs
    AllStructs,
    /// Semantic search using the query pattern
    SemanticSearch,
}

/// Filters extracted from query.
#[derive(Debug, Clone, Default)]
struct QueryFilters {
    /// File path filter
    file_path: Option<String>,
}

/// Result of the observation phase.
///
/// Contains all relevant context gathered from the code graph.
#[derive(Clone, Debug)]
pub struct Observation {
    /// The original query
    pub query: String,
    /// Relevant symbols found
    pub symbols: Vec<ObservedSymbol>,
    /// References between symbols
    pub references: Vec<ObservedReference>,
    /// CFG information for functions
    pub cfg_data: Vec<CfgInfo>,
}

/// A symbol observed during the observation phase.
#[derive(Clone, Debug)]
pub struct ObservedSymbol {
    /// Unique symbol identifier
    pub id: SymbolId,
    /// Symbol name
    pub name: String,
    /// Kind of symbol
    pub kind: SymbolKind,
    /// Source location
    pub location: forge_core::types::Location,
}

impl ObservedSymbol {
    /// Creates an ObservedSymbol from a Symbol.
    fn from_symbol(symbol: Symbol) -> Result<Self> {
        Ok(Self {
            id: symbol.id,
            name: symbol.name,
            kind: symbol.kind,
            location: symbol.location,
        })
    }
}

/// A reference observed between symbols.
#[derive(Clone, Debug)]
pub struct ObservedReference {
    /// Source symbol
    pub from: SymbolId,
    /// Target symbol
    pub to: SymbolId,
    /// Kind of reference
    pub kind: String,
}

/// CFG information for a symbol.
#[derive(Clone, Debug)]
pub struct CfgInfo {
    /// Symbol this CFG is for
    pub symbol_id: SymbolId,
    /// Number of execution paths
    pub path_count: usize,
    /// Estimated cyclomatic complexity
    pub complexity: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_observer() -> (Observer, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let observer = Observer::new(forge);
        (observer, temp_dir)
    }

    #[tokio::test]
    async fn test_observer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let observer = Observer::new(forge);

        // Observer should have empty cache
        let cache = observer.cache.read().await;
        assert!(cache.is_empty());
    }

    #[tokio::test]
    async fn test_parse_query_find_by_name() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let observer = Observer::new(forge);

        let query = observer.parse_query("find functions named process").unwrap();
        assert_eq!(query.query_type, QueryType::FindByName);
        assert_eq!(query.target_name, Some("process".to_string()));
    }

    #[tokio::test]
    async fn test_parse_query_functions_calling() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let observer = Observer::new(forge);

        let query = observer.parse_query("find functions that call parse").unwrap();
        assert_eq!(query.query_type, QueryType::FunctionsCalling);
        assert_eq!(query.target_name, Some("parse".to_string()));
    }

    #[tokio::test]
    async fn test_parse_query_all_functions() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let observer = Observer::new(forge);

        let query = observer.parse_query("show all functions").unwrap();
        assert_eq!(query.query_type, QueryType::AllFunctions);
    }

    #[tokio::test]
    async fn test_observation_caching() {
        let (observer, _temp_dir) = create_test_observer().await;

        // First call should not find anything in empty DB
        let result1 = observer.gather("test query").await;
        assert!(result1.is_ok());

        // Second call should hit cache
        let result2 = observer.gather("test query").await;
        assert!(result2.is_ok());

        // Results should be identical
        assert_eq!(result1.unwrap().query, result2.unwrap().query);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let (observer, _temp_dir) = create_test_observer().await;

        // Add something to cache
        let _ = observer.gather("test query").await;

        // Clear cache
        observer.clear_cache().await;

        // Cache should be empty
        let cache = observer.cache.read().await;
        assert!(cache.is_empty());
    }
}

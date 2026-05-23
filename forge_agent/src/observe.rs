//! Observation phase - Graph-based context gathering.
//!
//! This module implements the observation phase of the agent loop, gathering
//! relevant context from the code graph to inform intelligent operations.

use crate::Result;
use forge_core::{types::SymbolId, Forge};
use std::collections::HashMap;
use std::sync::Arc;

/// Trait for querying knowledge from a backend (e.g., atheneum).
#[async_trait::async_trait]
pub trait KnowledgeSource: Send + Sync {
    /// Queries knowledge for a target. Returns discovery metadata as JSON.
    async fn query(&self, target: &str) -> Option<Vec<serde_json::Value>>;
}

/// Observer for gathering context from a code graph.
///
/// The Observer uses Forge SDK to query symbols and references.
/// When an LLM provider is available, it enhances natural language
/// query parsing to extract symbol names and search terms.
/// When a knowledge source is available, it queries cached discoveries
/// before doing expensive graph searches.
#[derive(Clone)]
pub struct Observer {
    /// The Forge SDK instance for graph queries
    forge: Arc<Forge>,
    /// Cache for observation results (query -> observation)
    cache: Arc<tokio::sync::RwLock<HashMap<String, Observation>>>,
    /// Optional LLM provider for enhanced query parsing
    llm: Option<Arc<dyn crate::llm::LlmProvider>>,
    /// Optional knowledge source for cached discoveries
    knowledge_source: Option<Arc<dyn KnowledgeSource>>,
    /// Codebase context prefix injected into LLM prompts
    pub(crate) context_prefix: Option<String>,
}

impl Observer {
    /// Creates a new Observer with given Forge instance.
    pub fn new(forge: Forge) -> Self {
        Self {
            forge: Arc::new(forge),
            cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            llm: None,
            knowledge_source: None,
            context_prefix: None,
        }
    }

    /// Sets the LLM provider for enhanced query parsing.
    pub fn with_llm(mut self, provider: Arc<dyn crate::llm::LlmProvider>) -> Self {
        self.llm = Some(provider);
        self
    }

    /// Sets the knowledge source for cached discovery queries.
    pub fn with_knowledge_source(mut self, source: Arc<dyn KnowledgeSource>) -> Self {
        self.knowledge_source = Some(source);
        self
    }

    /// Sets the codebase context prefix injected into LLM prompts.
    pub fn with_context(mut self, ctx: &crate::context::AgentContext) -> Self {
        self.context_prefix = Some(ctx.context_prefix());
        self
    }

    /// Gathers observation data for a natural language query.
    pub async fn gather(&self, query: &str) -> Result<Observation> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(query) {
                return Ok(cached.clone());
            }
        }

        // For now, gather symbols using the graph API
        let symbols = self.gather_symbols(query).await?;

        let summary = self.summarize(query, &symbols).await;
        let observation = crate::Observation {
            query: query.to_string(),
            symbols,
            summary,
        };

        // Cache the result
        {
            let mut cache = self.cache.write().await;
            cache.insert(query.to_string(), observation.clone());
        }

        Ok(observation)
    }

    /// Gathers symbols by querying the search module and graph.
    async fn gather_symbols(&self, query: &str) -> Result<Vec<ObservedSymbol>> {
        let forge = self.forge.as_ref();
        let mut seen = std::collections::HashSet::new();
        let mut symbols = Vec::new();

        // Step 0: Query knowledge source for cached discoveries
        if let Some(ref source) = self.knowledge_source {
            if let Some(discoveries) = source.query(query).await {
                for disc in &discoveries {
                    if let Some(name) = disc.get("target").and_then(|v| v.as_str()) {
                        if let Ok(found) = forge.graph().find_symbol(name).await {
                            for sym in found {
                                if seen.insert(sym.id) {
                                    symbols.push(ObservedSymbol {
                                        id: sym.id,
                                        name: sym.name.to_string(),
                                        kind: sym.kind,
                                        location: sym.location,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Step 1: Semantic search via llmgrep/file scanning
        if let Ok(results) = forge.search().semantic_search(query).await {
            for sym in results {
                if seen.insert(sym.id) {
                    symbols.push(ObservedSymbol {
                        id: sym.id,
                        name: sym.name.to_string(),
                        kind: sym.kind,
                        location: sym.location,
                    });
                }
            }
        }

        // Step 2: Exact name lookup if query contains a specific name
        let query_lower = query.to_lowercase();
        if let Some(name) = extract_name_from_query(&query_lower) {
            if !name.is_empty() {
                if let Ok(found) = forge.graph().find_symbol(&name).await {
                    for sym in found {
                        if seen.insert(sym.id) {
                            symbols.push(ObservedSymbol {
                                id: sym.id,
                                name: sym.name.to_string(),
                                kind: sym.kind,
                                location: sym.location,
                            });
                        }
                    }
                }
            }
        } else if let Some(ref llm) = self.llm {
            // Step 3: LLM-enhanced parsing for natural language queries
            match llm_parse_query(llm.as_ref(), query).await {
                Ok(search_terms) => {
                    for term in search_terms {
                        if let Ok(found) = forge.graph().find_symbol(&term).await {
                            for sym in found {
                                if seen.insert(sym.id) {
                                    symbols.push(ObservedSymbol {
                                        id: sym.id,
                                        name: sym.name.to_string(),
                                        kind: sym.kind,
                                        location: sym.location,
                                    });
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("LLM query parsing failed, skipping LLM discovery: {e}");
                }
            }
        }

        Ok(symbols)
    }

    /// Clears the observation cache.
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Summarize gathered symbols using LLM. Returns None if no LLM configured.
    async fn summarize(&self, query: &str, symbols: &[ObservedSymbol]) -> Option<String> {
        let llm = self.llm.as_ref()?;

        let symbol_info: Vec<String> = symbols
            .iter()
            .map(|s| format!("{} ({:?}) at {:?}", s.name, s.kind, s.location))
            .collect();
        let symbol_list = symbol_info.join("\n");

        let prefix = self
            .context_prefix
            .as_deref()
            .map(|p| format!("{}\n", p))
            .unwrap_or_default();
        let prompt = format!(
            "{}Query: {}\n\nSymbols found:\n{}\n\nSummarize what these symbols reveal about the query.",
            prefix, query, symbol_list
        );

        let system = "You are a code intelligence assistant. Given a query and a list of symbols found, summarize what's relevant to the query in 2-3 sentences. Focus on: what the symbols do, how they relate, and what context is important.";

        match llm.complete(&prompt, Some(system)).await {
            Ok(summary) => Some(summary),
            Err(e) => {
                tracing::warn!("LLM summarization failed: {e}");
                None
            }
        }
    }
}

/// Result of the observation phase.
///
/// Contains relevant context gathered from the code graph.
#[derive(Clone, Debug)]
pub struct Observation {
    /// The original Query
    pub query: String,
    /// Relevant symbols found
    pub symbols: Vec<ObservedSymbol>,
    /// LLM-generated context summary (None if no LLM configured)
    pub summary: Option<String>,
}

/// A symbol observed during the observation phase.
#[derive(Clone, Debug)]
pub struct ObservedSymbol {
    /// Unique symbol identifier
    pub id: SymbolId,
    /// Symbol name
    pub name: String,
    /// Kind of symbol
    pub kind: forge_core::types::SymbolKind,
    /// Source location
    pub location: forge_core::types::Location,
}

/// Extracts a symbol name from a structured query.
fn extract_name_from_query(query: &str) -> Option<String> {
    // "rename X to Y" -> X
    if let Some(rest) = query.strip_prefix("rename ") {
        if let Some((name, _)) = rest.split_once(" to ") {
            return Some(name.trim().to_string());
        }
    }
    // "delete X" or "remove X" -> X
    if let Some(rest) = query
        .strip_prefix("delete ")
        .or_else(|| query.strip_prefix("remove "))
    {
        return Some(rest.trim().to_string());
    }
    // "find named X" or "find X" -> X
    if let Some(rest) = query.strip_prefix("find named ") {
        return Some(rest.trim().trim_end_matches('?').trim().to_string());
    }
    if let Some(rest) = query.strip_prefix("find ") {
        return Some(rest.trim().trim_end_matches('?').trim().to_string());
    }
    None
}

/// Uses LLM to extract search terms from a natural language query.
async fn llm_parse_query(llm: &dyn crate::llm::LlmProvider, query: &str) -> Result<Vec<String>> {
    let system = "You are a code search assistant. Given a natural language query about code, extract the most likely symbol names or search terms. Respond with ONLY a comma-separated list of names, no explanation. If the query is too vague, respond with 'none'.";
    let response = llm.complete(query, Some(system)).await.map_err(|e| {
        crate::AgentError::ObservationFailed(format!("LLM query parsing failed: {}", e))
    })?;

    let trimmed = response.trim().to_lowercase();
    if trimmed == "none" || trimmed.is_empty() {
        return Ok(vec![]);
    }

    let terms: Vec<String> = trimmed
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Ok(terms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::Forge;
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

    #[test]
    fn test_extract_name_from_query_structured_queries() {
        assert_eq!(
            extract_name_from_query("find named my_func"),
            Some("my_func".to_string())
        );
        assert_eq!(
            extract_name_from_query("rename old_name to new_name"),
            Some("old_name".to_string())
        );
        assert_eq!(
            extract_name_from_query("delete unused_helper"),
            Some("unused_helper".to_string())
        );
        assert_eq!(
            extract_name_from_query("remove dead_code"),
            Some("dead_code".to_string())
        );
        assert_eq!(
            extract_name_from_query("find MyStruct?"),
            Some("MyStruct".to_string())
        );
    }

    #[test]
    fn test_extract_name_from_query_natural_language() {
        // These return None — LLM enhancement would help here
        assert_eq!(
            extract_name_from_query("where is the auth middleware?"),
            None
        );
        assert_eq!(
            extract_name_from_query("show me the database connection handler"),
            None
        );
    }

    #[tokio::test]
    async fn test_llm_parse_query_extracts_terms() {
        use crate::llm::MockProvider;

        // MockProvider returns the prompt as-is with "mock: " prefix for complete_messages
        // But for complete(), it returns the canned response
        let mock = MockProvider::new("auth_middleware, validate_token");
        let terms = llm_parse_query(&mock, "where is the auth middleware?")
            .await
            .unwrap();
        assert_eq!(terms, vec!["auth_middleware", "validate_token"]);
    }

    #[tokio::test]
    async fn test_llm_parse_query_returns_empty_for_none() {
        use crate::llm::MockProvider;

        let mock = MockProvider::new("none");
        let terms = llm_parse_query(&mock, "what does this codebase do?")
            .await
            .unwrap();
        assert!(terms.is_empty());
    }

    #[tokio::test]
    async fn test_observer_with_llm_provider() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mock = Arc::new(crate::llm::MockProvider::new("handler"));

        let observer = Observer::new(forge).with_llm(mock);
        assert!(observer.llm.is_some());
    }

    /// Mock knowledge source for testing.
    struct MockKnowledgeSource {
        queries: std::sync::Mutex<Vec<String>>,
        response: Option<Vec<serde_json::Value>>,
    }

    impl MockKnowledgeSource {
        fn new(response: Option<Vec<serde_json::Value>>) -> Self {
            Self {
                queries: std::sync::Mutex::new(Vec::new()),
                response,
            }
        }

        fn queries(&self) -> Vec<String> {
            self.queries.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl KnowledgeSource for MockKnowledgeSource {
        async fn query(&self, target: &str) -> Option<Vec<serde_json::Value>> {
            self.queries.lock().unwrap().push(target.to_string());
            self.response.clone()
        }
    }

    #[tokio::test]
    async fn test_knowledge_source_is_queried() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let source = Arc::new(MockKnowledgeSource::new(None));

        let observer = Observer::new(forge).with_knowledge_source(source.clone());
        assert!(observer.knowledge_source.is_some());

        let _ = observer.gather("test query").await;
        let queries = source.queries();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0], "test query");
    }

    #[tokio::test]
    async fn test_knowledge_source_with_cached_discovery() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let source = Arc::new(MockKnowledgeSource::new(Some(vec![
            serde_json::json!({"target": "nonexistent_symbol"}),
        ])));

        let observer = Observer::new(forge).with_knowledge_source(source.clone());
        let result = observer.gather("test query").await;
        assert!(result.is_ok());
        // Source was queried, even though no symbols matched
        assert_eq!(source.queries().len(), 1);
    }

    #[tokio::test]
    async fn test_observer_without_knowledge_source() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let observer = Observer::new(forge);

        assert!(observer.knowledge_source.is_none());
        let result = observer.gather("test query").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_observer_summarize_with_llm() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mock = Arc::new(crate::llm::MockProvider::new(
            "Found an authentication middleware function that validates tokens.",
        ));

        let observer = Observer::new(forge).with_llm(mock);
        let result = observer.gather("where is the auth middleware?").await;

        assert!(result.is_ok());
        let obs = result.unwrap();
        assert!(
            obs.summary.is_some(),
            "summary should be populated when LLM is configured"
        );
        assert!(obs.summary.unwrap().contains("authentication"));
    }

    #[tokio::test]
    async fn test_observer_summarize_without_llm() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let observer = Observer::new(forge);

        let result = observer.gather("test query").await;
        assert!(result.is_ok());
        let obs = result.unwrap();
        assert!(
            obs.summary.is_none(),
            "summary should be None when no LLM configured"
        );
    }

    // ── Task 3: AgentContext wiring ───────────────────────────────────────

    #[tokio::test]
    async fn test_observer_with_context_sets_prefix() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let ctx = crate::context::AgentContext::from_path(temp_dir.path());
        let observer = Observer::new(forge).with_context(&ctx);
        assert!(
            observer.context_prefix.is_some(),
            "context_prefix should be set after with_context"
        );
        let prefix = observer.context_prefix.as_deref().unwrap_or("");
        assert!(
            prefix.contains("rust"),
            "prefix should contain language: got {prefix}"
        );
    }
}

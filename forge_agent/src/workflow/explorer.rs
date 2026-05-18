//! Knowledge explorer for forge agent.
//!
//! Explores wiki graph and project history to find relevant knowledge
//! before the model proposes a plan.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// What to explore and how deep to go.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExploreQuery {
    pub topic: String,
    pub entity_kinds: Vec<String>,
    pub depth: u32,
    pub limit: usize,
    pub include_history: bool,
}

/// A piece of discovered knowledge relevant to the current task.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredKnowledge {
    pub title: String,
    pub kind: String,
    pub summary: String,
    pub source: String,
    pub discovery_method: String,
    pub relevance: f64,
    pub related: Vec<String>,
    pub is_historical: bool,
}

/// Explores wiki graph + project metadata for relevant knowledge.
pub struct KnowledgeExplorer {
    #[allow(dead_code)] // used by sqlitegraph queries in next iteration
    wiki_db: Option<PathBuf>,
    #[allow(dead_code)] // used by magellan queries in next iteration
    project_db: Option<PathBuf>,
}

impl KnowledgeExplorer {
    /// Create explorer pointing to wiki DB.
    /// Returns None if wiki DB doesn't exist.
    pub fn new(wiki_db: PathBuf) -> Option<Self> {
        if wiki_db.exists() {
            Some(Self {
                wiki_db: Some(wiki_db),
                project_db: None,
            })
        } else {
            None
        }
    }

    /// Create explorer in code-graph-only mode (no wiki).
    pub fn code_only(project_db: PathBuf) -> Self {
        Self {
            wiki_db: None,
            project_db: Some(project_db),
        }
    }

    /// Set project DB for project-specific history.
    pub fn with_project_db(mut self, db: PathBuf) -> Self {
        self.project_db = Some(db);
        self
    }

    /// Explore wiki for knowledge relevant to a query.
    /// No-ops in code_only mode (returns empty vec).
    pub fn explore(&self, _query: &ExploreQuery) -> anyhow::Result<Vec<DiscoveredKnowledge>> {
        // Implementation will use sqlitegraph search + edge traversal
        // For now, returns empty vec — actual DB queries in next iteration
        Ok(Vec::new())
    }

    /// Find project history — past decisions, dead ends, lessons.
    /// Falls back to project's magellan DB if no wiki.
    pub fn find_project_history(&self, _project: &str) -> anyhow::Result<Vec<DiscoveredKnowledge>> {
        Ok(Vec::new())
    }

    /// Find cross-project connections.
    pub fn find_connections(
        &self,
        _symbols: &[String],
    ) -> anyhow::Result<Vec<DiscoveredKnowledge>> {
        Ok(Vec::new())
    }
}

// Implement KnowledgeSource trait so it slots into Observer
#[async_trait::async_trait]
impl crate::observe::KnowledgeSource for KnowledgeExplorer {
    async fn query(&self, target: &str) -> Option<Vec<serde_json::Value>> {
        let explore_query = ExploreQuery {
            topic: target.to_string(),
            entity_kinds: vec!["Knowledge".to_string()],
            depth: 2,
            limit: 10,
            include_history: true,
        };
        self.explore(&explore_query).ok().map(|results| {
            results
                .into_iter()
                .filter_map(|dk| serde_json::to_value(dk).ok())
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_new_returns_some_when_db_exists() {
        let dir = tempfile::tempdir().expect("tempdir creation failed");
        let db_path = dir.path().join("wiki.db");
        let mut f = std::fs::File::create(&db_path).expect("file creation failed");
        f.write_all(b"fake db").expect("write failed");
        f.flush().expect("flush failed");

        let explorer = KnowledgeExplorer::new(db_path);
        assert!(explorer.is_some(), "should return Some when db file exists");
    }

    #[test]
    fn test_new_returns_none_when_no_db() {
        let db_path = PathBuf::from("/tmp/absolutely_nonexistent_wiki_42.db");
        let explorer = KnowledgeExplorer::new(db_path);
        assert!(
            explorer.is_none(),
            "should return None when db file doesn't exist"
        );
    }

    #[test]
    fn test_code_only_mode_returns_empty_explore() {
        let explorer = KnowledgeExplorer::code_only(PathBuf::from("/tmp/project.db"));
        let query = ExploreQuery {
            topic: "test".to_string(),
            entity_kinds: vec!["Knowledge".to_string()],
            depth: 2,
            limit: 10,
            include_history: false,
        };
        let result = explorer.explore(&query).expect("explore should succeed");
        assert!(
            result.is_empty(),
            "code_only mode should return empty results"
        );
    }

    #[test]
    fn test_discovered_knowledge_serialization() {
        let knowledge = DiscoveredKnowledge {
            title: "Auth middleware".to_string(),
            kind: "Knowledge".to_string(),
            summary: "Validates JWT tokens".to_string(),
            source: "wiki".to_string(),
            discovery_method: "semantic_search".to_string(),
            relevance: 0.95,
            related: vec!["jwt".to_string(), "middleware".to_string()],
            is_historical: false,
        };

        let json = serde_json::to_string(&knowledge).expect("serialization failed");
        let roundtrip: DiscoveredKnowledge =
            serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(roundtrip.title, "Auth middleware");
        assert_eq!(roundtrip.kind, "Knowledge");
        assert_eq!(roundtrip.summary, "Validates JWT tokens");
        assert_eq!(roundtrip.source, "wiki");
        assert_eq!(roundtrip.relevance, 0.95);
        assert_eq!(roundtrip.related.len(), 2);
        assert!(!roundtrip.is_historical);
    }

    #[test]
    fn test_explore_query_serialization() {
        let query = ExploreQuery {
            topic: "database migrations".to_string(),
            entity_kinds: vec!["Event".to_string(), "Knowledge".to_string()],
            depth: 3,
            limit: 50,
            include_history: true,
        };

        let json = serde_json::to_string(&query).expect("serialization failed");
        let roundtrip: ExploreQuery = serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(roundtrip.topic, "database migrations");
        assert_eq!(roundtrip.entity_kinds.len(), 2);
        assert_eq!(roundtrip.depth, 3);
        assert_eq!(roundtrip.limit, 50);
        assert!(roundtrip.include_history);
    }
}

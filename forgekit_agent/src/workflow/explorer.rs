//! Knowledge explorer for forge agent.
//!
//! Explores wiki graph and project history to find relevant knowledge
//! before the model proposes a plan.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    wiki_db: Option<PathBuf>,
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
    /// No-ops in code_only mode (wiki_db is None) or when sqlite feature is off.
    pub fn explore(&self, query: &ExploreQuery) -> anyhow::Result<Vec<DiscoveredKnowledge>> {
        #[cfg(feature = "sqlite")]
        if let Some(wiki_db) = &self.wiki_db {
            return Self::query_entities_by_topic(
                wiki_db,
                &query.entity_kinds,
                &query.topic,
                query.limit,
                false,
            );
        }
        let _ = query;
        Ok(Vec::new())
    }

    /// Find project history — past decisions, dead ends, lessons.
    pub fn find_project_history(&self, project: &str) -> anyhow::Result<Vec<DiscoveredKnowledge>> {
        #[cfg(feature = "sqlite")]
        if let Some(project_db) = &self.project_db {
            let kinds = ["Decision", "Lesson", "Event", "Knowledge", "History"];
            return Self::query_entities_by_topic(
                project_db,
                &kinds.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                project,
                50,
                true,
            );
        }
        let _ = project;
        Ok(Vec::new())
    }

    /// Find cross-project connections for the given symbol names.
    pub fn find_connections(&self, symbols: &[String]) -> anyhow::Result<Vec<DiscoveredKnowledge>> {
        #[cfg(feature = "sqlite")]
        if let Some(project_db) = &self.project_db {
            return Self::query_connections(project_db, symbols);
        }
        let _ = symbols;
        Ok(Vec::new())
    }

    /// Open a sqlitegraph DB and return entities whose name or data contains the topic.
    #[cfg(feature = "sqlite")]
    fn query_entities_by_topic(
        db_path: &Path,
        entity_kinds: &[String],
        topic: &str,
        limit: usize,
        is_historical: bool,
    ) -> anyhow::Result<Vec<DiscoveredKnowledge>> {
        let graph = sqlitegraph::SqliteGraph::open(db_path)?;
        let topic_lower = topic.to_lowercase();
        let mut results = Vec::new();

        for kind in entity_kinds {
            if results.len() >= limit {
                break;
            }
            let entities = graph.find_entities_by_kind(kind)?;
            for entity in entities {
                if results.len() >= limit {
                    break;
                }
                let name_lower = entity.name.to_lowercase();
                let data_str = entity.data.to_string().to_lowercase();
                let matches = topic_lower.is_empty()
                    || name_lower.contains(&topic_lower)
                    || data_str.contains(&topic_lower);
                if !matches {
                    continue;
                }
                let summary = entity
                    .data
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&entity.name)
                    .to_string();
                let relevance = if name_lower.contains(&topic_lower) {
                    0.9
                } else {
                    0.6
                };
                results.push(DiscoveredKnowledge {
                    title: entity.name.clone(),
                    kind: entity.kind.clone(),
                    summary,
                    source: db_path.to_string_lossy().to_string(),
                    discovery_method: "sqlitegraph_entity_search".to_string(),
                    relevance,
                    related: Vec::new(),
                    is_historical,
                });
            }
        }
        Ok(results)
    }

    /// For each symbol, find matching entities and collect neighbour names via edges.
    #[cfg(feature = "sqlite")]
    fn query_connections(
        db_path: &Path,
        symbols: &[String],
    ) -> anyhow::Result<Vec<DiscoveredKnowledge>> {
        let graph = sqlitegraph::SqliteGraph::open(db_path)?;
        let pattern = sqlitegraph::PatternTriple::new("RelatedTo");
        let triples = graph.match_triples(&pattern)?;
        let mut results = Vec::new();

        for symbol in symbols {
            let symbol_lower = symbol.to_lowercase();
            let entities = graph.find_entities_by_kind("Knowledge")?;
            for entity in &entities {
                if !entity.name.to_lowercase().contains(&symbol_lower) {
                    continue;
                }
                let related: Vec<String> = triples
                    .iter()
                    .filter_map(|t| {
                        let neighbour_id = if t.start_id == entity.id {
                            t.end_id
                        } else if t.end_id == entity.id {
                            t.start_id
                        } else {
                            return None;
                        };
                        graph.get_entity(neighbour_id).ok().map(|e| e.name)
                    })
                    .collect();
                let summary = entity
                    .data
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&entity.name)
                    .to_string();
                results.push(DiscoveredKnowledge {
                    title: entity.name.clone(),
                    kind: entity.kind.clone(),
                    summary,
                    source: db_path.to_string_lossy().to_string(),
                    discovery_method: "sqlitegraph_connection_search".to_string(),
                    relevance: 0.75,
                    related,
                    is_historical: false,
                });
            }
        }
        Ok(results)
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

    #[cfg(feature = "sqlite")]
    #[test]
    fn test_explore_returns_results_from_wiki_db() {
        let dir = tempfile::tempdir().unwrap();
        let wiki_path = dir.path().join("wiki.db");

        {
            let graph = sqlitegraph::SqliteGraph::open(&wiki_path).unwrap();
            graph
                .insert_entity(&sqlitegraph::GraphEntity {
                    id: 0,
                    kind: "Knowledge".to_string(),
                    name: "Auth middleware uses JWT".to_string(),
                    file_path: None,
                    data: serde_json::json!({"summary": "JWT validation for auth routes"}),
                })
                .unwrap();
            graph
                .insert_entity(&sqlitegraph::GraphEntity {
                    id: 0,
                    kind: "Knowledge".to_string(),
                    name: "Database migrations strategy".to_string(),
                    file_path: None,
                    data: serde_json::json!({"summary": "Blue-green migration approach"}),
                })
                .unwrap();
        }

        let explorer = KnowledgeExplorer::new(wiki_path).expect("db file exists");
        let query = ExploreQuery {
            topic: "auth".to_string(),
            entity_kinds: vec!["Knowledge".to_string()],
            depth: 2,
            limit: 10,
            include_history: false,
        };

        let results = explorer.explore(&query).unwrap();
        assert!(
            !results.is_empty(),
            "explore must return matching entities from the wiki DB, got empty"
        );
        assert!(
            results
                .iter()
                .any(|r| r.title.to_lowercase().contains("auth")),
            "result should match the 'auth' topic"
        );
        assert_eq!(results[0].kind, "Knowledge");
        assert!(!results[0].source.is_empty());
    }

    #[cfg(feature = "sqlite")]
    #[test]
    fn test_find_project_history_returns_historical_entities() {
        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().join("project.db");

        {
            let graph = sqlitegraph::SqliteGraph::open(&project_path).unwrap();
            graph
                .insert_entity(&sqlitegraph::GraphEntity {
                    id: 0,
                    kind: "Decision".to_string(),
                    name: "Chose async runtime for forge".to_string(),
                    file_path: None,
                    data: serde_json::json!({"summary": "Tokio selected for async runtime"}),
                })
                .unwrap();
        }

        let explorer = KnowledgeExplorer::code_only(project_path);
        let results = explorer.find_project_history("forge").unwrap();
        assert!(
            !results.is_empty(),
            "find_project_history must return Decision entities from the project DB"
        );
        assert!(
            results[0].is_historical,
            "results must be flagged is_historical=true"
        );
    }

    #[cfg(feature = "sqlite")]
    #[test]
    fn test_find_connections_returns_related_symbols() {
        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().join("project.db");

        {
            let graph = sqlitegraph::SqliteGraph::open(&project_path).unwrap();
            let id_a = graph
                .insert_entity(&sqlitegraph::GraphEntity {
                    id: 0,
                    kind: "Knowledge".to_string(),
                    name: "AgentLoop".to_string(),
                    file_path: None,
                    data: serde_json::json!({"summary": "Main agent loop"}),
                })
                .unwrap();
            let id_b = graph
                .insert_entity(&sqlitegraph::GraphEntity {
                    id: 0,
                    kind: "Knowledge".to_string(),
                    name: "Observer".to_string(),
                    file_path: None,
                    data: serde_json::json!({"summary": "Observes code graph"}),
                })
                .unwrap();
            graph
                .insert_edge(&sqlitegraph::GraphEdge {
                    id: 0,
                    from_id: id_a,
                    to_id: id_b,
                    edge_type: "RelatedTo".to_string(),
                    data: serde_json::json!({}),
                })
                .unwrap();
        }

        let explorer = KnowledgeExplorer::code_only(project_path);
        let results = explorer
            .find_connections(&["AgentLoop".to_string()])
            .unwrap();
        assert!(
            !results.is_empty(),
            "find_connections must return entities related to AgentLoop"
        );
    }
}

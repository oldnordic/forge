use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum RetrievalSource {
    File,
    Graph,
    Knowledge,
}

#[derive(Clone, Debug)]
pub struct CodeSnippet {
    pub file: PathBuf,
    pub line: usize,
    pub content: String,
    pub score: f64,
    pub source: RetrievalSource,
}

impl fmt::Display for CodeSnippet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{} (score={:.2}):\n{}",
            self.file.display(),
            self.line,
            self.score,
            self.content
        )
    }
}

/// RAG context retrieval trait.
///
/// Implement this trait to provide custom code retrieval for agent context.
///
/// ## Stability
///
/// This trait is part of the stable SDK contract. Breaking changes to the
/// signature will be accompanied by a major version bump.
#[async_trait::async_trait]
pub trait CodeRetriever: Send + Sync {
    async fn retrieve(&self, query: &str, top_k: usize) -> Vec<CodeSnippet>;
}

pub struct FileCodeRetriever {
    root: PathBuf,
    context_lines: usize,
}

impl FileCodeRetriever {
    pub fn new(root: PathBuf) -> Self {
        FileCodeRetriever {
            root,
            context_lines: 3,
        }
    }

    pub fn with_context_lines(mut self, lines: usize) -> Self {
        self.context_lines = lines;
        self
    }

    fn is_source_file(path: &Path) -> bool {
        path.extension()
            .map(|e| {
                matches!(
                    e.to_str(),
                    Some("rs" | "py" | "ts" | "js" | "go" | "java" | "c" | "cpp" | "toml")
                )
            })
            .unwrap_or(false)
    }

    fn should_skip_dir(name: &str) -> bool {
        matches!(
            name,
            "target" | ".git" | ".forge" | ".magellan" | "node_modules"
        )
    }

    async fn collect_source_files(&self, dir: &Path, files: &mut Vec<PathBuf>) {
        let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
            return;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if Self::should_skip_dir(name) {
                        continue;
                    }
                }
                Box::pin(self.collect_source_files(&path, files)).await;
            } else if path.is_file() && Self::is_source_file(&path) {
                files.push(path);
            }
        }
    }

    fn score_line(line: &str, terms: &[&str]) -> f64 {
        let line_lower = line.to_lowercase();
        let mut score = 0.0;
        for term in terms {
            let term_lower = term.to_lowercase();
            if line_lower.contains(&term_lower) {
                score += 1.0;
                if line_lower.starts_with(&term_lower) {
                    score += 0.3;
                }
                if line.contains("fn ") || line.contains("struct ") || line.contains("impl ") {
                    score += 0.5;
                }
            }
        }
        if score > 0.0 {
            score / terms.len() as f64
        } else {
            0.0
        }
    }
}

#[async_trait::async_trait]
impl CodeRetriever for FileCodeRetriever {
    async fn retrieve(&self, query: &str, top_k: usize) -> Vec<CodeSnippet> {
        if query.trim().is_empty() {
            return Vec::new();
        }

        let terms: Vec<&str> = query.split_whitespace().filter(|w| w.len() >= 2).collect();

        if terms.is_empty() {
            return Vec::new();
        }

        let mut files = Vec::new();
        self.collect_source_files(&self.root, &mut files).await;

        let mut candidates: Vec<CodeSnippet> = Vec::new();

        for path in &files {
            let Ok(content) = tokio::fs::read_to_string(path).await else {
                continue;
            };
            let lines: Vec<&str> = content.lines().collect();
            for (idx, line) in lines.iter().enumerate() {
                let score = Self::score_line(line, &terms);
                if score > 0.0 {
                    let start = idx.saturating_sub(self.context_lines);
                    let end = (idx + self.context_lines + 1).min(lines.len());
                    let context_block: String = lines[start..end].join("\n");
                    let relative = path.strip_prefix(&self.root).unwrap_or(path);
                    candidates.push(CodeSnippet {
                        file: relative.to_path_buf(),
                        line: idx + 1,
                        content: context_block,
                        score,
                        source: RetrievalSource::File,
                    });
                }
            }
        }

        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(top_k);
        candidates
    }
}

#[cfg(feature = "atheneum")]
pub struct AtheneumRetriever {
    db_path: PathBuf,
}

#[cfg(feature = "atheneum")]
impl AtheneumRetriever {
    pub fn new(db_path: PathBuf) -> Self {
        AtheneumRetriever { db_path }
    }
}

#[cfg(feature = "atheneum")]
#[async_trait::async_trait]
impl CodeRetriever for AtheneumRetriever {
    async fn retrieve(&self, query: &str, top_k: usize) -> Vec<CodeSnippet> {
        let db_path = self.db_path.clone();
        let query = query.to_string();
        tokio::task::spawn_blocking(move || {
            let graph = match atheneum::graph::AtheneumGraph::open(&db_path) {
                Ok(g) => g,
                Err(_) => return Vec::new(),
            };

            let knowledge = match graph.query_knowledge(&query, None) {
                Ok(k) => k,
                Err(_) => return Vec::new(),
            };

            let mut snippets = Vec::new();

            if let Some(discoveries) = knowledge.get("discoveries").and_then(|d| d.as_array()) {
                for disc in discoveries.iter().take(top_k) {
                    let target = disc
                        .get("target")
                        .and_then(|t| t.as_str())
                        .unwrap_or("unknown");
                    let agent = disc
                        .get("agent")
                        .and_then(|a| a.as_str())
                        .unwrap_or("unknown");
                    let dtype = disc
                        .get("discovery_type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("unknown");
                    let metadata = disc
                        .get("metadata")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);

                    snippets.push(CodeSnippet {
                        file: PathBuf::from(format!("atheneum://discovery/{target}")),
                        line: 0,
                        content: format!(
                            "Discovery by {agent}: [{dtype}] {target}\nMetadata: {metadata}"
                        ),
                        score: 0.8,
                        source: RetrievalSource::Knowledge,
                    });
                }
            }

            if let Some(handoffs) = knowledge.get("handoffs").and_then(|h| h.as_array()) {
                for ho in handoffs.iter().take(top_k.saturating_sub(snippets.len())) {
                    let from = ho
                        .get("from_agent")
                        .and_then(|f| f.as_str())
                        .unwrap_or("unknown");
                    let manifest = ho
                        .get("manifest")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);

                    snippets.push(CodeSnippet {
                        file: PathBuf::from("atheneum://handoff"),
                        line: 0,
                        content: format!("Handoff from {from}:\n{manifest}"),
                        score: 0.6,
                        source: RetrievalSource::Knowledge,
                    });
                }
            }

            snippets.truncate(top_k);
            snippets
        })
        .await
        .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_line_exact_match() {
        let score = FileCodeRetriever::score_line("fn hello()", &["hello"]);
        assert!(score > 0.0);
    }

    #[test]
    fn score_line_no_match() {
        let score = FileCodeRetriever::score_line("fn world()", &["hello"]);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn score_line_definition_bonus() {
        let def_score = FileCodeRetriever::score_line("fn hello()", &["hello"]);
        let call_score = FileCodeRetriever::score_line("hello()", &["hello"]);
        assert!(def_score > call_score);
    }

    #[test]
    fn is_source_file_filters() {
        assert!(FileCodeRetriever::is_source_file(Path::new("foo.rs")));
        assert!(FileCodeRetriever::is_source_file(Path::new("foo.py")));
        assert!(!FileCodeRetriever::is_source_file(Path::new("foo.txt")));
        assert!(!FileCodeRetriever::is_source_file(Path::new("foo.md")));
    }

    #[test]
    fn should_skip_dir_filters() {
        assert!(FileCodeRetriever::should_skip_dir("target"));
        assert!(FileCodeRetriever::should_skip_dir(".git"));
        assert!(!FileCodeRetriever::should_skip_dir("src"));
    }
}

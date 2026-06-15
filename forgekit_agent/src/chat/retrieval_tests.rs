use super::retrieval::{CodeRetriever, CodeSnippet, FileCodeRetriever, RetrievalSource};
use std::path::PathBuf;

struct MockRetriever {
    snippets: Vec<CodeSnippet>,
}

impl MockRetriever {
    fn new(snippets: Vec<CodeSnippet>) -> Self {
        MockRetriever { snippets }
    }
}

#[async_trait::async_trait]
impl CodeRetriever for MockRetriever {
    async fn retrieve(&self, _query: &str, top_k: usize) -> Vec<CodeSnippet> {
        self.snippets.iter().take(top_k).cloned().collect()
    }
}

fn make_snippet(file: &str, line: usize, content: &str, score: f64) -> CodeSnippet {
    CodeSnippet {
        file: PathBuf::from(file),
        line,
        content: content.to_string(),
        score,
        source: RetrievalSource::File,
    }
}

#[tokio::test]
async fn mock_retriever_returns_top_k() {
    let snippets = vec![
        make_snippet("a.rs", 1, "fn a()", 0.9),
        make_snippet("b.rs", 2, "fn b()", 0.8),
        make_snippet("c.rs", 3, "fn c()", 0.7),
    ];
    let retriever = MockRetriever::new(snippets);
    let results = retriever.retrieve("test", 2).await;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].file, PathBuf::from("a.rs"));
    assert_eq!(results[1].file, PathBuf::from("b.rs"));
}

#[tokio::test]
async fn mock_retriever_empty_results() {
    let retriever = MockRetriever::new(vec![]);
    let results = retriever.retrieve("nothing", 5).await;
    assert!(results.is_empty());
}

#[tokio::test]
async fn file_retriever_finds_matching_lines() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("example.rs");
    tokio::fs::write(
        &file_path,
        "fn hello() {\n    println!(\"hello\");\n}\n\nfn world() {\n    println!(\"world\");\n}\n",
    )
    .await
    .unwrap();

    let retriever = FileCodeRetriever::new(dir.path().to_path_buf());
    let results = retriever.retrieve("hello", 10).await;

    assert!(!results.is_empty(), "should find 'hello' in example.rs");
    let first = &results[0];
    assert!(first.file.ends_with("example.rs"));
    assert_eq!(first.line, 1);
    assert!(first.content.contains("hello"));
    assert_eq!(first.source, RetrievalSource::File);
}

#[tokio::test]
async fn file_retriever_respects_top_k() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..5 {
        let file_path = dir.path().join(format!("file{i}.rs"));
        tokio::fs::write(&file_path, format!("fn func_{i}() {{ }}"))
            .await
            .unwrap();
    }

    let retriever = FileCodeRetriever::new(dir.path().to_path_buf());
    let results = retriever.retrieve("func", 3).await;
    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn file_retriever_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    let retriever = FileCodeRetriever::new(dir.path().to_path_buf());
    let results = retriever.retrieve("anything", 10).await;
    assert!(results.is_empty());
}

#[tokio::test]
async fn file_retriever_skips_non_source_files() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(dir.path().join("data.txt"), "fn hello")
        .await
        .unwrap();
    tokio::fs::write(dir.path().join("code.rs"), "fn hello() {}")
        .await
        .unwrap();

    let retriever = FileCodeRetriever::new(dir.path().to_path_buf());
    let results = retriever.retrieve("hello", 10).await;
    assert_eq!(results.len(), 1);
    assert!(results[0].file.ends_with("code.rs"));
}

#[tokio::test]
async fn file_retriever_skips_target_and_git_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let target_dir = dir.path().join("target");
    tokio::fs::create_dir_all(&target_dir).await.unwrap();
    tokio::fs::write(target_dir.join("build.rs"), "fn secret_target() {}")
        .await
        .unwrap();

    let git_dir = dir.path().join(".git");
    tokio::fs::create_dir_all(&git_dir).await.unwrap();
    tokio::fs::write(git_dir.join("config.rs"), "fn secret_git() {}")
        .await
        .unwrap();

    tokio::fs::write(dir.path().join("src.rs"), "fn visible() {}")
        .await
        .unwrap();

    let retriever = FileCodeRetriever::new(dir.path().to_path_buf());
    let results = retriever.retrieve("secret", 10).await;
    assert!(results.is_empty(), "should not search target/ or .git/");

    let results = retriever.retrieve("visible", 10).await;
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn file_retriever_multi_line_context() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("code.rs"),
        "// comment\nfn important() {\n    todo!()\n}\n",
    )
    .await
    .unwrap();

    let retriever = FileCodeRetriever::new(dir.path().to_path_buf());
    let results = retriever.retrieve("important", 10).await;

    assert_eq!(results.len(), 1);
    assert!(results[0].content.contains("fn important()"));
}

#[tokio::test]
async fn code_snippet_display_format() {
    let snippet = make_snippet("src/main.rs", 42, "fn main() {}", 0.95);
    let display = format!("{snippet}");
    assert!(display.contains("src/main.rs"));
    assert!(display.contains("42"));
    assert!(display.contains("fn main()"));
}

#[tokio::test]
async fn retrieval_source_debug() {
    assert_eq!(format!("{:?}", RetrievalSource::File), "File");
    assert_eq!(format!("{:?}", RetrievalSource::Graph), "Graph");
    assert_eq!(format!("{:?}", RetrievalSource::Knowledge), "Knowledge");
}

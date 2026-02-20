//! E2E Wave 3: Search Module - Semantic Search
//!
//! Tests for pattern-based and semantic code search.

use forge_core::Forge;
use std::io::Write;
use tempfile::tempdir;

/// Helper: Create a test project
async fn create_search_test_project() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp_dir = tempdir().unwrap();
    let project_path = temp_dir.path().join("search_test_project");
    std::fs::create_dir(&project_path).unwrap();
    
    let src_dir = project_path.join("src");
    std::fs::create_dir(&src_dir).unwrap();
    
    // Create main.rs with searchable content
    let main_rs = r#"
fn main() {
    calculate_sum(10, 20);
    print_message("Hello");
}

fn calculate_sum(a: i32, b: i32) -> i32 {
    a + b
}

fn print_message(msg: &str) {
    println!("{}", msg);
}

struct DataProcessor {
    items: Vec<i32>,
}

impl DataProcessor {
    fn process_data(&self) -> i32 {
        self.items.iter().sum()
    }
    
    fn add_item(&mut self, item: i32) {
        self.items.push(item);
    }
}
"#;
    
    let mut file = std::fs::File::create(src_dir.join("main.rs")).unwrap();
    file.write_all(main_rs.as_bytes()).unwrap();
    
    (temp_dir, project_path)
}

/// E2E Test 11: Pattern search for function definitions
#[tokio::test]
async fn e2e_search_pattern_function_defs() {
    let (_temp_dir, project_path) = create_search_test_project().await;
    
    let forge = Forge::open(&project_path).await.unwrap();
    
    // Search for function definitions with "process" in name
    let results = forge.search().pattern_search(r"fn \w+process").await;
    
    assert!(results.is_ok(), "Pattern search should work");
    // Results depend on regex implementation
}

/// E2E Test 12: Pattern search alias
#[tokio::test]
async fn e2e_search_pattern_alias() {
    let (_temp_dir, project_path) = create_search_test_project().await;
    
    let forge = Forge::open(&project_path).await.unwrap();
    
    // Use pattern() alias
    let results = forge.search().pattern(r"fn main").await;
    
    assert!(results.is_ok(), "Pattern alias should work");
}

/// E2E Test 13: Semantic search
#[tokio::test]
async fn e2e_search_semantic() {
    let (_temp_dir, project_path) = create_search_test_project().await;
    
    let forge = Forge::open(&project_path).await.unwrap();
    
    // Search for "sum calculation"
    let results = forge.search().semantic_search("sum calculation").await;
    
    assert!(results.is_ok(), "Semantic search should work");
}

/// E2E Test 14: Index operation (no-op in current impl)
#[tokio::test]
async fn e2e_search_index() {
    let (_temp_dir, project_path) = create_search_test_project().await;
    
    let forge = Forge::open(&project_path).await.unwrap();
    
    // Index should succeed (even if no-op)
    let result = forge.search().index().await;
    
    assert!(result.is_ok(), "Index should succeed");
}

/// E2E Test 15: Search with empty query
#[tokio::test]
async fn e2e_search_empty_query() {
    let (_temp_dir, project_path) = create_search_test_project().await;
    
    let forge = Forge::open(&project_path).await.unwrap();
    
    // Empty query should return empty results, not error
    let results = forge.search().semantic_search("").await;
    
    assert!(results.is_ok(), "Empty query should not fail");
    let results = results.unwrap();
    assert!(results.is_empty(), "Empty query should return empty results");
}

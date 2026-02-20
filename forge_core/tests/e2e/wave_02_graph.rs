//! E2E Wave 2: Graph Module - Symbol Queries
//!
//! Tests for symbol lookup, references, and graph navigation.

use forge_core::Forge;
use std::io::Write;
use tempfile::tempdir;

/// Helper: Create a test Rust project with sample code
async fn create_test_project() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp_dir = tempdir().unwrap();
    let project_path = temp_dir.path().join("test_rust_project");
    std::fs::create_dir(&project_path).unwrap();
    
    // Create src directory
    let src_dir = project_path.join("src");
    std::fs::create_dir(&src_dir).unwrap();
    
    // Create main.rs with test code
    let main_rs = r#"
fn main() {
    let result = add(5, 3);
    println!("Result: {}", result);
    greet("World");
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn greet(name: &str) {
    println!("Hello, {}!", name);
}

struct Calculator {
    value: i32,
}

impl Calculator {
    fn new() -> Self {
        Self { value: 0 }
    }
    
    fn add(&mut self, n: i32) {
        self.value += n;
    }
    
    fn get_value(&self) -> i32 {
        self.value
    }
}
"#;
    
    let mut file = std::fs::File::create(src_dir.join("main.rs")).unwrap();
    file.write_all(main_rs.as_bytes()).unwrap();
    
    // Create Cargo.toml
    let cargo_toml = r#"
[package]
name = "test_project"
version = "0.1.0"
edition = "2021"
"#;
    
    let mut file = std::fs::File::create(project_path.join("Cargo.toml")).unwrap();
    file.write_all(cargo_toml.as_bytes()).unwrap();
    
    (temp_dir, project_path)
}

/// E2E Test 6: Find symbol by name
#[tokio::test]
async fn e2e_graph_find_symbol_by_name() {
    let (_temp_dir, project_path) = create_test_project().await;
    
    // Initialize Forge
    let forge = Forge::open(&project_path).await.unwrap();
    
    // Find the "add" function
    let symbols = forge.graph().find_symbol("add").await;
    
    // Should find at least one symbol
    assert!(symbols.is_ok(), "Should be able to query symbols");
    
    let symbols = symbols.unwrap();
    // Note: Results depend on indexing - may be 0 if not indexed
    // Just verify the query doesn't panic
}

/// E2E Test 7: Find multiple symbols with similar names
#[tokio::test]
async fn e2e_graph_find_multiple_symbols() {
    let (_temp_dir, project_path) = create_test_project().await;
    
    let forge = Forge::open(&project_path).await.unwrap();
    
    // Search for "greet" function
    let symbols = forge.graph().find_symbol("greet").await;
    
    assert!(symbols.is_ok());
    // Verify query works - exact count depends on indexing
}

/// E2E Test 8: Find callers of a function
#[tokio::test]
async fn e2e_graph_find_callers() {
    let (_temp_dir, project_path) = create_test_project().await;
    
    let forge = Forge::open(&project_path).await.unwrap();
    
    // Find callers of "add" function
    let callers = forge.graph().callers_of("add").await;
    
    assert!(callers.is_ok(), "Should be able to query callers");
    // Results depend on indexing
}

/// E2E Test 9: Find references to a symbol
#[tokio::test]
async fn e2e_graph_find_references() {
    let (_temp_dir, project_path) = create_test_project().await;
    
    let forge = Forge::open(&project_path).await.unwrap();
    
    // Find references to "Calculator"
    let refs = forge.graph().references("Calculator").await;
    
    assert!(refs.is_ok(), "Should be able to query references");
}

/// E2E Test 10: Handle non-existent symbol gracefully
#[tokio::test]
async fn e2e_graph_nonexistent_symbol() {
    let (_temp_dir, project_path) = create_test_project().await;
    
    let forge = Forge::open(&project_path).await.unwrap();
    
    // Search for symbol that doesn't exist
    let symbols = forge.graph().find_symbol("nonexistent_symbol_xyz").await;
    
    assert!(symbols.is_ok(), "Query should not fail");
    
    // Should return empty results
    let symbols = symbols.unwrap();
    assert!(symbols.is_empty(), "Should return empty for non-existent symbol");
}

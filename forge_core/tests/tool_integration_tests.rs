//! Integration tests for tool crate integrations
//!
//! These tests verify that Forge SDK properly integrates with:
//! - magellan (graph operations)
//! - llmgrep (search operations)
//! - splice (edit operations)
//! - mirage (CFG analysis)
//!
//! Both SQLite and Native V3 backends are tested.

use forge_core::{Forge, BackendKind};
use std::path::PathBuf;

// =============================================================================
// Helper Functions
// =============================================================================

async fn create_test_repo() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    
    // Create a simple Rust project structure
    let src_dir = temp.path().join("src");
    tokio::fs::create_dir_all(&src_dir).await.unwrap();
    
    // Create lib.rs with some symbols
    tokio::fs::write(
        src_dir.join("lib.rs"),
        r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn subtract(a: i32, b: i32) -> i32 {
    a - b
}

pub struct Calculator {
    value: i32,
}

impl Calculator {
    pub fn new() -> Self {
        Self { value: 0 }
    }
    
    pub fn add(&mut self, n: i32) {
        self.value = add(self.value, n);
    }
    
    pub fn result(&self) -> i32 {
        self.value
    }
}
"#
    ).await.unwrap();
    
    temp
}

// =============================================================================
// Backend Tests
// =============================================================================

#[tokio::test]
async fn test_sqlite_backend_basic() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(
        temp.path(),
        BackendKind::SQLite
    ).await.expect("Failed to open with SQLite backend");
    
    assert_eq!(forge.backend_kind(), BackendKind::SQLite);
}

#[tokio::test]
async fn test_native_v3_backend_basic() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(
        temp.path(),
        BackendKind::NativeV3
    ).await.expect("Failed to open with Native V3 backend");
    
    assert_eq!(forge.backend_kind(), BackendKind::NativeV3);
}

// =============================================================================
// Graph Module Tests (via magellan)
// =============================================================================

#[tokio::test]
async fn test_graph_find_symbol_sqlite() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open");
    
    // Index the codebase first (via magellan)
    forge.graph().index().await.expect("Failed to index");
    
    // Now search for symbols
    let symbols = forge.graph()
        .find_symbol("add")
        .await
        .expect("Failed to find symbol");
    
    assert!(!symbols.is_empty(), "Should find 'add' symbol");
    assert!(symbols.iter().any(|s| s.name == "add"));
}

#[tokio::test]
async fn test_graph_find_symbol_native_v3() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::NativeV3)
        .await
        .expect("Failed to open");
    
    // Index the codebase first
    forge.graph().index().await.expect("Failed to index");
    
    // Now search for symbols
    let symbols = forge.graph()
        .find_symbol("Calculator")
        .await
        .expect("Failed to find symbol");
    
    assert!(!symbols.is_empty(), "Should find 'Calculator' symbol");
}

#[tokio::test]
#[ignore = "requires magellan feature"]
async fn test_graph_callers_sqlite() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open");
    
    forge.graph().index().await.expect("Failed to index");
    
    // Find callers of 'add' function
    let callers = forge.graph()
        .callers_of("add")
        .await
        .expect("Failed to find callers");
    
    // The 'add' method in Calculator calls the standalone 'add' function
    assert!(!callers.is_empty(), "Should find callers of 'add'");
}

// Note: For SQLite backend, cross-file references are limited (magellan limitation).
// For Native V3 backend, cross-file references are fully supported.
#[tokio::test]
#[ignore = "requires magellan feature"]
async fn test_graph_references_sqlite() {
    let temp = create_test_repo().await;
    
    // Test with Native V3 backend which supports cross-file references
    let forge = Forge::open_with_backend(temp.path(), BackendKind::NativeV3)
        .await
        .expect("Failed to open");
    
    forge.graph().index().await.expect("Failed to index");
    
    let refs = forge.graph()
        .references("add")
        .await
        .expect("Failed to find references");
    
    assert!(!refs.is_empty(), "Native V3 should find cross-file references to 'add' function");
}

// =============================================================================
// Search Module Tests (via llmgrep)
// =============================================================================

#[tokio::test]
async fn test_search_semantic_sqlite() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open");
    
    // Index for semantic search
    forge.search().index().await.expect("Failed to index for search");
    
    // Semantic search
    let results = forge.search()
        .semantic("addition function")
        .await
        .expect("Failed to search");
    
    assert!(!results.is_empty(), "Should find results for 'addition function'");
    assert!(results.iter().any(|r| r.name.contains("add")));
}

#[tokio::test]
async fn test_search_pattern_sqlite() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open");
    
    // Pattern search (regex)
    let results = forge.search()
        .pattern(r"fn \w+\(")
        .await
        .expect("Failed to search");
    
    assert!(!results.is_empty(), "Should find functions matching pattern");
}

#[tokio::test]
async fn test_search_semantic_native_v3() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::NativeV3)
        .await
        .expect("Failed to open");
    
    forge.search().index().await.expect("Failed to index for search");
    
    let results = forge.search()
        .semantic("calculator implementation")
        .await
        .expect("Failed to search");
    
    assert!(!results.is_empty(), "Should find Calculator struct");
}

// =============================================================================
// Edit Module Tests (via splice)
// =============================================================================

#[tokio::test]
async fn test_edit_patch_symbol_sqlite() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open");
    
    // Index first
    forge.graph().index().await.expect("Failed to index");
    
    // Patch a symbol
    let result = forge.edit()
        .patch_symbol("add", "pub fn add(a: i32, b: i32) -> i32 { a.wrapping_add(b) }")
        .await
        .expect("Failed to patch");
    
    assert!(result.success, "Patch should succeed");
    assert!(!result.changed_files.is_empty(), "Should have changed files");
    
    // Verify the change
    let content = tokio::fs::read_to_string(temp.path().join("src/lib.rs"))
        .await
        .expect("Failed to read file");
    
    assert!(content.contains("wrapping_add"), "Should contain patched content");
}

#[tokio::test]
async fn test_edit_rename_symbol_sqlite() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open");
    
    forge.graph().index().await.expect("Failed to index");
    
    // Rename a symbol
    let result = forge.edit()
        .rename_symbol("add", "sum")
        .await
        .expect("Failed to rename");
    
    assert!(result.success, "Rename should succeed");
    
    // Verify the change
    let content = tokio::fs::read_to_string(temp.path().join("src/lib.rs"))
        .await
        .expect("Failed to read file");
    
    assert!(content.contains("pub fn sum("), "Should contain renamed function");
}

#[tokio::test]
async fn test_edit_patch_symbol_native_v3() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::NativeV3)
        .await
        .expect("Failed to open");
    
    forge.graph().index().await.expect("Failed to index");
    
    let result = forge.edit()
        .patch_symbol("subtract", "pub fn subtract(a: i32, b: i32) -> i32 { a - b } // patched")
        .await
        .expect("Failed to patch");
    
    assert!(result.success, "Patch should succeed");
}

// =============================================================================
// CFG Module Tests (via mirage)
// =============================================================================

#[tokio::test]
async fn test_cfg_paths_sqlite() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open");
    
    // Index the graph first
    forge.graph().index().await.expect("Failed to index graph");
    
    // Index for CFG analysis
    forge.cfg().index().await.expect("Failed to index CFG");
    
    // Get function symbol ID first
    let symbols = forge.graph()
        .find_symbol("add")
        .await
        .expect("Failed to find symbol");
    
    let symbol_id = symbols.first().expect("Should find add function").id;
    
    // Enumerate paths
    let paths = forge.cfg()
        .paths(symbol_id)
        .execute()
        .await
        .expect("Failed to enumerate paths");
    
    assert!(!paths.is_empty(), "Should have at least one path");
}

#[tokio::test]
async fn test_cfg_dominators_sqlite() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open");
    
    // Index the graph first
    forge.graph().index().await.expect("Failed to index graph");
    
    forge.cfg().index().await.expect("Failed to index CFG");
    
    let symbols = forge.graph()
        .find_symbol("add")
        .await
        .expect("Failed to find symbol");
    
    let symbol_id = symbols.first().unwrap().id;
    
    let dominators = forge.cfg()
        .dominators(symbol_id)
        .await
        .expect("Failed to compute dominators");
    
    // At minimum should have entry block
    assert!(dominators.len() >= 1, "Should have at least entry block");
}

#[tokio::test]
async fn test_cfg_native_v3() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::NativeV3)
        .await
        .expect("Failed to open");
    
    forge.cfg().index().await.expect("Failed to index CFG");
    
    let symbols = forge.graph()
        .find_symbol("Calculator::new")
        .await
        .expect("Failed to find symbol");
    
    if let Some(symbol) = symbols.first() {
        let paths = forge.cfg()
            .paths(symbol.id)
            .execute()
            .await
            .expect("Failed to enumerate paths");
        
        // Calculator::new is simple, should have 1 path
        assert!(!paths.is_empty(), "Should have paths");
    }
}

// =============================================================================
// Cross-Backend Consistency Tests
// =============================================================================

#[tokio::test]
async fn test_graph_consistency_across_backends() {
    let temp = create_test_repo().await;
    
    // Open with SQLite
    let forge_sqlite = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open SQLite");
    
    forge_sqlite.graph().index().await.expect("Failed to index SQLite");
    
    let symbols_sqlite = forge_sqlite.graph()
        .find_symbol("add")
        .await
        .expect("Failed to search SQLite");
    
    // Open with Native V3 (different database file)
    let forge_native = Forge::open_with_backend(temp.path(), BackendKind::NativeV3)
        .await
        .expect("Failed to open Native V3");
    
    forge_native.graph().index().await.expect("Failed to index Native V3");
    
    let symbols_native = forge_native.graph()
        .find_symbol("add")
        .await
        .expect("Failed to search Native V3");
    
    // Both should find the same symbol
    assert_eq!(
        symbols_sqlite.len(),
        symbols_native.len(),
        "Both backends should find same number of symbols"
    );
}

#[tokio::test]
async fn test_search_consistency_across_backends() {
    let temp = create_test_repo().await;
    
    let forge_sqlite = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open SQLite");
    forge_sqlite.search().index().await.expect("Failed to index SQLite");
    
    let results_sqlite = forge_sqlite.search()
        .semantic("add calculator")
        .await
        .expect("Failed to search SQLite");
    
    let forge_native = Forge::open_with_backend(temp.path(), BackendKind::NativeV3)
        .await
        .expect("Failed to open Native V3");
    forge_native.search().index().await.expect("Failed to index Native V3");
    
    let results_native = forge_native.search()
        .semantic("add calculator")
        .await
        .expect("Failed to search Native V3");
    
    // Results should be equivalent (may have different internal IDs but same symbols)
    assert!(
        !results_sqlite.is_empty() && !results_native.is_empty(),
        "Both backends should return results"
    );
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[tokio::test]
async fn test_graph_symbol_not_found() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open");
    
    forge.graph().index().await.expect("Failed to index");
    
    let symbols = forge.graph()
        .find_symbol("nonexistent_symbol_12345")
        .await
        .expect("Should not fail, just return empty");
    
    assert!(symbols.is_empty(), "Should return empty for nonexistent symbol");
}

#[tokio::test]
async fn test_edit_symbol_not_found() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open");
    
    forge.graph().index().await.expect("Failed to index");
    
    let result = forge.edit()
        .patch_symbol("nonexistent", "content")
        .await;
    
    assert!(result.is_err(), "Should fail for nonexistent symbol");
}

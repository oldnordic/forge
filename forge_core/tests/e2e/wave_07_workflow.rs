//! Wave 7: Workflow Module E2E Tests
//!
//! End-to-end workflows combining all ForgeKit modules.

use forge_core::Forge;

#[tokio::test]
async fn e2e_workflow_open_and_query() {
    // Complete workflow: Open codebase, search, graph query
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("lib.rs"),
        "fn process() {}\nfn handle() { process(); }"
    ).unwrap();
    
    // Open the codebase
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    // Search for symbols
    let search_results = forge.search().pattern_search(r"fn \w+\(").await.unwrap();
    assert!(!search_results.is_empty());
    
    // Query graph for callers
    let callers = forge.graph().callers_of("process").await;
    assert!(callers.is_ok());
}

#[tokio::test]
async fn e2e_workflow_edit_and_verify() {
    // Complete workflow: Edit code and verify changes
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("lib.rs"),
        "fn old_name() { println!(\"old\"); }"
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    // Rename symbol
    let result = forge.edit().rename_symbol("old_name", "new_name").await;
    assert!(result.is_ok());
    
    // Verify the change
    let content = std::fs::read_to_string(temp_dir.path().join("lib.rs")).unwrap();
    assert!(content.contains("new_name"));
}

#[tokio::test]
async fn e2e_workflow_full_codebase_indexing() {
    // Workflow: Create multi-file codebase and index it
    let temp_dir = tempfile::tempdir().unwrap();
    
    // Create multiple source files
    std::fs::write(temp_dir.path().join("main.rs"), "fn main() { helper(); }").unwrap();
    std::fs::write(temp_dir.path().join("helper.rs"), "pub fn helper() {}").unwrap();
    std::fs::write(temp_dir.path().join("utils.rs"), "pub fn util() {}").unwrap();
    
    // Open and index
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    // Index all modules
    let graph_result = forge.graph().index().await;
    let search_result = forge.search().index().await;
    let cfg_result = forge.cfg().index().await;
    
    assert!(graph_result.is_ok());
    assert!(search_result.is_ok());
    assert!(cfg_result.is_ok());
    
    // Search should find symbols from all files
    let results = forge.search().pattern_search(r"fn \w+\(").await.unwrap();
    assert!(results.len() >= 3);
}

#[tokio::test]
async fn e2e_workflow_chain_operations() {
    // Workflow: Chain multiple operations together
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("lib.rs"),
        "fn target() {}\nfn a() { target(); }\nfn b() { target(); }"
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    // 1. Find symbol
    let symbols = forge.search().pattern_search("target").await.unwrap();
    assert!(!symbols.is_empty());
    
    // 2. Analyze impact
    let impact = forge.analysis().analyze_impact("target").await;
    assert!(impact.is_ok());
    
    // 3. Query callers
    let callers = forge.graph().callers_of("target").await;
    assert!(callers.is_ok());
}

#[tokio::test]
async fn e2e_workflow_error_handling() {
    // Workflow: Verify graceful error handling across modules
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(temp_dir.path().join("lib.rs"), "fn test() {}").unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    // Try operations on non-existent symbols
    let symbol_result = forge.graph().find_symbol("nonexistent").await;
    let patch_result = forge.edit().patch_symbol("nonexistent", "fn nonexistent() {}").await;
    let impact_result = forge.analysis().analyze_impact("nonexistent").await;
    
    // All should handle gracefully (either Ok(empty) or Err)
    // The important thing is they don't panic
    let _ = symbol_result;
    let _ = patch_result;
    let _ = impact_result;
    
    // Test passes if we get here without panic
    assert!(true);
}

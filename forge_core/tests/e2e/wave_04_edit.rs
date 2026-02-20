//! Wave 4: Edit Module E2E Tests
//!
//! Tests for span-safe code refactoring operations.

use forge_core::Forge;

#[tokio::test]
async fn e2e_edit_patch_symbol_function() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("lib.rs"),
        "fn hello() { println!(\"Hello\"); }\n"
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    let result = forge.edit().patch_symbol("hello", "fn hello() { println!(\"Hi!\"); }").await;
    
    assert!(result.is_ok());
    let edit_result = result.unwrap();
    assert!(edit_result.success);
    assert_eq!(edit_result.changed_files.len(), 1);
    
    // Verify file was modified
    let content = std::fs::read_to_string(temp_dir.path().join("lib.rs")).unwrap();
    assert!(content.contains("Hi!"));
    assert!(!content.contains("Hello"));
}

#[tokio::test]
async fn e2e_edit_rename_symbol() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("lib.rs"),
        "fn old_name() {}\nfn caller() { old_name(); }"
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    let result = forge.edit().rename_symbol("old_name", "new_name").await;
    
    assert!(result.is_ok());
    let edit_result = result.unwrap();
    assert!(edit_result.success);
    assert_eq!(edit_result.changed_files.len(), 1);
    
    // Verify both definition and call were renamed
    let content = std::fs::read_to_string(temp_dir.path().join("lib.rs")).unwrap();
    assert!(content.contains("fn new_name()"));
    assert!(content.contains("new_name();"));
    assert!(!content.contains("old_name"));
}

#[tokio::test]
async fn e2e_edit_patch_nonexistent_symbol() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(temp_dir.path().join("lib.rs"), "fn existing() {}").unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    let result = forge.edit().patch_symbol("nonexistent", "fn nonexistent() {}").await;
    
    assert!(result.is_err());
}

#[tokio::test]
async fn e2e_edit_rename_nonexistent_symbol() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(temp_dir.path().join("lib.rs"), "fn existing() {}").unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    let result = forge.edit().rename_symbol("nonexistent", "new_name").await;
    
    assert!(result.is_err());
}

#[tokio::test]
async fn e2e_edit_patch_multiple_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("a.rs"),
        "pub fn shared() { println!(\"A\"); }"
    ).unwrap();
    std::fs::write(
        temp_dir.path().join("b.rs"),
        "pub fn shared() { println!(\"B\"); }"
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    // Patch symbol that exists in multiple files
    let result = forge.edit().patch_symbol("shared", "pub fn shared() { println!(\"Updated\"); }").await;
    
    assert!(result.is_ok());
    let edit_result = result.unwrap();
    assert!(edit_result.success);
    // First file found gets modified
    assert!(!edit_result.changed_files.is_empty());
}

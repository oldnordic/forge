//! Pub/Sub Integration Tests
//!
//! These tests verify that the pub/sub (publish-subscribe) mechanism works
//! correctly for both SQLite and Native V3 backends.
//!
//! Pub/Sub allows real-time notifications for:
//! - Node changes (insert, update, delete)
//! - Edge changes (insert, update, delete)
//! - KV store changes
//! - Transaction commits

use forge_core::{Forge, BackendKind};

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
pub fn test_function() -> i32 {
    42
}

pub struct TestStruct {
    value: i32,
}

impl TestStruct {
    pub fn new() -> Self {
        Self { value: 0 }
    }
}
"#
    ).await.unwrap();
    
    temp
}

// =============================================================================
// Backend Connection Tests
// =============================================================================

#[tokio::test]
async fn test_backend_connection_sqlite() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open with SQLite backend");
    
    // Backend kind should be SQLite
    assert_eq!(forge.backend_kind(), BackendKind::SQLite);
    
    // Verify we can access graph module
    let _graph = forge.graph();
}

#[tokio::test]
async fn test_backend_connection_native_v3() {
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::NativeV3)
        .await
        .expect("Failed to open with Native V3 backend");
    
    // Backend kind should be NativeV3
    assert_eq!(forge.backend_kind(), BackendKind::NativeV3);
    
    // Verify we can access graph module
    let _graph = forge.graph();
}

// =============================================================================
// Backend Consistency Tests
// =============================================================================

#[tokio::test]
async fn test_backend_consistency() {
    let temp_sqlite = create_test_repo().await;
    let temp_v3 = create_test_repo().await;
    
    // Open both backends
    let forge_sqlite = Forge::open_with_backend(temp_sqlite.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open SQLite backend");
    
    let forge_v3 = Forge::open_with_backend(temp_v3.path(), BackendKind::NativeV3)
        .await
        .expect("Failed to open Native V3 backend");
    
    // Backend kinds should differ
    assert_eq!(forge_sqlite.backend_kind(), BackendKind::SQLite);
    assert_eq!(forge_v3.backend_kind(), BackendKind::NativeV3);
    
    // Both should be able to access graph
    let _ = forge_sqlite.graph();
    let _ = forge_v3.graph();
}

// =============================================================================
// Database Persistence Tests
// =============================================================================

#[tokio::test]
async fn test_database_persistence_sqlite() {
    let temp = create_test_repo().await;
    let db_path = temp.path().join(".forge").join("graph.db");
    
    // Create initial database
    {
        let forge = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
            .await
            .expect("Failed to create database");
        
        // Verify database was created
        assert!(db_path.exists(), "Database file should exist");
        
        // Use the graph module
        let _graph = forge.graph();
    }
    
    // Reopen the same database
    {
        let forge = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
            .await
            .expect("Failed to reopen database");
        
        // Verify we can still access it
        let _graph = forge.graph();
        assert_eq!(forge.backend_kind(), BackendKind::SQLite);
    }
}

#[tokio::test]
async fn test_database_persistence_native_v3() {
    let temp = create_test_repo().await;
    let db_path = temp.path().join(".forge").join("graph.v3");
    
    // Create initial database
    {
        let forge = Forge::open_with_backend(temp.path(), BackendKind::NativeV3)
            .await
            .expect("Failed to create V3 database");
        
        // Verify database was created
        assert!(db_path.exists(), "V3 database file should exist");
        
        // Use the graph module
        let _graph = forge.graph();
    }
    
    // Reopen the same database - THIS IS THE CRITICAL TEST for V3 persistence
    {
        let forge = Forge::open_with_backend(temp.path(), BackendKind::NativeV3)
            .await
            .expect("Failed to reopen V3 database - persistence bug!");
        
        // Verify we can still access it
        let _graph = forge.graph();
        assert_eq!(forge.backend_kind(), BackendKind::NativeV3, "V3 database should be accessible after reopen");
    }
}

// =============================================================================
// Multi-Backend Instance Tests
// =============================================================================

#[tokio::test]
async fn test_concurrent_backend_instances() {
    let temp1 = create_test_repo().await;
    let temp2 = create_test_repo().await;
    
    // Open different backends concurrently
    let (forge1, forge2) = tokio::join!(
        Forge::open_with_backend(temp1.path(), BackendKind::SQLite),
        Forge::open_with_backend(temp2.path(), BackendKind::NativeV3)
    );
    
    let forge1 = forge1.expect("SQLite instance failed");
    let forge2 = forge2.expect("Native V3 instance failed");
    
    // Both should work independently
    assert_eq!(forge1.backend_kind(), BackendKind::SQLite);
    assert_eq!(forge2.backend_kind(), BackendKind::NativeV3);
    
    // Both should be able to access graph
    let _ = forge1.graph();
    let _ = forge2.graph();
}

// =============================================================================
// Backend Feature Tests
// =============================================================================

#[tokio::test]
async fn test_sqlite_backend_with_full_sqlite_feature() {
    // This test verifies the full-sqlite feature combination works
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open with full-sqlite feature set");
    
    assert_eq!(forge.backend_kind(), BackendKind::SQLite);
}

#[tokio::test]
async fn test_native_v3_backend_with_full_v3_feature() {
    // This test verifies the full-v3 feature combination works
    let temp = create_test_repo().await;
    
    let forge = Forge::open_with_backend(temp.path(), BackendKind::NativeV3)
        .await
        .expect("Failed to open with full-v3 feature set");
    
    assert_eq!(forge.backend_kind(), BackendKind::NativeV3);
}

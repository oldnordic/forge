//! E2E Wave 1: Core Initialization & Storage
//!
//! Tests for Forge initialization, storage backends, and basic lifecycle.

use forge_core::{Forge, BackendKind};
use std::path::PathBuf;
use tempfile::tempdir;

/// E2E Test 1: Forge initialization with default backend
#[tokio::test]
async fn e2e_forge_initialization_default() {
    let temp_dir = tempdir().unwrap();
    let project_path = temp_dir.path().join("test_project");
    std::fs::create_dir(&project_path).unwrap();
    
    // Create a Forge instance
    let forge = Forge::open(&project_path).await;
    
    assert!(forge.is_ok(), "Forge should initialize successfully");
    
    let forge = forge.unwrap();
    assert_eq!(forge.backend_kind(), BackendKind::default());
}

/// E2E Test 2: Forge initialization with SQLite backend
#[tokio::test]
async fn e2e_forge_initialization_sqlite() {
    let temp_dir = tempdir().unwrap();
    let project_path = temp_dir.path().join("test_project");
    std::fs::create_dir(&project_path).unwrap();
    
    let forge = Forge::open_with_backend(&project_path, BackendKind::SQLite).await;
    
    assert!(forge.is_ok(), "Forge should initialize with SQLite backend");
    assert_eq!(forge.unwrap().backend_kind(), BackendKind::SQLite);
}

/// E2E Test 3: Forge creates .forge directory structure
#[tokio::test]
async fn e2e_forge_creates_directory_structure() {
    let temp_dir = tempdir().unwrap();
    let project_path = temp_dir.path().join("test_project");
    std::fs::create_dir(&project_path).unwrap();
    
    // Initialize Forge
    let _ = Forge::open(&project_path).await.unwrap();
    
    // Check .forge directory exists
    let forge_dir = project_path.join(".forge");
    assert!(forge_dir.exists(), ".forge directory should be created");
    
    // Check graph.db exists
    let graph_db = forge_dir.join("graph.db");
    assert!(graph_db.exists(), "graph.db should be created");
}

/// E2E Test 4: Forge reopens existing database
#[tokio::test]
async fn e2e_forge_reopens_existing() {
    let temp_dir = tempdir().unwrap();
    let project_path = temp_dir.path().join("test_project");
    std::fs::create_dir(&project_path).unwrap();
    
    // First initialization
    let forge1 = Forge::open(&project_path).await.unwrap();
    let backend_kind_1 = forge1.backend_kind();
    
    // Second initialization (reopen)
    let forge2 = Forge::open(&project_path).await;
    
    assert!(forge2.is_ok(), "Should reopen existing database");
    assert_eq!(forge2.unwrap().backend_kind(), backend_kind_1);
}

/// E2E Test 5: Forge handles invalid paths gracefully
#[tokio::test]
async fn e2e_forge_handles_invalid_path() {
    // Try to open a non-existent path
    let invalid_path = PathBuf::from("/nonexistent/path/that/does/not/exist");
    
    let result = Forge::open(&invalid_path).await;
    
    // Should fail gracefully
    assert!(result.is_err(), "Should fail for invalid path");
}

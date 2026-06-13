use std::path::PathBuf;
use std::sync::Arc;

use crate::types::{Language, Location, Reference, ReferenceKind, Symbol, SymbolId, SymbolKind};

use super::*;

#[test]
fn test_backend_kind_default() {
    assert_eq!(BackendKind::default(), BackendKind::SQLite);
}

#[test]
fn test_backend_kind_to_sqlitegraph() {
    assert_eq!(
        BackendKind::SQLite.to_sqlitegraph_kind(),
        SqliteGraphBackendKind::SQLite
    );
    assert_eq!(
        BackendKind::NativeV3.to_sqlitegraph_kind(),
        SqliteGraphBackendKind::Native
    );
}

#[test]
fn test_backend_kind_file_extension() {
    assert_eq!(BackendKind::SQLite.file_extension(), "db");
    assert_eq!(BackendKind::NativeV3.file_extension(), "v3");
}

#[test]
fn test_backend_kind_default_filename() {
    assert_eq!(BackendKind::SQLite.default_filename(), "graph.db");
    assert_eq!(BackendKind::NativeV3.default_filename(), "graph.v3");
}

#[test]
fn test_backend_kind_display() {
    assert_eq!(BackendKind::SQLite.to_string(), "SQLite");
    assert_eq!(BackendKind::NativeV3.to_string(), "NativeV3");
}

#[tokio::test]
async fn test_open_sqlite_creates_database() {
    let temp_dir = tempfile::tempdir().unwrap();
    let store = UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
        .await
        .unwrap();

    assert_eq!(store.backend_kind(), BackendKind::SQLite);
    assert!(store.db_path().to_string_lossy().contains(".magellan"));
    assert!(store.db_path().extension().is_some_and(|e| e == "db"));
    assert!(store.is_connected());
}

#[tokio::test]
async fn test_open_native_v3_creates_database() {
    let temp_dir = tempfile::tempdir().unwrap();
    let store = UnifiedGraphStore::open(temp_dir.path(), BackendKind::NativeV3)
        .await
        .unwrap();

    assert_eq!(store.backend_kind(), BackendKind::NativeV3);
    assert!(store.db_path().to_string_lossy().contains(".magellan"));
    assert!(store.db_path().extension().is_some_and(|e| e == "db"));
    assert!(store.is_connected());
}

#[tokio::test]
async fn test_open_with_custom_path() {
    let temp_dir = tempfile::tempdir().unwrap();
    let custom_db = temp_dir.path().join("custom").join("graph.db");

    let store = UnifiedGraphStore::open_with_path(temp_dir.path(), &custom_db, BackendKind::SQLite)
        .await
        .unwrap();

    assert_eq!(store.db_path(), custom_db);
    assert!(store.is_connected());
}

#[tokio::test]
async fn test_insert_symbol_returns_id() {
    let store = UnifiedGraphStore::memory().await.unwrap();

    let symbol = Symbol {
        id: SymbolId(0),
        name: Arc::from("test_function"),
        fully_qualified_name: Arc::from("crate::test_function"),
        kind: SymbolKind::Function,
        language: Language::Rust,
        location: Location {
            file_path: PathBuf::from("src/lib.rs"),
            byte_start: 0,
            byte_end: 100,
            line_number: 10,
        },
        parent_id: None,
        metadata: serde_json::json!({"doc": "Test function"}),
    };

    let id = store.insert_symbol(&symbol).await.unwrap();
    assert!(id.0 > 0);
}

#[tokio::test]
async fn test_query_symbols_empty() {
    let temp_dir = tempfile::tempdir().unwrap();
    let store = UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
        .await
        .unwrap();

    let results = store.query_symbols("nonexistent_xyz").await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_insert_reference_placeholder() {
    let store = UnifiedGraphStore::memory().await.unwrap();

    let reference = Reference {
        from: SymbolId(1),
        to: SymbolId(2),
        from_name: None,
        to_name: None,
        kind: ReferenceKind::Call,
        location: Location {
            file_path: PathBuf::from("src/lib.rs"),
            byte_start: 25,
            byte_end: 35,
            line_number: 2,
        },
    };

    store.insert_reference(&reference).await.unwrap();
}

#[tokio::test]
async fn test_symbol_exists_unknown_id() {
    let (store, _dir) = isolated_store().await;
    assert!(!store.symbol_exists(SymbolId(99999)).await.unwrap());
}

#[tokio::test]
async fn test_get_all_symbols_empty_db() {
    let (store, _dir) = isolated_store().await;
    let symbols = store.get_all_symbols().await.unwrap();
    assert!(symbols.is_empty());
}

#[tokio::test]
async fn test_get_all_symbols_returns_inserted() {
    let (store, _dir) = isolated_store().await;
    store
        .insert_symbol(&make_symbol("alpha_get_all"))
        .await
        .unwrap();
    store
        .insert_symbol(&make_symbol("beta_get_all"))
        .await
        .unwrap();
    let symbols = store.get_all_symbols().await.unwrap();
    assert_eq!(
        symbols.len(),
        2,
        "get_all_symbols should return all inserted symbols"
    );
}

#[tokio::test]
async fn test_symbol_count_empty_db() {
    let (store, _dir) = isolated_store().await;
    assert_eq!(store.symbol_count().await.unwrap(), 0);
}

#[tokio::test]
async fn test_symbol_count_after_inserts() {
    let (store, _dir) = isolated_store().await;
    store
        .insert_symbol(&make_symbol("count_sym_a"))
        .await
        .unwrap();
    store
        .insert_symbol(&make_symbol("count_sym_b"))
        .await
        .unwrap();
    store
        .insert_symbol(&make_symbol("count_sym_c"))
        .await
        .unwrap();
    assert_eq!(
        store.symbol_count().await.unwrap(),
        3,
        "symbol_count should equal insert count"
    );
}

#[test]
fn test_unified_graph_store_clone() {
    let store = UnifiedGraphStore {
        codebase_path: PathBuf::from("/test"),
        db_path: PathBuf::from("/test/graph.db"),
        backend_kind: BackendKind::SQLite,
        references: std::sync::Mutex::new(Vec::new()),
    };

    let cloned = store.clone();

    assert_eq!(cloned.codebase_path, PathBuf::from("/test"));
    assert_eq!(cloned.db_path, PathBuf::from("/test/graph.db"));
    assert_eq!(cloned.backend_kind, BackendKind::SQLite);
}

#[test]
fn test_unified_graph_store_debug() {
    let store = UnifiedGraphStore {
        codebase_path: PathBuf::from("/test"),
        db_path: PathBuf::from("/test/graph.db"),
        backend_kind: BackendKind::SQLite,
        references: std::sync::Mutex::new(Vec::new()),
    };

    let debug_str = format!("{:?}", store);
    assert!(debug_str.contains("UnifiedGraphStore"));
    assert!(debug_str.contains("codebase_path: \"/test\""));
    assert!(debug_str.contains("db_path: \"/test/graph.db\""));
    assert!(debug_str.contains("backend_kind: SQLite"));
}

#[test]
fn test_default_db_path_uses_home_dot_magellan() {
    let project = std::path::Path::new("/home/user/Projects/my-cool-project");
    let db = default_db_path(project);
    assert!(db.to_string_lossy().contains(".magellan"));
    assert!(db.to_string_lossy().contains("my-cool-project"));
}

#[test]
fn test_default_db_path_fallback_stem() {
    let project = std::path::Path::new("/");
    let db = default_db_path(project);
    assert!(db.to_string_lossy().contains(".magellan"));
}

#[test]
fn test_fallback_db_path_uses_subdirectory() {
    let project = std::path::Path::new("/home/user/Projects/geographdb-core");
    let db = fallback_db_path(project);
    let db_str = db.to_string_lossy();
    assert!(
        db_str.contains(".magellan/geographdb-core/geographdb-core.db"),
        "expected subdirectory convention, got: {}",
        db_str
    );
}

#[test]
fn test_lookup_registry_finds_forge_core() {
    let src_dir = std::path::Path::new("/home/feanor/Projects/forge/forge_core/src");
    if !src_dir.exists() {
        return;
    }
    let db = default_db_path(src_dir);
    let db_str = db.to_string_lossy();
    assert!(
        db_str.contains("forge/forge-core.db"),
        "expected registry match for forge_core/src, got: {}",
        db_str
    );
}

#[test]
fn test_lookup_registry_finds_forge_agent() {
    let src_dir = std::path::Path::new("/home/feanor/Projects/forge/forge_agent/src");
    if !src_dir.exists() {
        return;
    }
    let db = default_db_path(src_dir);
    let db_str = db.to_string_lossy();
    assert!(
        db_str.contains("forge/forge-agent.db"),
        "expected registry match for forge_agent/src, got: {}",
        db_str
    );
}

#[test]
fn test_lookup_registry_from_codebase_path_without_src() {
    let crate_dir = std::path::Path::new("/home/feanor/Projects/forge/forge_core");
    if !crate_dir.exists() {
        return;
    }
    let db = default_db_path(crate_dir);
    let db_str = db.to_string_lossy();
    assert!(
        db_str.contains("forge/forge-core.db"),
        "expected registry match for forge_core (without /src), got: {}",
        db_str
    );
}

#[test]
fn test_lookup_registry_from_agent_codebase_path() {
    let crate_dir = std::path::Path::new("/home/feanor/Projects/forge/forge_agent");
    if !crate_dir.exists() {
        return;
    }
    let db = default_db_path(crate_dir);
    let db_str = db.to_string_lossy();
    assert!(
        db_str.contains("forge/forge-agent.db"),
        "expected registry match for forge_agent (without /src), got: {}",
        db_str
    );
}

fn make_symbol(name: &str) -> Symbol {
    Symbol {
        id: SymbolId(0),
        name: Arc::from(name),
        fully_qualified_name: Arc::from(name),
        kind: SymbolKind::Function,
        language: Language::Rust,
        location: Location {
            file_path: PathBuf::from("src/lib.rs"),
            byte_start: 0,
            byte_end: 10,
            line_number: 1,
        },
        parent_id: None,
        metadata: serde_json::Value::Null,
    }
}

async fn isolated_store() -> (UnifiedGraphStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store = UnifiedGraphStore::open_with_path(dir.path(), &db_path, BackendKind::SQLite)
        .await
        .unwrap();
    (store, dir)
}

#[tokio::test]
async fn test_insert_symbol_unique_ids() {
    let (store, _dir) = isolated_store().await;
    let id1 = store.insert_symbol(&make_symbol("alpha_fn")).await.unwrap();
    let id2 = store.insert_symbol(&make_symbol("beta_fn")).await.unwrap();
    assert_ne!(id1, id2, "each insert should return a unique ID");
}

#[tokio::test]
async fn test_symbol_exists_after_insert() {
    let (store, _dir) = isolated_store().await;
    let id = store.insert_symbol(&make_symbol("check_fn")).await.unwrap();
    assert!(
        store.symbol_exists(id).await.unwrap(),
        "symbol should exist after insert"
    );
}

#[tokio::test]
async fn test_query_symbols_finds_inserted() {
    let (store, _dir) = isolated_store().await;
    store
        .insert_symbol(&make_symbol("my_unique_query_target"))
        .await
        .unwrap();
    let results = store.query_symbols("my_unique_query_target").await.unwrap();
    assert!(!results.is_empty(), "query should find the inserted symbol");
}

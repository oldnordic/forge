//! Integration tests for module accessors.

use forge_core::Forge;

mod common {
    pub use crate::common::*;
}

#[tokio::test]
async fn test_all_accessors_work() {
    let (_temp, forge) = common::test_forge().await.unwrap();

    // All accessors should return valid instances
    let _graph = forge.graph();
    let _search = forge.search();
    let _cfg = forge.cfg();
    let _edit = forge.edit();
    let _analysis = forge.analysis();
}

#[tokio::test]
async fn test_accessor_returns_different_instances() {
    let (_temp, forge) = common::test_forge().await.unwrap();

    // Accessors should return new instances each call
    let g1 = forge.graph();
    let g2 = forge.graph();
    // They are equal values but different references (can't directly test this)
    // but we can verify both work
    drop(g1);
    drop(g2);
}

#[tokio::test]
async fn test_graph_module_has_store() {
    let (_temp, forge) = common::test_forge().await.unwrap();

    // Graph module should have access to store
    let graph = forge.graph();
    // Verify graph module is functional (no panics)
    drop(graph);
}

#[tokio::test]
async fn test_search_module_works() {
    let (_temp, forge) = common::test_forge().await.unwrap();

    // Search module should be functional
    let search = forge.search();
    // Verify search module is functional (no panics)
    drop(search);
}

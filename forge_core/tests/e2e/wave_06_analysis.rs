//! Wave 6: Analysis Module E2E Tests
//!
//! Tests for combined analysis operations using graph, CFG, and edit modules.

use forge_core::Forge;

#[tokio::test]
async fn e2e_analysis_impact_analysis_exists() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("lib.rs"),
        "fn target() {}\nfn caller1() { target(); }\nfn caller2() { target(); }"
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    // Analyze impact of changing 'target' function
    let impact = forge.analysis().analyze_impact("target").await;
    
    // v0.1: Method exists and returns successfully
    assert!(impact.is_ok());
}

#[tokio::test]
async fn e2e_analysis_find_dead_code_exists() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("lib.rs"),
        "pub fn used() {}\nfn unused() {}"
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    // v0.1: Method exists and returns successfully
    let dead_code = forge.analysis().find_dead_code().await;
    assert!(dead_code.is_ok());
}

#[tokio::test]
async fn e2e_analysis_complexity_metrics_exists() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("lib.rs"),
        "fn simple() {}\nfn complex() { if true { if true { loop {} } } }"
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    // v0.1: Method exists and returns successfully
    let metrics = forge.analysis().complexity_metrics("complex").await;
    assert!(metrics.is_ok());
}

#[tokio::test]
async fn e2e_analysis_cross_references_exists() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("lib.rs"),
        "fn foo() { bar(); }\nfn bar() { baz(); }\nfn baz() {}"
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    // v0.1: Method exists and returns successfully
    let xrefs = forge.analysis().cross_references("bar").await;
    assert!(xrefs.is_ok());
}

#[tokio::test]
async fn e2e_analysis_module_dependencies_exists() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp_dir.path().join("mod_a")).unwrap();
    std::fs::create_dir(temp_dir.path().join("mod_b")).unwrap();
    
    std::fs::write(
        temp_dir.path().join("mod_a.rs"),
        "pub fn func_a() { crate::mod_b::func_b(); }"
    ).unwrap();
    std::fs::write(
        temp_dir.path().join("mod_b.rs"),
        "pub fn func_b() {}"
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    // v0.1: Method exists and returns successfully
    let deps = forge.analysis().module_dependencies().await;
    assert!(deps.is_ok());
}

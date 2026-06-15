//! E2E Tests for ForgeKit SDK
//!
//! End-to-end tests organized by waves.

pub mod wave_01_core;
pub mod wave_02_graph;
pub mod wave_03_search;
pub mod wave_04_edit;
pub mod wave_05_cfg;
pub mod wave_06_analysis;
pub mod wave_07_workflow;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static PROJECT_SEQ: AtomicU64 = AtomicU64::new(0);

/// Create a temp directory containing a project subdirectory with a **unique
/// basename**.
///
/// The basename uniqueness is critical: `Forge::open` resolves the database to
/// `~/.magellan/<stem>/<stem>.db` keyed by the directory's file name. If two
/// parallel tests share the same basename they race on the same database file
/// during schema initialization (`duplicate column name`). A per-call counter
/// guarantees every test resolves to its own database.
pub fn unique_project_dir() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("create temp dir");
    let id = PROJECT_SEQ.fetch_add(1, Ordering::Relaxed);
    let project_path = temp.path().join(format!("proj_{id}"));
    std::fs::create_dir(&project_path).expect("create project dir");
    (temp, project_path)
}

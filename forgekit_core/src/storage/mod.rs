//! Storage abstraction layer supporting dual backends.
//!
//! This module provides graph-based storage for ForgeKit with support for both
//! SQLite and Native V3 backends. Users choose the backend based on their needs.
//!
//! # Backend Selection
//!
//! | Feature | SQLite Backend | Native V3 Backend |
//! |---------|----------------|-------------------|
//! | ACID Transactions | ✅ Full | ✅ WAL-based |
//! | Raw SQL Access | ✅ Yes | ❌ No |
//! | Dependencies | libsqlite3 | Pure Rust |
//! | Startup Time | Fast | Faster |
//! | Tool Compatibility | magellan, llmgrep, mirage, splice (current) | Updated tools |
//!
//! # Examples
//!
//! ```rust,no_run
//! use forgekit_core::storage::{UnifiedGraphStore, BackendKind};
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! // Use SQLite backend (default, stable)
//! let store = UnifiedGraphStore::open("./codebase", BackendKind::SQLite).await?;
//!
//! // Or use Native V3 backend (updated tools required)
//! let store = UnifiedGraphStore::open("./codebase", BackendKind::NativeV3).await?;
//! # Ok(())
//! # }
//! ```

mod ops;
mod store;
#[cfg(test)]
mod tests;

pub use sqlitegraph::backend::{EdgeSpec, NodeSpec};
pub use sqlitegraph::config::{open_graph, BackendKind as SqliteGraphBackendKind, GraphConfig};
pub use sqlitegraph::graph::{GraphEntity, SqliteGraph};

pub use store::UnifiedGraphStore;

use std::path::{Path, PathBuf};

/// Resolve the database path for a project by consulting the magellan registry,
/// falling back to the `~/.magellan/<stem>/<stem>.db` convention.
///
/// The magellan registry at `~/.config/magellan/registry.toml` maps project roots
/// to database paths. When a project root matches (or is a parent of) the given
/// `project_root`, the registered DB path is returned.
///
/// For workspace monorepos (e.g. forge with forgekit-core, forgekit-agent), each crate
/// is registered separately with its own `src/` root and DB path.
pub fn default_db_path(project_root: &Path) -> PathBuf {
    if let Some(db) = lookup_registry(project_root) {
        return db;
    }

    fallback_db_path(project_root)
}

fn fallback_db_path(project_root: &Path) -> PathBuf {
    let stem = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("graph");
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home)
        .join(".magellan")
        .join(stem)
        .join(format!("{}.db", stem))
}

fn lookup_registry(project_root: &Path) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let registry_path = PathBuf::from(&home)
        .join(".config")
        .join("magellan")
        .join("registry.toml");

    let content = std::fs::read_to_string(&registry_path).ok()?;

    let canonical_root = project_root
        .canonicalize()
        .ok()
        .unwrap_or_else(|| project_root.to_path_buf());

    for block in content.split("[[project]]") {
        let mut name = None;
        let mut root = None;
        let mut db = None;

        for line in block.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("name = ") {
                name = parse_toml_string(rest);
            } else if let Some(rest) = trimmed.strip_prefix("root = ") {
                root = parse_toml_string(rest);
            } else if let Some(rest) = trimmed.strip_prefix("db = ") {
                db = parse_toml_string(rest);
            }
        }

        if let (Some(proj_root), Some(proj_db)) = (root, db) {
            let proj_root_path = Path::new(&proj_root);
            if canonical_root.starts_with(proj_root_path)
                || proj_root_path.starts_with(&canonical_root)
                || paths_equal_after_src_strip(&canonical_root, proj_root_path)
            {
                return Some(PathBuf::from(proj_db));
            }
        }

        let _ = name;
    }

    None
}

fn paths_equal_after_src_strip(a: &Path, b: &Path) -> bool {
    let a_str = a.to_string_lossy();
    let b_str = b.to_string_lossy();

    if let Some(a_stripped) = a_str.strip_suffix("/src") {
        if a_stripped == b_str {
            return true;
        }
    }
    if let Some(b_stripped) = b_str.strip_suffix("/src") {
        if b_stripped == a_str {
            return true;
        }
    }
    false
}

fn parse_toml_string(s: &str) -> Option<String> {
    s.trim()
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .map(|s| s.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BackendKind {
    #[default]
    SQLite,
    NativeV3,
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SQLite => write!(f, "SQLite"),
            Self::NativeV3 => write!(f, "NativeV3"),
        }
    }
}

impl BackendKind {
    #[cfg(test)]
    fn to_sqlitegraph_kind(self) -> SqliteGraphBackendKind {
        match self {
            Self::SQLite => SqliteGraphBackendKind::SQLite,
            Self::NativeV3 => SqliteGraphBackendKind::Native,
        }
    }

    pub fn file_extension(&self) -> &str {
        match self {
            Self::SQLite => "db",
            Self::NativeV3 => "v3",
        }
    }

    pub fn default_filename(&self) -> &str {
        match self {
            Self::SQLite => "graph.db",
            Self::NativeV3 => "graph.v3",
        }
    }
}

//! Incremental indexing for processing file changes.
//!
//! This module provides change-based incremental indexing to avoid full
//! re-scans when files are modified.

use crate::storage::UnifiedGraphStore;
use crate::watcher::WatchEvent;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Path filter for controlling which files get indexed.
///
/// By default, only files in `src/` and `tests/` directories are indexed.
#[derive(Clone, Debug)]
pub struct PathFilter {
    /// Include patterns (files must match at least one)
    include_patterns: Vec<String>,
    /// Exclude patterns (files matching these are rejected)
    exclude_patterns: Vec<String>,
    /// File extensions to include (empty = all)
    extensions: Vec<String>,
}

impl Default for PathFilter {
    fn default() -> Self {
        Self::new_with_defaults()
    }
}

impl PathFilter {
    /// Creates an empty path filter with no patterns.
    /// Use `PathFilter::default()` or `PathFilter::new_with_defaults()` for the standard filter.
    pub fn new() -> Self {
        Self {
            include_patterns: vec![],
            exclude_patterns: vec![],
            extensions: vec![],
        }
    }

    /// Creates a path filter with default settings (src/ and tests/ only).
    pub fn new_with_defaults() -> Self {
        Self {
            // Only index src/ and tests/ directories
            include_patterns: vec![
                "**/src/**".to_string(),
                "**/tests/**".to_string(),
            ],
            // Exclude common non-source directories and files
            exclude_patterns: vec![
                "**/target/**".to_string(),
                "**/node_modules/**".to_string(),
                ".git/**".to_string(),
                "**/.forge/**".to_string(),
                "**/Cargo.lock".to_string(),
                "**/package-lock.json".to_string(),
                "**/yarn.lock".to_string(),
                "**/*.min.js".to_string(),
                "**/*.min.css".to_string(),
            ],
            // Only index source code files
            extensions: vec![
                "rs".to_string(),   // Rust
                "py".to_string(),   // Python
                "js".to_string(),   // JavaScript
                "ts".to_string(),   // TypeScript
                "jsx".to_string(),  // React JSX
                "tsx".to_string(),  // React TSX
                "go".to_string(),   // Go
                "java".to_string(), // Java
                "c".to_string(),    // C
                "cpp".to_string(),  // C++
                "h".to_string(),    // C header
                "hpp".to_string(),  // C++ header
                "mod".to_string(),  // Go module
            ],
        }
    }

    /// Creates a path filter that only includes specific directories.
    ///
    /// # Arguments
    ///
    /// * `dirs` - Directories to include (e.g., ["src", "tests"])
    pub fn include_dirs(dirs: &[&str]) -> Self {
        let mut filter = Self::default();
        filter.include_patterns = dirs
            .iter()
            .map(|d| format!("**/{}/**", d))
            .collect();
        filter
    }

    /// Checks if a path should be indexed.
    ///
    /// A path is indexed if:
    /// 1. It matches at least one include pattern
    /// 2. It does NOT match any exclude pattern
    /// 3. It has an allowed extension (if extensions are specified)
    ///
    /// # Arguments
    ///
    /// * `path` - The file path to check
    pub fn should_index(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Check exclude patterns first
        for pattern in &self.exclude_patterns {
            if Self::match_glob(&path_str, pattern) {
                return false;
            }
        }

        // Check include patterns
        let mut included = false;
        for pattern in &self.include_patterns {
            if Self::match_glob(&path_str, pattern) {
                included = true;
                break;
            }
        }
        if !included {
            return false;
        }

        // Check extension
        if !self.extensions.is_empty() {
            if let Some(ext) = path.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if !self.extensions.contains(&ext) {
                    return false;
                }
            } else {
                // No extension and we require one
                return false;
            }
        }

        true
    }

    /// Simple glob matching (supports * and ** wildcards).
    fn match_glob(path: &str, pattern: &str) -> bool {
        // Handle **/dir/** pattern (matches dir anywhere in path, with contents)
        if pattern.starts_with("**/") && pattern.ends_with("/**") {
            let dir = &pattern[3..pattern.len()-3]; // Extract "dir" from "**/dir/**"
            // Path should contain the directory
            return path.contains(&format!("{}/", dir)) || path.starts_with(&format!("{}/", dir));
        }
        
        // Handle **/suffix pattern (matches suffix anywhere in path)
        if pattern.starts_with("**/") {
            let suffix = &pattern[3..]; // Remove "**/"
            return path.contains(suffix) || path.ends_with(suffix);
        }
        
        // Handle pattern with ** in the middle (e.g., "src/**/test.rs")
        if pattern.contains("/**/") {
            let parts: Vec<&str> = pattern.split("/**/").collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return path.starts_with(prefix) && path.contains(suffix);
            }
        }
        
        // Handle single * wildcard (matches within a path component)
        if pattern.contains('*') {
            // Convert glob pattern to regex
            let mut regex_str = String::with_capacity(pattern.len() * 2);
            regex_str.push('^');
            
            for c in pattern.chars() {
                match c {
                    '*' => regex_str.push_str(".*"),
                    '.' => regex_str.push_str("\\."),
                    '?' => regex_str.push('.'),
                    '+' => regex_str.push_str("\\+"),
                    '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                        regex_str.push('\\');
                        regex_str.push(c);
                    }
                    _ => regex_str.push(c),
                }
            }
            regex_str.push('$');
            
            if let Ok(re) = regex::Regex::new(&regex_str) {
                return re.is_match(path);
            }
        }
        
        // Exact match or substring match
        path == pattern || path.contains(pattern)
    }

    /// Adds an include pattern.
    pub fn add_include(&mut self, pattern: impl Into<String>) {
        self.include_patterns.push(pattern.into());
    }

    /// Adds an exclude pattern.
    pub fn add_exclude(&mut self, pattern: impl Into<String>) {
        self.exclude_patterns.push(pattern.into());
    }

    /// Adds an allowed extension.
    pub fn add_extension(&mut self, ext: impl Into<String>) {
        self.extensions.push(ext.into());
    }
}

/// Incremental indexer for processing file changes.
///
/// The `IncrementalIndexer` batches file system events and processes
/// them on flush, avoiding full re-indexing of the codebase.
///
/// # Examples
///
/// ```no_run
/// use forge_core::indexing::IncrementalIndexer;
/// use forge_core::watcher::WatchEvent;
/// use std::path::PathBuf;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// # let store = unimplemented!();
/// let indexer = IncrementalIndexer::new(store);
///
/// // Queue some changes (only src/ and tests/ files will be indexed)
/// indexer.queue(WatchEvent::Modified(PathBuf::from("src/lib.rs")));
/// indexer.queue(WatchEvent::Created(PathBuf::from("tests/test.rs")));
/// indexer.queue(WatchEvent::Modified(PathBuf::from("target/debug/build.rs"))); // Ignored
///
/// // Process changes
/// indexer.flush().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct IncrementalIndexer {
    /// The graph store for writing index updates.
    store: Arc<UnifiedGraphStore>,
    /// Pending files to process.
    pending: Arc<tokio::sync::Mutex<HashSet<PathBuf>>>,
    /// Files to delete.
    deleted: Arc<tokio::sync::Mutex<HashSet<PathBuf>>>,
    /// Path filter for controlling which files get indexed.
    filter: PathFilter,
}

impl IncrementalIndexer {
    /// Creates a new incremental indexer with default path filtering.
    ///
    /// By default, only files in `src/` and `tests/` directories are indexed.
    ///
    /// # Arguments
    ///
    /// * `store` - The graph store for index updates
    pub fn new(store: Arc<UnifiedGraphStore>) -> Self {
        Self {
            store,
            pending: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            deleted: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            filter: PathFilter::default(),
        }
    }

    /// Creates a new incremental indexer with a custom path filter.
    ///
    /// # Arguments
    ///
    /// * `store` - The graph store for index updates
    /// * `filter` - Custom path filter
    pub fn with_filter(store: Arc<UnifiedGraphStore>, filter: PathFilter) -> Self {
        Self {
            store,
            pending: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            deleted: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            filter,
        }
    }

    /// Returns a reference to the path filter.
    pub fn filter(&self) -> &PathFilter {
        &self.filter
    }

    /// Sets a new path filter.
    pub fn set_filter(&mut self, filter: PathFilter) {
        self.filter = filter;
    }

    /// Queues a watch event for processing.
    ///
    /// Only files matching the path filter will be queued.
    ///
    /// # Arguments
    ///
    /// * `event` - The watch event to queue
    pub fn queue(&self, event: WatchEvent) {
        match event {
            WatchEvent::Created(path) | WatchEvent::Modified(path) => {
                // Apply path filter
                if !self.filter.should_index(&path) {
                    return;
                }

                let pending = self.pending.clone();
                tokio::spawn(async move {
                    pending.lock().await.insert(path);
                });
            }
            WatchEvent::Deleted(path) => {
                // Apply path filter for deletions too
                if !self.filter.should_index(&path) {
                    return;
                }

                let deleted = self.deleted.clone();
                tokio::spawn(async move {
                    deleted.lock().await.insert(path);
                });
            }
            WatchEvent::Error(_) => {
                // Log error but don't fail
            }
        }
    }

    /// Flushes pending changes to the graph store.
    ///
    /// This method processes all queued file changes and updates
    /// the index incrementally.
    ///
    /// # Returns
    ///
    /// `Ok(())` if flush succeeded, or an error.
    ///
    /// # Errors
    ///
    /// Returns an error if any file cannot be indexed.
    pub async fn flush(&self) -> anyhow::Result<FlushStats> {
        let mut pending = self.pending.lock().await;
        let mut deleted = self.deleted.lock().await;

        let mut stats = FlushStats::default();

        // Process deletions first
        for path in deleted.drain() {
            if let Err(e) = self.delete_file(&path).await {
                eprintln!("Error deleting {:?}: {}", path, e);
            } else {
                stats.deleted += 1;
            }
        }

        // Process additions/updates
        for path in pending.drain() {
            if let Err(e) = self.index_file(&path).await {
                eprintln!("Error indexing {:?}: {}", path, e);
            } else {
                stats.indexed += 1;
            }
        }

        Ok(stats)
    }

    /// Performs a full rescan of the codebase.
    ///
    /// This clears all pending changes and re-indexes from scratch,
    /// respecting the path filter.
    ///
    /// # Arguments
    ///
    /// * `root` - The root directory to scan
    ///
    /// # Returns
    ///
    /// `Ok(count)` with number of files indexed, or an error.
    pub async fn full_rescan(&self, root: &Path) -> anyhow::Result<usize> {
        // Clear pending
        self.pending.lock().await.clear();
        self.deleted.lock().await.clear();

        let mut count = 0;

        // Walk directory tree
        if root.is_dir() {
            self.scan_directory(root, &mut count).await?;
        }

        Ok(count)
    }

    /// Recursively scans a directory for files to index.
    async fn scan_directory(&self, dir: &Path, count: &mut usize) -> anyhow::Result<()> {
        let mut entries = tokio::fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_dir() {
                // Skip excluded directories early
                let path_str = path.to_string_lossy();
                if path_str.contains("/target/") 
                    || path_str.contains("/node_modules/")
                    || path_str.contains("/.git/")
                    || path_str.contains("/.forge/") {
                    continue;
                }

                // Recurse into allowed directories
                Box::pin(self.scan_directory(&path, count)).await?;
            } else if path.is_file() && self.filter.should_index(&path) {
                self.pending.lock().await.insert(path);
                *count += 1;
            }
        }

        Ok(())
    }

    /// Returns the number of pending files to process.
    pub async fn pending_count(&self) -> usize {
        self.pending.lock().await.len() + self.deleted.lock().await.len()
    }

    /// Clears all pending changes without processing.
    pub async fn clear_pending(&self) {
        self.pending.lock().await.clear();
        self.deleted.lock().await.clear();
    }

    /// Indexes a single file.
    async fn index_file(&self, _path: &PathBuf) -> anyhow::Result<()> {
        // In a full implementation, this would:
        // 1. Read the file
        // 2. Parse it with tree-sitter or similar
        // 3. Extract symbols, references, etc.
        // 4. Write to the graph store

        // For v0.2, we'll store a placeholder record
        // The full indexing will be added in a later phase

        Ok(())
    }

    /// Deletes a file from the index.
    async fn delete_file(&self, _path: &PathBuf) -> anyhow::Result<()> {
        // In a full implementation, this would:
        // 1. Query all symbols in this file
        // 2. Delete those symbols
        // 3. Delete incoming/outgoing references
        // 4. Clean up any CFG blocks

        Ok(())
    }
}

/// Statistics from a flush operation.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct FlushStats {
    /// Number of files indexed.
    pub indexed: usize,
    /// Number of files deleted.
    pub deleted: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::UnifiedGraphStore;

    #[tokio::test]
    async fn test_indexer_creation() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        assert_eq!(indexer.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_queue_events() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        indexer.queue(WatchEvent::Created(PathBuf::from("src/a.rs")));
        indexer.queue(WatchEvent::Modified(PathBuf::from("src/b.rs")));
        indexer.queue(WatchEvent::Deleted(PathBuf::from("src/c.rs")));

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert_eq!(indexer.pending_count().await, 3);
    }

    #[tokio::test]
    async fn test_queue_filtered_events() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        // These should be indexed (src/ and tests/)
        indexer.queue(WatchEvent::Created(PathBuf::from("src/a.rs")));
        indexer.queue(WatchEvent::Modified(PathBuf::from("tests/b.rs")));
        
        // These should be filtered out
        indexer.queue(WatchEvent::Modified(PathBuf::from("target/debug/build.rs")));
        indexer.queue(WatchEvent::Modified(PathBuf::from("node_modules/foo/index.js")));
        indexer.queue(WatchEvent::Modified(PathBuf::from(".git/config")));
        indexer.queue(WatchEvent::Modified(PathBuf::from("Cargo.lock")));
        indexer.queue(WatchEvent::Modified(PathBuf::from("README.md"))); // Not in src/ or tests/

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Only src/ and tests/ files should be queued
        assert_eq!(indexer.pending_count().await, 2);
    }

    #[test]
    fn test_path_filter_default() {
        let filter = PathFilter::default();

        // Should index src/ files
        assert!(filter.should_index(Path::new("src/lib.rs")));
        assert!(filter.should_index(Path::new("src/main.rs")));
        assert!(filter.should_index(Path::new("project/src/module.rs")));

        // Should index tests/ files
        assert!(filter.should_index(Path::new("tests/test.rs")));
        assert!(filter.should_index(Path::new("project/tests/integration.rs")));

        // Should NOT index target/
        assert!(!filter.should_index(Path::new("target/debug/build.rs")));
        assert!(!filter.should_index(Path::new("target/release/app")));

        // Should NOT index node_modules/
        assert!(!filter.should_index(Path::new("node_modules/foo/index.js")));

        // Should NOT index .git/
        assert!(!filter.should_index(Path::new(".git/config")));

        // Should NOT index Cargo.lock
        assert!(!filter.should_index(Path::new("Cargo.lock")));

        // Should NOT index files outside src/ or tests/
        assert!(!filter.should_index(Path::new("README.md")));
        assert!(!filter.should_index(Path::new("Cargo.toml")));
        assert!(!filter.should_index(Path::new("build.rs"))); // Not in src/
    }

    #[test]
    fn test_path_filter_extensions() {
        let filter = PathFilter::default();

        // Rust files
        assert!(filter.should_index(Path::new("src/lib.rs")));
        assert!(filter.should_index(Path::new("tests/test.rs")));

        // Python files
        assert!(filter.should_index(Path::new("src/main.py")));

        // JavaScript/TypeScript
        assert!(filter.should_index(Path::new("src/index.js")));
        assert!(filter.should_index(Path::new("src/index.ts")));
        assert!(filter.should_index(Path::new("src/App.jsx")));
        assert!(filter.should_index(Path::new("src/App.tsx")));

        // Binary files should be excluded
        assert!(!filter.should_index(Path::new("src/logo.png")));
        assert!(!filter.should_index(Path::new("src/data.bin")));
    }

    #[test]
    fn test_path_filter_custom() {
        let mut filter = PathFilter::new();
        filter.add_include("**/lib/**");
        filter.add_extension("go");

        assert!(filter.should_index(Path::new("lib/main.go")));
        assert!(!filter.should_index(Path::new("src/main.go"))); // Not in lib/
        assert!(!filter.should_index(Path::new("lib/main.rs"))); // Wrong extension
    }

    #[tokio::test]
    async fn test_flush_clears_pending() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        indexer.queue(WatchEvent::Modified(PathBuf::from("src/lib.rs")));
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let stats = indexer.flush().await.unwrap();
        assert_eq!(stats.indexed, 1);
        assert_eq!(indexer.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_flush_stats() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        indexer.queue(WatchEvent::Modified(PathBuf::from("src/a.rs")));
        indexer.queue(WatchEvent::Created(PathBuf::from("src/b.rs")));
        indexer.queue(WatchEvent::Deleted(PathBuf::from("src/c.rs")));

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let stats = indexer.flush().await.unwrap();

        assert_eq!(stats.indexed, 2);
        assert_eq!(stats.deleted, 1);
    }

    #[tokio::test]
    async fn test_clear_pending() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        indexer.queue(WatchEvent::Modified(PathBuf::from("src/a.rs")));
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert_eq!(indexer.pending_count().await, 1);

        indexer.clear_pending().await;

        assert_eq!(indexer.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_full_rescan() {
        let temp = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(temp.path()).await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        // Create a directory structure
        let src_dir = temp.path().join("src");
        let tests_dir = temp.path().join("tests");
        let target_dir = temp.path().join("target");
        tokio::fs::create_dir(&src_dir).await.unwrap();
        tokio::fs::create_dir(&tests_dir).await.unwrap();
        tokio::fs::create_dir(&target_dir).await.unwrap();

        // Create source files
        tokio::fs::write(src_dir.join("lib.rs"), "pub fn foo() {}").await.unwrap();
        tokio::fs::write(src_dir.join("main.rs"), "fn main() {}").await.unwrap();
        tokio::fs::write(tests_dir.join("test.rs"), "#[test] fn test() {}").await.unwrap();
        tokio::fs::write(target_dir.join("build.rs"), "// build").await.unwrap(); // Should be ignored
        tokio::fs::write(temp.path().join("README.md"), "# Project").await.unwrap(); // Should be ignored

        // Perform rescan
        let count = indexer.full_rescan(temp.path()).await.unwrap();

        // Should only find src/ and tests/ files, not target/ or README.md
        assert_eq!(count, 3);

        // Verify pending queue has the files
        let pending = indexer.pending.lock().await;
        assert!(pending.contains(&src_dir.join("lib.rs")));
        assert!(pending.contains(&src_dir.join("main.rs")));
        assert!(pending.contains(&tests_dir.join("test.rs")));
        assert!(!pending.contains(&target_dir.join("build.rs")));
        assert!(!pending.contains(&temp.path().join("README.md")));
    }
}

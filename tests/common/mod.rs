//! Common test utilities for ForgeKit integration tests.

use std::path::{Path, PathBuf};
use tempfile::TempDir;
use forge_core::{SymbolId, SymbolKind, Language, Location, Span};

/// Creates a test Forge instance with temporary storage.
///
/// # Examples
///
/// ```no_run
/// use tests_common::test_forge;
///
/// #[tokio::test]
/// async fn test_something() {
///     let (_temp, forge) = test_forge().await.unwrap();
///     // Use forge...
/// }
/// ```
pub async fn test_forge() -> anyhow::Result<(TempDir, forge_core::Forge)> {
    let temp = TempDir::new()?;
    let forge = forge_core::Forge::open(temp.path()).await?;
    Ok((temp, forge))
}

/// Creates a test file with the given content in the specified directory.
///
/// # Arguments
///
/// * `dir` - Directory to create the file in
/// * `name` - Name of the file to create
/// * `content` - Content to write to the file
///
/// # Returns
///
/// The full path to the created file
pub async fn create_test_file(dir: &Path, name: &str, content: &str) -> anyhow::Result<std::path::PathBuf> {
    let file_path = dir.join(name);
    tokio::fs::write(&file_path, content).await?;
    Ok(file_path)
}

/// Creates a test directory structure for a Rust project.
///
/// This creates a basic Cargo project structure:
/// - Cargo.toml with basic metadata
/// - src/main.rs with a simple main function
///
/// # Arguments
///
/// * `dir` - Directory to create the project in
/// * `name` - Name of the project
pub async fn create_test_rust_project(dir: &Path, name: &str) -> anyhow::Result<()> {
    let cargo_toml = format!(r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
"#, name);

    let main_rs = r#"fn main() {
    println!("Hello, world!");
}"#;

    create_test_file(dir, "Cargo.toml", &cargo_toml).await?;

    let src_dir = dir.join("src");
    tokio::fs::create_dir_all(&src_dir).await?;
    create_test_file(&src_dir, "main.rs", main_rs).await?;

    Ok(())
}

/// Creates a test Symbol with standard test values.
///
/// # Returns
///
/// A Symbol with:
/// - id: SymbolId(1)
/// - name: "test_function"
/// - fully_qualified_name: "my_crate::test_function"
/// - kind: SymbolKind::Function
/// - language: Language::Rust
/// - location: from test_location()
/// - parent_id: None
/// - metadata: serde_json::Value::Null
pub fn test_symbol() -> forge_core::Symbol {
    forge_core::Symbol {
        id: SymbolId(1),
        name: "test_function".to_string(),
        fully_qualified_name: "my_crate::test_function".to_string(),
        kind: SymbolKind::Function,
        language: Language::Rust,
        location: test_location(),
        parent_id: None,
        metadata: serde_json::Value::Null,
    }
}

/// Creates a test Location with standard test values.
///
/// # Returns
///
/// A Location with:
/// - file_path: PathBuf::from("src/test.rs")
/// - byte_start: 42
/// - byte_end: 84
/// - line_number: 7
pub fn test_location() -> Location {
    Location {
        file_path: PathBuf::from("src/test.rs"),
        byte_start: 42,
        byte_end: 84,
        line_number: 7,
    }
}

/// Creates a test Span with standard test values.
///
/// # Returns
///
/// A Span with:
/// - start: 10
/// - end: 50
pub fn test_span() -> Span {
    Span {
        start: 10,
        end: 50,
    }
}

/// Assert helper that verifies a Result is Err and contains expected substring.
///
/// # Arguments
///
/// * `result` - The Result to check
/// * `expected` - Substring expected to be in the error message
///
/// # Panics
///
/// - If result is Ok
/// - If error message doesn't contain expected substring
pub fn assert_error_variant<T>(result: anyhow::Result<T>, expected: &str) {
    match result {
        Err(e) => {
            let error_string = e.to_string();
            assert!(
                error_string.contains(expected),
                "Expected error containing '{}', got: {}",
                expected,
                error_string
            );
        }
        Ok(_) => panic!("Expected error, got Ok"),
    }
}

/// Async helper for polling conditions with timeout.
///
/// # Arguments
///
/// * `condition` - Closure that returns true when condition is met
/// * `timeout_ms` - Maximum time to wait in milliseconds
///
/// # Returns
///
/// Ok(()) if condition becomes true, Err if timeout expires
///
/// # Examples
///
/// ```no_run
/// use tests_common::wait_for;
///
/// #[tokio::test]
/// async fn test_something() {
///     let mut value = false;
///     tokio::spawn(async {
///         tokio::time::sleep(std::time::Duration::from_millis(100)).await;
///         value = true;
///     });
///
///     wait_for(|| value, 200).await.unwrap();
/// }
/// ```
pub async fn wait_for<F>(mut condition: F, timeout_ms: u64) -> anyhow::Result<()>
where
    F: FnMut() -> bool,
{
    use std::time::Instant;
    let start = Instant::now();
    while !condition() {
        if start.elapsed().as_millis() > timeout_ms as u128 {
            anyhow::bail!("Timeout waiting for condition");
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_test_forge() {
        let (_temp, forge) = test_forge().await.unwrap();
        // Just verify it was created successfully
        assert!(true);
    }

    #[tokio::test]
    async fn test_create_test_file() {
        let temp = TempDir::new().unwrap();
        let file_path = create_test_file(temp.path(), "test.txt", "Hello, World!").await.unwrap();

        assert!(file_path.exists());
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[tokio::test]
    async fn test_create_test_rust_project() {
        let temp = TempDir::new().unwrap();
        create_test_rust_project(temp.path(), "test_project").await.unwrap();

        assert!(temp.path().join("Cargo.toml").exists());
        assert!(temp.path().join("src/main.rs").exists());
    }

    #[test]
    fn test_test_symbol() {
        let symbol = test_symbol();
        assert_eq!(symbol.id.0, 1);
        assert_eq!(symbol.name, "test_function");
        assert_eq!(symbol.fully_qualified_name, "my_crate::test_function");
        assert_eq!(symbol.kind, SymbolKind::Function);
        assert_eq!(symbol.language, Language::Rust);
        assert!(symbol.parent_id.is_none());
    }

    #[test]
    fn test_test_location() {
        let location = test_location();
        assert_eq!(location.file_path, PathBuf::from("src/test.rs"));
        assert_eq!(location.byte_start, 42);
        assert_eq!(location.byte_end, 84);
        assert_eq!(location.line_number, 7);
    }

    #[test]
    fn test_test_span() {
        let span = test_span();
        assert_eq!(span.start, 10);
        assert_eq!(span.end, 50);
        assert_eq!(span.len(), 40);
    }

    #[test]
    fn test_assert_error_variant_success() {
        let result: anyhow::Result<()> = Err(anyhow::anyhow!("Something went wrong"));
        assert_error_variant(result, "wrong");
    }

    #[test]
    #[should_panic(expected = "Expected error containing")]
    fn test_assert_error_variant_missing_substring() {
        let result: anyhow::Result<()> = Err(anyhow::anyhow!("Something went wrong"));
        assert_error_variant(result, "missing");
    }

    #[test]
    #[should_panic(expected = "Expected error, got Ok")]
    fn test_assert_error_variant_ok_result() {
        let result: anyhow::Result<()> = Ok(());
        assert_error_variant(result, "error");
    }

    #[tokio::test]
    async fn test_wait_for_success() {
        let mut value = false;
        tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            value = true;
        });

        wait_for(|| value, 200).await.unwrap();
    }

    #[tokio::test]
    async fn test_wait_for_timeout() {
        let result = wait_for(|| false, 50).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Timeout"));
    }

    #[tokio::test]
    async fn test_wait_for_immediate() {
        let result = wait_for(|| true, 100).await;
        assert!(result.is_ok());
    }
}

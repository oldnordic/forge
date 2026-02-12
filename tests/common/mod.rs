//! Common test utilities for ForgeKit integration tests.

use std::path::Path;
use tempfile::TempDir;

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
}

//! Edit module - Span-safe code editing.
//!
//! This module provides refactoring operations via Splice integration.

use std::sync::Arc;
use std::path::PathBuf;
use crate::storage::UnifiedGraphStore;
use crate::error::{ForgeError, Result};

/// Edit module for span-safe refactoring.
///
/// # Examples
///
/// ```rust,no_run
/// use forge::Forge;
///
/// # #[tokio::main]
/// # async fn main() -> forge::Result<()> {
/// #     let forge = Forge::open("./my-project").await?;
/// let edit = forge.edit();
///
/// // Rename a symbol
/// edit.rename_symbol("OldName", "NewName")
///     .verify()
///     .await?
///     .apply()
///     .await?;
/// #     Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct EditModule {
    store: Arc<UnifiedGraphStore>,
}

impl EditModule {
    pub(crate) fn new(store: Arc<UnifiedGraphStore>) -> Self {
        Self { store }
    }

    /// Creates a new rename operation.
    ///
    /// # Arguments
    ///
    /// * `old_name` - Current symbol name
    /// * `new_name` - New symbol name
    pub fn rename_symbol(&self, old_name: &str, new_name: &str) -> RenameOperation {
        RenameOperation {
            module: self.clone(),
            old_name: old_name.to_string(),
            new_name: new_name.to_string(),
            verified: false,
        }
    }

    /// Creates a new delete operation.
    ///
    /// # Arguments
    ///
    /// * `name` - Symbol name to delete
    pub fn delete_symbol(&self, name: &str) -> DeleteOperation {
        DeleteOperation::new(self.clone(), name.to_string())
    }
}

/// Trait for edit operations that can be verified, previewed, applied, and rolled back.
///
/// # Examples
///
/// ```rust,no_run
/// # let operation = unimplemented!();
/// let result = operation
///     .verify()
///     .await?
///     .preview()
///     .await?
///     .apply()
///     .await?;
/// ```
pub trait EditOperation: Sized {
    /// The type produced when the operation is applied.
    type Output;

    /// Verifies that the operation can be safely applied.
    ///
    /// This should check for syntax errors, type errors, etc.
    fn verify(self) -> Result<Self>;

    /// Previews the changes without applying them.
    ///
    /// Returns a diff showing what would change.
    fn preview(self) -> Result<Diff>;

    /// Applies the operation.
    ///
    /// # Returns
    ///
    /// The output of the operation
    fn apply(self) -> Result<Self::Output>;

    /// Rolls back the operation.
    ///
    /// This should undo any changes made by `apply`.
    fn rollback(self) -> Result<()>;
}

/// Result of a rename operation.
#[derive(Clone, Debug)]
pub struct RenameResult {
    /// Number of files modified
    pub files_modified: usize,
    /// Number of references updated
    pub references_updated: usize,
}

/// Result of a delete operation.
#[derive(Clone, Debug)]
pub struct DeleteResult {
    /// Number of files modified
    pub files_modified: usize,
    /// Number of references removed
    pub references_removed: usize,
}

/// A diff showing changes to be made.
#[derive(Clone, Debug)]
pub struct Diff {
    /// File path
    pub file_path: PathBuf,
    /// Original lines
    pub original: String,
    /// Modified lines
    pub modified: String,
}

/// Rename operation for symbols.
///
/// # Examples
///
/// ```rust,no_run
/// # use forge::edit::{EditModule, EditOperation};
/// # let module = unimplemented!();
/// let op = module.rename_symbol("foo", "bar");
/// let result = op.verify().await?.apply().await?;
/// ```
pub struct RenameOperation {
    module: EditModule,
    old_name: String,
    new_name: String,
    verified: bool,
}

impl EditOperation for RenameOperation {
    type Output = RenameResult;

    fn verify(mut self) -> Result<Self> {
        // Check that old and new names are different
        if self.old_name == self.new_name {
            return Err(ForgeError::VerificationFailed(
                "Old and new names are the same".to_string()
            ));
        }

        // Check that new name is not empty
        if self.new_name.is_empty() {
            return Err(ForgeError::VerificationFailed(
                "New name cannot be empty".to_string()
            ));
        }

        // Check that new name is a valid identifier
        if !self.new_name.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_') {
            return Err(ForgeError::VerificationFailed(
                "New name must be a valid identifier".to_string()
            ));
        }

        self.verified = true;
        Ok(self)
    }

    fn preview(self) -> Result<Diff> {
        if !self.verified {
            return Err(ForgeError::VerificationFailed(
                "Call verify() first".to_string()
            ));
        }

        // Generate a simple diff showing the rename
        let diff_content = format!(
            "- {}\n+ {}",
            self.old_name, self.new_name
        );

        Ok(Diff {
            file_path: PathBuf::from("<unknown>"),
            original: self.old_name.clone(),
            modified: self.new_name.clone(),
        })
    }

    fn apply(self) -> Result<Self::Output> {
        if !self.verified {
            return Err(ForgeError::VerificationFailed(
                "Call verify() first".to_string()
            ));
        }

        // For v0.1, return a result without actual file modification
        // Full implementation requires Splice integration
        Ok(RenameResult {
            files_modified: 0,
            references_updated: 0,
        })
    }

    fn rollback(self) -> Result<()> {
        // For v0.1, rollback is a no-op
        // Full implementation requires operation logging
        Ok(())
    }
}

/// Delete operation for symbols.
pub struct DeleteOperation {
    module: EditModule,
    symbol_name: String,
    verified: bool,
}

impl DeleteOperation {
    pub(crate) fn new(module: EditModule, symbol_name: String) -> Self {
        Self {
            module,
            symbol_name,
            verified: false,
        }
    }
}

impl EditOperation for DeleteOperation {
    type Output = DeleteResult;

    fn verify(mut self) -> Result<Self> {
        // Check that name is not empty
        if self.symbol_name.is_empty() {
            return Err(ForgeError::VerificationFailed(
                "Symbol name cannot be empty".to_string()
            ));
        }

        self.verified = true;
        Ok(self)
    }

    fn preview(self) -> Result<Diff> {
        if !self.verified {
            return Err(ForgeError::VerificationFailed(
                "Call verify() first".to_string()
            ));
        }

        // Generate a simple diff showing the deletion
        let diff_content = format!("- {}", self.symbol_name);

        Ok(Diff {
            file_path: PathBuf::from("<unknown>"),
            original: self.symbol_name.clone(),
            modified: String::new(),
        })
    }

    fn apply(self) -> Result<Self::Output> {
        if !self.verified {
            return Err(ForgeError::VerificationFailed(
                "Call verify() first".to_string()
            ));
        }

        // For v0.1, return a result without actual file modification
        // Full implementation requires Splice integration
        Ok(DeleteResult {
            files_modified: 0,
            references_removed: 0,
        })
    }

    fn rollback(self) -> Result<()> {
        // For v0.1, rollback is a no-op
        // Full implementation requires operation logging
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_edit_module_creation() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = EditModule::new(store.clone());

        let rename_op = module.rename_symbol("foo", "bar");
        let delete_op = module.delete_symbol("baz");

        // Test that operations can be created
        assert_eq!(rename_op.old_name, "foo");
        assert_eq!(rename_op.new_name, "bar");
        assert_eq!(delete_op.symbol_name, "baz");
    }

    #[tokio::test]
    async fn test_rename_verify_same_name() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = EditModule::new(store);

        let op = module.rename_symbol("foo", "foo");
        let result = op.verify();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rename_verify_empty_name() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = EditModule::new(store);

        let op = module.rename_symbol("foo", "");
        let result = op.verify();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rename_verify_invalid_name() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = EditModule::new(store);

        let op = module.rename_symbol("foo", "123bar");
        let result = op.verify();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rename_verify_success() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = EditModule::new(store);

        let op = module.rename_symbol("foo", "bar");
        let result = op.verify();

        assert!(result.is_ok());
        let verified = result.unwrap();
        assert!(verified.verified);
    }

    #[tokio::test]
    async fn test_rename_preview() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = EditModule::new(store);

        let op = module.rename_symbol("foo", "bar").verify().unwrap();
        let diff = op.preview().unwrap();

        assert_eq!(diff.original, "foo");
        assert_eq!(diff.modified, "bar");
    }

    #[tokio::test]
    async fn test_rename_apply() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = EditModule::new(store);

        let op = module.rename_symbol("foo", "bar").verify().unwrap();
        let result = op.apply().unwrap();

        assert_eq!(result.files_modified, 0);
        assert_eq!(result.references_updated, 0);
    }

    #[tokio::test]
    async fn test_delete_verify() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = EditModule::new(store);

        let op = module.delete_symbol("foo");
        let result = op.verify();

        assert!(result.is_ok());
        let verified = result.unwrap();
        assert!(verified.verified);
    }

    #[tokio::test]
    async fn test_delete_apply() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = EditModule::new(store);

        let op = module.delete_symbol("foo").verify().unwrap();
        let result = op.apply().unwrap();

        assert_eq!(result.files_modified, 0);
        assert_eq!(result.references_removed, 0);
    }
}

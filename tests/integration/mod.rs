//! Integration tests for ForgeKit SDK.
//!
//! These tests verify the public API surface and cross-module interactions.

mod builder_tests;
mod accessor_tests;
mod runtime_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_integration_module_exists() {
        // Verify all integration modules compile
        assert!(true);
    }
}

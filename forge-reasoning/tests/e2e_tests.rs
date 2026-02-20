//! End-to-End (E2E) Tests for Temporal Checkpointing
//!
//! These tests exercise the full system from a user's perspective,
//! simulating real-world debugging workflows.

mod e2e;

// Re-export all e2e tests
pub use e2e::*;

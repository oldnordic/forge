//! End-to-End (E2E) Tests for Temporal Checkpointing
//!
//! These tests exercise the full system from a user's perspective,
//! simulating real-world debugging workflows.

pub mod e2e_checkpoint_workflow;
pub mod e2e_session_management;
pub mod e2e_data_integrity;
pub mod e2e_websocket_workflow;
pub mod e2e_recovery_scenarios;

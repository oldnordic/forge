//! Testing utilities for agent unit tests.
//!
//! Provides reusable mock tools and assertion helpers for testing
//! agent behavior without external dependencies.

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::Value;

use crate::chat::tools::registry::AsyncTool;
use crate::chat::tools::types::ToolDef;

#[derive(Debug, Clone)]
pub struct RecordedCall {
    pub arguments: Value,
    pub output: String,
    pub was_error: bool,
}

pub struct RecordingTool {
    name: String,
    response: String,
    calls: Arc<Mutex<Vec<RecordedCall>>>,
}

impl RecordingTool {
    pub fn new(name: impl Into<String>, response: impl Into<String>) -> Self {
        RecordingTool {
            name: name.into(),
            response: response.into(),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn call_count(&self) -> usize {
        self.calls.lock().len()
    }

    pub fn calls(&self) -> Vec<RecordedCall> {
        self.calls.lock().clone()
    }

    pub fn last_call(&self) -> Option<RecordedCall> {
        let calls = self.calls.lock();
        calls.last().cloned()
    }

    pub fn arguments_at(&self, index: usize) -> Option<Value> {
        let calls = self.calls.lock();
        calls.get(index).map(|c| c.arguments.clone())
    }
}

#[async_trait]
impl AsyncTool for RecordingTool {
    async fn call(&self, arguments: Value) -> Result<String, String> {
        let output = self.response.clone();
        self.calls.lock().push(RecordedCall {
            arguments,
            output: output.clone(),
            was_error: false,
        });
        Ok(output)
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: self.name.clone(),
            description: format!("Recording test tool: {}", self.name),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {"type": "string"}
                }
            }),
        }
    }
}

pub struct FailingTool {
    name: String,
    error_message: String,
}

impl FailingTool {
    pub fn new(name: impl Into<String>, error_message: impl Into<String>) -> Self {
        FailingTool {
            name: name.into(),
            error_message: error_message.into(),
        }
    }
}

#[async_trait]
impl AsyncTool for FailingTool {
    async fn call(&self, _arguments: Value) -> Result<String, String> {
        Err(self.error_message.clone())
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: self.name.clone(),
            description: format!("Failing test tool: {}", self.name),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {"type": "string"}
                }
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn recording_tool_captures_calls() {
        let tool = RecordingTool::new("test_tool", "ok");
        let result = tool
            .call(serde_json::json!({"input": "hello"}))
            .await
            .unwrap();
        assert_eq!(result, "ok");
        assert_eq!(tool.call_count(), 1);

        let call = tool.last_call().unwrap();
        assert_eq!(call.arguments["input"], "hello");
        assert_eq!(call.output, "ok");
        assert!(!call.was_error);
    }

    #[tokio::test]
    async fn recording_tool_multiple_calls() {
        let tool = RecordingTool::new("multi", "response");
        tool.call(serde_json::json!({"n": 1})).await.unwrap();
        tool.call(serde_json::json!({"n": 2})).await.unwrap();
        tool.call(serde_json::json!({"n": 3})).await.unwrap();

        assert_eq!(tool.call_count(), 3);
        assert_eq!(tool.arguments_at(0).unwrap()["n"], 1);
        assert_eq!(tool.arguments_at(1).unwrap()["n"], 2);
        assert_eq!(tool.arguments_at(2).unwrap()["n"], 3);
    }

    #[tokio::test]
    async fn failing_tool_returns_error() {
        let tool = FailingTool::new("fail_tool", "something went wrong");
        let result = tool.call(serde_json::json!({})).await;
        assert_eq!(result, Err("something went wrong".to_string()));
    }

    #[tokio::test]
    async fn recording_tool_definition() {
        let tool = RecordingTool::new("my_tool", "ok");
        let def = tool.definition();
        assert_eq!(def.name, "my_tool");
        assert!(def.description.contains("my_tool"));
    }

    #[tokio::test]
    async fn recording_tool_no_calls_initially() {
        let tool = RecordingTool::new("empty", "ok");
        assert_eq!(tool.call_count(), 0);
        assert!(tool.last_call().is_none());
        assert!(tool.arguments_at(0).is_none());
    }
}

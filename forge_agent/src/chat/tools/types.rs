use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ToolDef {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        ToolDef {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }

    pub fn empty(name: impl Into<String>, description: impl Into<String>) -> Self {
        ToolDef::new(
            name,
            description,
            serde_json::json!({"type": "object", "properties": {}}),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

impl ToolCall {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        ToolCall {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolOutput {
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn success(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        ToolOutput {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            is_error: false,
        }
    }

    pub fn error(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        ToolOutput {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            is_error: true,
        }
    }
}

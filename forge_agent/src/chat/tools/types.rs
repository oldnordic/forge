use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
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
#[non_exhaustive]
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
#[non_exhaustive]
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

    pub fn truncated(mut self, max_bytes: usize) -> Self {
        if self.content.len() > max_bytes {
            let truncated_len = self.content.len() - max_bytes;
            self.content.truncate(max_bytes);
            self.content
                .push_str(&format!("\n[... truncated {truncated_len} bytes ...]"));
        }
        self
    }
}

pub fn truncate_tool_output(content: &str, max_bytes: usize) -> String {
    if content.len() <= max_bytes {
        return content.to_string();
    }
    let truncated_len = content.len() - max_bytes;
    let mut result = content[..max_bytes].to_string();
    result.push_str(&format!("\n[... truncated {truncated_len} bytes ...]"));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_truncation_when_under_limit() {
        let output = ToolOutput::success("call_1", "hello").truncated(100);
        assert_eq!(output.content, "hello");
    }

    #[test]
    fn truncation_when_over_limit() {
        let long = "x".repeat(200);
        let output = ToolOutput::success("call_1", &long).truncated(100);
        assert!(output.content.contains("[... truncated 100 bytes ...]"));
        assert!(output.content.len() < 200);
    }

    #[test]
    fn truncation_exact_boundary() {
        let data = "x".repeat(100);
        let output = ToolOutput::success("call_1", &data).truncated(100);
        assert_eq!(output.content, "x".repeat(100));
    }

    #[test]
    fn free_function_truncate() {
        let long = "abcdefghij".repeat(10);
        let result = truncate_tool_output(&long, 50);
        assert!(result.contains("[... truncated 50 bytes ...]"));
    }

    #[test]
    fn free_function_no_truncate() {
        let result = truncate_tool_output("short", 100);
        assert_eq!(result, "short");
    }
}

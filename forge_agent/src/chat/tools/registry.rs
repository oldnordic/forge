use super::types::{ToolCall, ToolDef, ToolOutput};
use async_trait::async_trait;
use std::collections::HashMap;

#[async_trait]
pub trait AsyncTool: Send + Sync {
    async fn call(&self, arguments: serde_json::Value) -> Result<String, String>;
    fn definition(&self) -> ToolDef;
}

#[async_trait]
pub trait ToolRegistry: Send + Sync {
    async fn execute(&self, call: &ToolCall) -> ToolOutput;
    fn definitions(&self) -> Vec<ToolDef>;
    fn has_tool(&self, name: &str) -> bool;
}

pub struct BuiltinToolRegistry {
    tools: HashMap<String, Box<dyn AsyncTool>>,
}

impl BuiltinToolRegistry {
    pub fn new() -> Self {
        BuiltinToolRegistry {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn AsyncTool>) {
        let def = tool.definition();
        self.tools.insert(def.name, tool);
    }

    pub fn register_many(&mut self, tools: Vec<Box<dyn AsyncTool>>) {
        for tool in tools {
            self.register(tool);
        }
    }
}

impl Default for BuiltinToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolRegistry for BuiltinToolRegistry {
    async fn execute(&self, call: &ToolCall) -> ToolOutput {
        match self.tools.get(&call.name) {
            Some(tool) => match tool.call(call.arguments.clone()).await {
                Ok(result) => ToolOutput::success(&call.id, result),
                Err(err) => ToolOutput::error(&call.id, err),
            },
            None => ToolOutput::error(&call.id, format!("Unknown tool: {}", call.name)),
        }
    }

    fn definitions(&self) -> Vec<ToolDef> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}

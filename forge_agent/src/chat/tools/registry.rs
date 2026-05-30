use super::types::{ToolCall, ToolDef, ToolOutput};
use super::validation::validate_tool_arguments;
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
    defs_cache: Option<Vec<ToolDef>>,
}

impl BuiltinToolRegistry {
    pub fn new() -> Self {
        BuiltinToolRegistry {
            tools: HashMap::new(),
            defs_cache: None,
        }
    }

    pub fn register(&mut self, tool: Box<dyn AsyncTool>) {
        let def = tool.definition();
        self.tools.insert(def.name, tool);
        self.defs_cache = None;
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
            Some(tool) => {
                let def = tool.definition();
                if let Err(e) = validate_tool_arguments(&def, &call.arguments) {
                    return ToolOutput::error(&call.id, e);
                }
                match tool.call(call.arguments.clone()).await {
                    Ok(result) => ToolOutput::success(&call.id, result),
                    Err(err) => ToolOutput::error(&call.id, err),
                }
            }
            None => ToolOutput::error(&call.id, format!("Unknown tool: {}", call.name)),
        }
    }

    fn definitions(&self) -> Vec<ToolDef> {
        if let Some(ref cache) = self.defs_cache {
            return cache.clone();
        }
        self.tools.values().map(|t| t.definition()).collect()
    }

    fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}

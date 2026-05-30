pub mod builtins;
pub mod registry;
pub mod types;
pub mod validation;

pub use builtins::{default_builtin_tools, FileReadTool, FileWriteTool, ShellExecTool};
pub use registry::{AsyncTool, BuiltinToolRegistry, ToolRegistry};
pub use types::{ToolCall, ToolDef, ToolOutput};
pub use validation::validate_tool_arguments;

#[cfg(test)]
mod tests;

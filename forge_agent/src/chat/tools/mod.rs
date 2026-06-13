pub mod builtins;
pub mod registry;
pub mod types;
pub mod validation;

#[cfg(feature = "atheneum")]
pub mod atheneum_tool;
#[cfg(feature = "envoy")]
pub mod envoy_tool;

pub use builtins::{
    default_builtin_tools, default_builtin_tools_sandboxed, default_builtin_tools_with_graph,
    default_builtin_tools_with_graph_sandboxed, FileReadTool, FileWriteTool, GraphQueryTool,
    ShellExecTool,
};
pub use registry::{AsyncTool, BuiltinToolRegistry, ToolRegistry};
pub use types::{ToolCall, ToolDef, ToolOutput};
pub use validation::validate_tool_arguments;

#[cfg(feature = "atheneum")]
pub use atheneum_tool::AtheneumTool;
#[cfg(feature = "envoy")]
pub use envoy_tool::EnvoyTool;

#[cfg(test)]
mod tests;

pub mod conversation;
pub mod providers;
pub mod react;
pub mod stream;
pub mod tools;
pub mod types;

pub use conversation::Conversation;
#[cfg(feature = "llm-anthropic")]
pub use providers::AnthropicChatProvider;
#[cfg(feature = "llm-ollama")]
pub use providers::OllamaChatProvider;
#[cfg(feature = "llm-openai")]
pub use providers::OpenAiChatProvider;
pub use providers::{ChatProvider, LlmProviderAdapter, MockChatProvider, RetryProvider};
pub use react::{AgentError, ReActLoop};
pub use stream::StreamEvent;
pub use tools::{
    default_builtin_tools, AsyncTool, BuiltinToolRegistry, FileReadTool, FileWriteTool,
    ShellExecTool, ToolCall, ToolDef, ToolOutput, ToolRegistry,
};
pub use types::{ChatMessage, ChatResponse, ContentBlock, LlmError, Role, Usage};

#[cfg(test)]
mod react_tests;

#[cfg(test)]
mod stream_tests;

#[cfg(test)]
mod types_tests;

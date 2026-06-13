pub mod context_window;
pub mod conversation;
pub mod events;
pub mod hooks;
pub mod memory;
pub mod prompts;
pub mod providers;
pub mod react;
pub mod retrieval;
pub mod sandbox;
pub mod skills;
pub mod step;
pub mod stream;
pub mod testing;
pub mod token_tracker;
pub mod tools;
pub mod types;

pub use context_window::{estimate_tokens, ContextWindow, TrimStrategy};
pub use conversation::Conversation;
pub use memory::{ConversationStore, FileConversationStore, SessionMeta, StoredConversation};
pub use prompts::{FewShotExample, PromptLibrary, PromptTemplate};
#[cfg(feature = "llm-anthropic")]
pub use providers::AnthropicChatProvider;
#[cfg(feature = "llm-ollama")]
pub use providers::OllamaChatProvider;
#[cfg(feature = "llm-openai")]
pub use providers::OpenAiChatProvider;
pub use providers::{
    chat_structured, ChatProvider, ContextTrimmer, LlmProviderAdapter, MockChatProvider,
    RetryProvider,
};
pub use react::{AgentError, ReActLoop, VerifierFn};
pub use retrieval::{CodeRetriever, CodeSnippet, FileCodeRetriever, RetrievalSource};
pub use sandbox::{Sandbox, SharedSandbox};
pub use step::StepEvent;

pub use events::{AgentEvent, EventBus};
#[cfg(feature = "atheneum")]
pub use retrieval::AtheneumRetriever;
pub use stream::{ReactStreamEvent, StreamEvent};
pub use testing::{FailingTool, RecordedCall, RecordingTool};
pub use token_tracker::{TokenTracker, TokenUsage};
pub use tools::{
    default_builtin_tools, default_builtin_tools_sandboxed, default_builtin_tools_with_graph,
    default_builtin_tools_with_graph_sandboxed, AsyncTool, BuiltinToolRegistry, FileReadTool,
    FileWriteTool, GraphQueryTool, ShellExecTool, ToolCall, ToolDef, ToolOutput, ToolRegistry,
};
pub use types::{ChatMessage, ChatResponse, ContentBlock, LlmError, Role, Usage};

pub use hooks::{HookConfig, HookContext, HookEvent, HookGroup, HookResult, HookRunner, HookSpec};
pub use skills::{
    SkillContent, SkillLoader, SkillManifest, SkillMatch, SkillRegistry, SkillTool,
    MAX_INJECTED_BYTES, MIN_CONFIDENCE_SCORE,
};

#[cfg(feature = "atheneum")]
pub use tools::AtheneumTool;
#[cfg(feature = "envoy")]
pub use tools::EnvoyTool;

#[cfg(test)]
mod react_tests;

#[cfg(test)]
mod retrieval_tests;

#[cfg(test)]
mod stream_tests;

#[cfg(test)]
mod types_tests;

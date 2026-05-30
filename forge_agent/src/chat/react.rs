use std::sync::Arc;

use crate::chat::providers::ChatProvider;
use crate::chat::tools::registry::ToolRegistry;
use crate::chat::tools::types::ToolCall;
use crate::chat::types::{ChatMessage, ContentBlock, LlmError};
use crate::llm::LlmConfig;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("maximum iterations exceeded")]
    MaxIterations,

    #[error("provider error: {0}")]
    Provider(#[from] LlmError),

    #[error("tool error: {0}")]
    Tool(String),
}

pub struct ReActLoop<R: ToolRegistry> {
    provider: Arc<dyn ChatProvider>,
    registry: R,
    config: LlmConfig,
    max_iterations: usize,
    system_prompt: Option<String>,
}

impl<R: ToolRegistry> ReActLoop<R> {
    pub fn new(provider: Arc<dyn ChatProvider>, registry: R, config: LlmConfig) -> Self {
        ReActLoop {
            provider,
            registry,
            config,
            max_iterations: 10,
            system_prompt: None,
        }
    }

    pub fn with_max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub async fn run(&self, prompt: &str) -> Result<String, AgentError> {
        let mut conversation = crate::chat::conversation::Conversation::new();
        if let Some(ref prompt) = self.system_prompt {
            conversation.push(ChatMessage::system(prompt.clone()));
        }
        conversation.push(ChatMessage::user(prompt));

        for _ in 0..self.max_iterations {
            let tools = self.registry.definitions();
            let response = self
                .provider
                .chat(conversation.messages(), &tools, &self.config)
                .await?;

            conversation.push(response.message.clone());

            if !response.message.has_tool_calls() {
                return Ok(response.message.text().unwrap_or_default().to_string());
            }

            for block in &response.message.content {
                if let ContentBlock::ToolCall {
                    id,
                    name,
                    arguments,
                } = block
                {
                    let call = ToolCall::new(id.clone(), name.clone(), arguments.clone());
                    let output = self.registry.execute(&call).await;

                    if output.is_error {
                        conversation.push(ChatMessage::tool_error(
                            &output.tool_call_id,
                            &output.content,
                        ));
                    } else {
                        conversation.push(ChatMessage::tool_result(
                            &output.tool_call_id,
                            &output.content,
                        ));
                    }
                }
            }
        }

        Err(AgentError::MaxIterations)
    }
}

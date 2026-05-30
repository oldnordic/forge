use async_trait::async_trait;
use forge_agent::chat::conversation::Conversation;
use forge_agent::chat::providers::ollama::OllamaChatProvider;
use forge_agent::chat::providers::ChatProvider;
use forge_agent::chat::tools::registry::{AsyncTool, BuiltinToolRegistry, ToolRegistry};
use forge_agent::chat::tools::types::{ToolCall, ToolDef};
use forge_agent::chat::types::{ChatMessage, ContentBlock};
use forge_agent::llm::LlmConfig;

struct FileReadTool;

#[async_trait]
impl AsyncTool for FileReadTool {
    async fn call(&self, args: serde_json::Value) -> Result<String, String> {
        let path = args["path"].as_str().unwrap_or("");
        tokio::fs::read_to_string(path)
            .await
            .map_err(|e| format!("Failed to read {}: {}", path, e))
    }

    fn definition(&self) -> ToolDef {
        ToolDef::new(
            "file_read",
            "Read the contents of a file",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path to read"}
                },
                "required": ["path"]
            }),
        )
    }
}

#[tokio::main]
async fn main() {
    let mut registry = BuiltinToolRegistry::new();
    registry.register(Box::new(FileReadTool));

    let provider = OllamaChatProvider::local();
    let config = LlmConfig::new("qwen3.5-agent:latest").with_temperature(0.0);

    let mut conversation = Conversation::new();
    conversation.push(ChatMessage::system(
        "You are a helpful assistant. Use the file_read tool when asked about file contents. \
         After getting tool results, give a brief text answer. Do NOT make additional tool calls after you have the answer.",
    ));
    conversation.push(ChatMessage::user(
        "Read the file /home/feanor/Projects/forge/Cargo.toml and tell me the workspace members. Be brief.",
    ));

    let tools = registry.definitions();
    let max_iters = 6;

    for i in 0..max_iters {
        println!("\n=== ITERATION {} ===", i + 1);
        println!(
            "Sending {} messages to model...",
            conversation.messages().len()
        );

        let response = provider
            .chat(conversation.messages(), &tools, &config)
            .await
            .expect("chat failed");

        println!("Model response:");
        for block in &response.message.content {
            match block {
                ContentBlock::Text { text } => {
                    println!("  TEXT: {}", text.chars().take(200).collect::<String>())
                }
                ContentBlock::ToolCall {
                    id,
                    name,
                    arguments,
                } => {
                    println!("  TOOL_CALL: id={}, name={}, args={}", id, name, arguments);
                }
                ContentBlock::ToolResult {
                    tool_call_id,
                    content,
                    is_error,
                    ..
                } => {
                    println!(
                        "  TOOL_RESULT: id={}, error={}, content={}",
                        tool_call_id,
                        is_error,
                        content.chars().take(200).collect::<String>()
                    );
                }
            }
        }

        conversation.push(response.message.clone());

        if !response.message.has_tool_calls() {
            println!("\n=== FINAL ANSWER (no tool calls) ===");
            if let Some(text) = response.message.text() {
                println!("{}", text);
            }
            return;
        }

        for block in &response.message.content {
            if let ContentBlock::ToolCall {
                id,
                name,
                arguments,
            } = block
            {
                let call = ToolCall::new(id.clone(), name.clone(), arguments.clone());
                let output = registry.execute(&call).await;
                println!(
                    "\n  Executed {} -> error={}, content={}",
                    name,
                    output.is_error,
                    output.content.chars().take(150).collect::<String>()
                );

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

    println!("\n=== MAX ITERATIONS REACHED ===");
}

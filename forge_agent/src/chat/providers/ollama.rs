use crate::chat::providers::ChatProvider;
use crate::chat::stream::StreamEvent;
use crate::chat::tools::types::ToolDef;
use crate::chat::types::{ChatMessage, ChatResponse, ContentBlock, LlmError, Role, Usage};
use crate::llm::LlmConfig;
use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

pub struct OllamaChatProvider {
    client: reqwest::Client,
    endpoint: String,
}

impl OllamaChatProvider {
    pub fn new(endpoint: impl Into<String>) -> Self {
        OllamaChatProvider {
            client: reqwest::Client::new(),
            endpoint: endpoint.into(),
        }
    }

    pub fn local() -> Self {
        Self::new("http://localhost:11434")
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct OllamaToolCall {
    function: OllamaFunction,
}

#[derive(Serialize, Deserialize, Clone)]
struct OllamaFunction {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Serialize)]
struct OllamaToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize)]
struct OllamaTool {
    #[serde(rename = "type")]
    kind: &'static str,
    function: OllamaToolFunction,
}

#[derive(Serialize)]
struct OllamaMessage<'a> {
    role: &'a str,
    content: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OllamaToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_name: Option<&'a str>,
}

#[derive(Serialize)]
struct OllamaMessageOwned {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OllamaToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_name: Option<String>,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<OllamaMessage<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OllamaTool>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Serialize)]
struct ChatRequestOwned {
    model: String,
    messages: Vec<OllamaMessageOwned>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OllamaTool>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptionsOwned>,
}

#[derive(Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

#[derive(Serialize)]
struct OllamaOptionsOwned {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

#[derive(Deserialize)]
struct ChatResponseRaw {
    message: ChatMessageRaw,
    #[serde(default)]
    done_reason: Option<String>,
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
}

#[derive(Deserialize)]
struct ChatMessageRaw {
    #[allow(dead_code)]
    role: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Deserialize)]
struct StreamChunkRaw {
    #[serde(default)]
    message: Option<StreamMessageRaw>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
}

#[derive(Deserialize)]
struct StreamMessageRaw {
    #[serde(default)]
    content: String,
    #[serde(default)]
    #[allow(dead_code)]
    thinking: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

fn convert_message(msg: &ChatMessage) -> OllamaMessage<'_> {
    let role = match msg.role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    };

    let text = if msg.role == Role::Tool {
        msg.content
            .iter()
            .find_map(|b| match b {
                ContentBlock::ToolResult { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .unwrap_or("")
    } else {
        msg.text().unwrap_or("")
    };

    let tool_calls: Option<Vec<OllamaToolCall>> = if msg.has_tool_calls() {
        Some(
            msg.tool_calls()
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::ToolCall {
                        name, arguments, ..
                    } => Some(OllamaToolCall {
                        function: OllamaFunction {
                            name: name.clone(),
                            arguments: arguments.clone(),
                        },
                    }),
                    _ => None,
                })
                .collect(),
        )
    } else {
        None
    };

    let tool_name = msg.content.iter().find_map(|block| match block {
        ContentBlock::ToolResult { name, .. } => name.as_deref(),
        _ => None,
    });

    OllamaMessage {
        role,
        content: text,
        tool_calls,
        tool_name,
    }
}

fn convert_message_owned(msg: &ChatMessage) -> OllamaMessageOwned {
    let role = match msg.role {
        Role::System => "system".to_string(),
        Role::User => "user".to_string(),
        Role::Assistant => "assistant".to_string(),
        Role::Tool => "tool".to_string(),
    };

    let text = if msg.role == Role::Tool {
        msg.content
            .iter()
            .find_map(|b| match b {
                ContentBlock::ToolResult { content, .. } => Some(content.clone()),
                _ => None,
            })
            .unwrap_or_default()
    } else {
        msg.text().unwrap_or("").to_string()
    };

    let tool_calls: Option<Vec<OllamaToolCall>> = if msg.has_tool_calls() {
        Some(
            msg.tool_calls()
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::ToolCall {
                        name, arguments, ..
                    } => Some(OllamaToolCall {
                        function: OllamaFunction {
                            name: name.clone(),
                            arguments: arguments.clone(),
                        },
                    }),
                    _ => None,
                })
                .collect(),
        )
    } else {
        None
    };

    let tool_name = msg.content.iter().find_map(|block| match block {
        ContentBlock::ToolResult { name, .. } => name.clone(),
        _ => None,
    });

    OllamaMessageOwned {
        role,
        content: text,
        tool_calls,
        tool_name,
    }
}

fn convert_tool_def(def: &ToolDef) -> OllamaTool {
    OllamaTool {
        kind: "function",
        function: OllamaToolFunction {
            name: def.name.clone(),
            description: def.description.clone(),
            parameters: def.parameters.clone(),
        },
    }
}

fn build_options_owned(config: &LlmConfig) -> Option<OllamaOptionsOwned> {
    if config.temperature.is_some()
        || config.top_p.is_some()
        || !config.stop.is_empty()
        || config.max_tokens.is_some()
    {
        Some(OllamaOptionsOwned {
            temperature: config.temperature,
            top_p: config.top_p,
            stop: config.stop.clone(),
            num_predict: config.max_tokens,
        })
    } else {
        None
    }
}

#[async_trait]
impl ChatProvider for OllamaChatProvider {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDef],
        config: &LlmConfig,
    ) -> Result<ChatResponse, LlmError> {
        let ollama_messages: Vec<OllamaMessage<'_>> =
            messages.iter().map(convert_message).collect();
        let ollama_tools: Vec<OllamaTool> = tools.iter().map(convert_tool_def).collect();

        let mut format = None;
        if config.json_mode {
            format = Some(serde_json::json!("json"));
        }

        let options = if config.temperature.is_some()
            || config.top_p.is_some()
            || !config.stop.is_empty()
            || config.max_tokens.is_some()
        {
            Some(OllamaOptions {
                temperature: config.temperature,
                top_p: config.top_p,
                stop: config.stop.clone(),
                num_predict: config.max_tokens,
            })
        } else {
            None
        };

        let request = ChatRequest {
            model: &config.model,
            messages: ollama_messages,
            tools: ollama_tools,
            stream: false,
            format,
            options,
        };

        let resp = self
            .client
            .post(format!("{}/api/chat", self.endpoint))
            .json(&request)
            .send()
            .await
            .map_err(|e| LlmError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Http(format!("Ollama {}: {}", status, body)));
        }

        let raw: ChatResponseRaw = resp
            .json()
            .await
            .map_err(|e| LlmError::Parse(e.to_string()))?;

        let mut content_blocks: Vec<ContentBlock> = Vec::new();

        let text_content = if raw.message.content.is_empty() {
            raw.message.thinking.unwrap_or_default()
        } else {
            raw.message.content
        };

        if !text_content.is_empty() {
            content_blocks.push(ContentBlock::text(text_content));
        }

        if let Some(tool_calls) = raw.message.tool_calls {
            for (i, tc) in tool_calls.into_iter().enumerate() {
                let id = format!("ollama_call_{i}");
                content_blocks.push(ContentBlock::tool_call(
                    id,
                    tc.function.name,
                    tc.function.arguments,
                ));
            }
        }

        Ok(ChatResponse {
            message: ChatMessage {
                role: Role::Assistant,
                content: content_blocks,
            },
            usage: Usage {
                prompt_tokens: raw.prompt_eval_count,
                completion_tokens: raw.eval_count,
                total_tokens: raw
                    .prompt_eval_count
                    .zip(raw.eval_count)
                    .map(|(p, c)| p + c),
            },
            model: config.model.clone(),
            finish_reason: raw.done_reason,
        })
    }

    fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDef],
        config: &LlmConfig,
    ) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send>> {
        let endpoint = self.endpoint.clone();
        let client = self.client.clone();
        let model = config.model.clone();
        let json_mode = config.json_mode;

        let ollama_messages: Vec<OllamaMessageOwned> =
            messages.iter().map(convert_message_owned).collect();
        let ollama_tools: Vec<OllamaTool> = tools.iter().map(convert_tool_def).collect();
        let options = build_options_owned(config);

        let mut format = None;
        if json_mode {
            format = Some(serde_json::json!("json"));
        }

        let request = ChatRequestOwned {
            model,
            messages: ollama_messages,
            tools: ollama_tools,
            stream: true,
            format,
            options,
        };

        let response_future = client
            .post(format!("{}/api/chat", endpoint))
            .json(&request)
            .send();

        super::ndjson_stream::spawn_line_stream((), response_future, |_, line| {
            let parsed: StreamChunkRaw = match serde_json::from_str(line) {
                Ok(p) => p,
                Err(_) => return Vec::new(),
            };
            let mut events = Vec::new();
            if parsed.done {
                if parsed.prompt_eval_count.is_some() || parsed.eval_count.is_some() {
                    events.push(StreamEvent::Usage(Usage {
                        prompt_tokens: parsed.prompt_eval_count,
                        completion_tokens: parsed.eval_count,
                        total_tokens: parsed
                            .prompt_eval_count
                            .zip(parsed.eval_count)
                            .map(|(p, c)| p + c),
                    }));
                }
                events.push(StreamEvent::Done);
                return events;
            }
            if let Some(msg) = parsed.message {
                if !msg.content.is_empty() {
                    events.push(StreamEvent::Token(msg.content));
                }
                if let Some(tool_calls) = msg.tool_calls {
                    for (i, tc) in tool_calls.into_iter().enumerate() {
                        let id = format!("ollama_call_{i}");
                        events.push(StreamEvent::ToolCallStart {
                            index: i,
                            id,
                            name: tc.function.name,
                        });
                        let args_str = tc.function.arguments.to_string();
                        if !args_str.is_empty() && args_str != "null" {
                            events.push(StreamEvent::ToolCallArgumentDelta {
                                index: i,
                                delta: args_str,
                            });
                        }
                        events.push(StreamEvent::ToolCallEnd { index: i });
                    }
                }
            }
            events
        })
    }
}

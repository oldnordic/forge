use crate::chat::providers::ChatProvider;
use crate::chat::stream::StreamEvent;
use crate::chat::tools::types::ToolDef;
use crate::chat::types::{ChatMessage, ChatResponse, ContentBlock, LlmError, Role, Usage};
use crate::llm::LlmConfig;
use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

pub struct AnthropicChatProvider {
    client: reqwest::Client,
    endpoint: String,
    api_key: String,
    default_max_tokens: u32,
}

impl AnthropicChatProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        AnthropicChatProvider {
            client: reqwest::Client::new(),
            endpoint: "https://api.anthropic.com".to_string(),
            api_key: api_key.into(),
            default_max_tokens: 4096,
        }
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.default_max_tokens = tokens;
        self
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum AnthropicContentBlock {
    Text {
        #[serde(rename = "type")]
        kind: &'static str,
        text: String,
    },
    ToolUse {
        #[serde(rename = "type")]
        kind: &'static str,
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        #[serde(rename = "type")]
        kind: &'static str,
        tool_use_id: String,
        content: String,
    },
}

#[derive(Serialize)]
struct AnthropicToolDef {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessageBlock>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<AnthropicToolDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop_sequences: Vec<String>,
}

#[derive(Serialize)]
struct AnthropicStreamRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessageBlock>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<AnthropicToolDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop_sequences: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Serialize)]
struct AnthropicMessageBlock {
    role: String,
    content: Vec<AnthropicContentBlock>,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentResp>,
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    usage: Option<AnthropicUsageResp>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum AnthropicContentResp {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[derive(Deserialize)]
struct AnthropicUsageResp {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

fn convert_messages(messages: &[ChatMessage]) -> (Option<String>, Vec<AnthropicMessageBlock>) {
    let mut system_text = String::new();
    let mut blocks: Vec<AnthropicMessageBlock> = Vec::new();

    for msg in messages {
        match msg.role {
            Role::System => {
                if let Some(t) = msg.text() {
                    if !system_text.is_empty() {
                        system_text.push('\n');
                    }
                    system_text.push_str(t);
                }
            }
            Role::Tool => {
                let mut content: Vec<AnthropicContentBlock> = Vec::new();
                for block in &msg.content {
                    if let ContentBlock::ToolResult {
                        tool_call_id,
                        content: result_text,
                        ..
                    } = block
                    {
                        content.push(AnthropicContentBlock::ToolResult {
                            kind: "tool_result",
                            tool_use_id: tool_call_id.clone(),
                            content: result_text.clone(),
                        });
                    }
                }
                if let Some(last) = blocks.last_mut() {
                    if last.role == "user" {
                        last.content.extend(content);
                        continue;
                    }
                }
                blocks.push(AnthropicMessageBlock {
                    role: "user".to_string(),
                    content,
                });
            }
            _ => {
                let mut content: Vec<AnthropicContentBlock> = Vec::new();
                if let Some(t) = msg.text() {
                    if !t.is_empty() {
                        content.push(AnthropicContentBlock::Text {
                            kind: "text",
                            text: t.to_string(),
                        });
                    }
                }
                for block in &msg.content {
                    match block {
                        ContentBlock::ToolCall {
                            id,
                            name,
                            arguments,
                        } => {
                            content.push(AnthropicContentBlock::ToolUse {
                                kind: "tool_use",
                                id: id.clone(),
                                name: name.clone(),
                                input: arguments.clone(),
                            });
                        }
                        ContentBlock::Text { text } if !text.is_empty() => {
                            content.push(AnthropicContentBlock::Text {
                                kind: "text",
                                text: text.clone(),
                            });
                        }
                        _ => {}
                    }
                }
                if !content.is_empty() {
                    blocks.push(AnthropicMessageBlock {
                        role: match msg.role {
                            Role::User => "user".to_string(),
                            Role::Assistant => "assistant".to_string(),
                            _ => "user".to_string(),
                        },
                        content,
                    });
                }
            }
        }
    }

    let system = if system_text.is_empty() {
        None
    } else {
        Some(system_text)
    };
    (system, blocks)
}

fn convert_tool_def(def: &ToolDef) -> AnthropicToolDef {
    AnthropicToolDef {
        name: def.name.clone(),
        description: def.description.clone(),
        input_schema: def.parameters.clone(),
    }
}

#[async_trait]
impl ChatProvider for AnthropicChatProvider {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDef],
        config: &LlmConfig,
    ) -> Result<ChatResponse, LlmError> {
        let (system, anthropic_messages) = convert_messages(messages);
        let anthropic_tools: Vec<AnthropicToolDef> = tools.iter().map(convert_tool_def).collect();

        let max_tokens = config.max_tokens.unwrap_or(self.default_max_tokens);

        let request = AnthropicRequest {
            model: &config.model,
            max_tokens,
            system,
            messages: anthropic_messages,
            tools: anthropic_tools,
            temperature: config.temperature,
            top_p: config.top_p,
            stop_sequences: config.stop.clone(),
        };

        let resp = self
            .client
            .post(format!("{}/v1/messages", self.endpoint))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await
            .map_err(|e| LlmError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if status.as_u16() == 429 {
                return Err(LlmError::RateLimited { retry_after: None });
            }
            if body.contains("max_tokens") || body.contains("too many tokens") {
                return Err(LlmError::ContextLengthExceeded);
            }
            return Err(LlmError::Http(format!("Anthropic {}: {}", status, body)));
        }

        let raw: AnthropicResponse = resp
            .json()
            .await
            .map_err(|e| LlmError::Parse(e.to_string()))?;

        let mut content_blocks: Vec<ContentBlock> = Vec::new();

        for block in raw.content {
            match block {
                AnthropicContentResp::Text { text } => {
                    if !text.is_empty() {
                        content_blocks.push(ContentBlock::text(text));
                    }
                }
                AnthropicContentResp::ToolUse { id, name, input } => {
                    content_blocks.push(ContentBlock::tool_call(id, name, input));
                }
            }
        }

        let usage = raw
            .usage
            .map(|u| Usage {
                prompt_tokens: u.input_tokens,
                completion_tokens: u.output_tokens,
                total_tokens: u.input_tokens.zip(u.output_tokens).map(|(i, o)| i + o),
            })
            .unwrap_or_default();

        Ok(ChatResponse {
            message: ChatMessage {
                role: Role::Assistant,
                content: content_blocks,
            },
            usage,
            model: config.model.clone(),
            finish_reason: raw.stop_reason,
        })
    }

    fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDef],
        config: &LlmConfig,
    ) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send>> {
        let endpoint = self.endpoint.clone();
        let api_key = self.api_key.clone();
        let client = self.client.clone();
        let model = config.model.clone();
        let max_tokens = config.max_tokens.unwrap_or(self.default_max_tokens);
        let temperature = config.temperature;
        let top_p = config.top_p;
        let stop_sequences = config.stop.clone();

        let (system, anthropic_messages) = convert_messages(messages);
        let anthropic_tools: Vec<AnthropicToolDef> = tools.iter().map(convert_tool_def).collect();

        let request = AnthropicStreamRequest {
            model,
            max_tokens,
            system,
            messages: anthropic_messages,
            tools: anthropic_tools,
            temperature,
            top_p,
            stop_sequences,
            stream: true,
        };

        let response_future = client
            .post(format!("{}/v1/messages", endpoint))
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send();

        struct AnthropicSseState {
            event_type: String,
            data: String,
        }

        super::ndjson_stream::spawn_line_stream(
            AnthropicSseState {
                event_type: String::new(),
                data: String::new(),
            },
            response_future,
            |state, line| {
                if let Some(ev) = line.strip_prefix("event: ") {
                    state.event_type = ev.to_string();
                    return Vec::new();
                }
                if let Some(d) = line.strip_prefix("data: ") {
                    state.data = d.to_string();
                    return Vec::new();
                }
                if !line.is_empty() {
                    return Vec::new();
                }

                let event_type = std::mem::take(&mut state.event_type);
                let data = std::mem::take(&mut state.data);

                if data.is_empty() {
                    return Vec::new();
                }

                let parsed: serde_json::Value = match serde_json::from_str(&data) {
                    Ok(p) => p,
                    Err(_) => return Vec::new(),
                };

                let mut events = Vec::new();

                match event_type.as_str() {
                    "content_block_delta" => {
                        let delta_type = parsed["delta"]["type"].as_str().unwrap_or("");
                        match delta_type {
                            "text_delta" => {
                                if let Some(text) = parsed["delta"]["text"].as_str() {
                                    if !text.is_empty() {
                                        events.push(StreamEvent::Token(text.to_string()));
                                    }
                                }
                            }
                            "input_json_delta" => {
                                if let Some(json_str) = parsed["delta"]["partial_json"].as_str() {
                                    if let Some(index) = parsed["index"].as_u64() {
                                        events.push(StreamEvent::ToolCallArgumentDelta {
                                            index: index as usize,
                                            delta: json_str.to_string(),
                                        });
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    "content_block_start" => {
                        let block_type = parsed["content_block"]["type"].as_str().unwrap_or("");
                        if block_type == "tool_use" {
                            if let (Some(id), Some(name)) = (
                                parsed["content_block"]["id"].as_str(),
                                parsed["content_block"]["name"].as_str(),
                            ) {
                                let index = parsed["index"].as_u64().unwrap_or(0) as usize;
                                events.push(StreamEvent::ToolCallStart {
                                    index,
                                    id: id.to_string(),
                                    name: name.to_string(),
                                });
                            }
                        }
                    }
                    "content_block_stop" => {
                        if let Some(index) = parsed["index"].as_u64() {
                            events.push(StreamEvent::ToolCallEnd {
                                index: index as usize,
                            });
                        }
                    }
                    "message_delta" => {
                        if let Some(usage) = parsed.get("usage") {
                            events.push(StreamEvent::Usage(Usage {
                                prompt_tokens: None,
                                completion_tokens: usage["output_tokens"].as_u64(),
                                total_tokens: None,
                            }));
                        }
                    }
                    "message_stop" => {
                        events.push(StreamEvent::Done);
                    }
                    _ => {}
                }

                events
            },
        )
    }
}

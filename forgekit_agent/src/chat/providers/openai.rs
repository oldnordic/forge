use crate::chat::providers::ChatProvider;
use crate::chat::stream::StreamEvent;
use crate::chat::tools::types::ToolDef;
use crate::chat::types::{ChatMessage, ChatResponse, ContentBlock, LlmError, Role, Usage};
use crate::llm::LlmConfig;
use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

pub struct OpenAiChatProvider {
    client: reqwest::Client,
    endpoint: String,
    api_key: String,
}

impl OpenAiChatProvider {
    pub fn new(endpoint: impl Into<String>, api_key: impl Into<String>) -> Self {
        OpenAiChatProvider {
            client: reqwest::Client::new(),
            endpoint: endpoint.into(),
            api_key: api_key.into(),
        }
    }

    pub fn openai(api_key: impl Into<String>) -> Self {
        Self::new("https://api.openai.com/v1", api_key)
    }

    pub fn local_compatible(port: u16) -> Self {
        Self::new(format!("http://localhost:{port}/v1"), "local")
    }
}

#[derive(Serialize)]
struct OpenAiMessage<'a> {
    role: &'a str,
    content: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCallReq>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<&'a str>,
}

#[derive(Serialize)]
struct OpenAiMessageOwned {
    role: String,
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCallReq>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Serialize)]
struct OpenAiToolCallReq {
    id: String,
    #[serde(rename = "type")]
    kind: &'static str,
    function: OpenAiFunctionReq,
}

#[derive(Serialize)]
struct OpenAiFunctionReq {
    name: String,
    arguments: String,
}

#[derive(Serialize)]
struct OpenAiToolDef {
    #[serde(rename = "type")]
    kind: &'static str,
    function: OpenAiToolFunction,
}

#[derive(Serialize)]
struct OpenAiToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize)]
struct OpenAiRequest<'a> {
    model: &'a str,
    messages: Vec<OpenAiMessage<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAiToolDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<String>,
}

#[derive(Serialize)]
struct OpenAiRequestOwned {
    model: String,
    messages: Vec<OpenAiMessageOwned>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAiToolDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessageResp,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiMessageResp {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCallResp>>,
}

#[derive(Deserialize)]
struct OpenAiToolCallResp {
    id: String,
    function: OpenAiFunctionResp,
}

#[derive(Deserialize)]
struct OpenAiFunctionResp {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct OpenAiStreamChunk {
    #[serde(default)]
    choices: Vec<OpenAiStreamChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct OpenAiStreamChoice {
    #[serde(default)]
    delta: OpenAiStreamDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Default)]
struct OpenAiStreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiStreamToolCall>>,
}

#[derive(Deserialize)]
struct OpenAiStreamToolCall {
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<OpenAiStreamFunction>,
}

#[derive(Deserialize)]
struct OpenAiStreamFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

fn convert_message(msg: &ChatMessage) -> OpenAiMessage<'_> {
    let role = match msg.role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    };

    let content = if msg.role == Role::Tool {
        msg.content.iter().find_map(|b| match b {
            ContentBlock::ToolResult { content, .. } => Some(content.as_str()),
            _ => None,
        })
    } else {
        msg.text().filter(|s| !s.is_empty())
    };

    let tool_calls: Option<Vec<OpenAiToolCallReq>> = if msg.has_tool_calls() {
        Some(
            msg.tool_calls()
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::ToolCall {
                        id,
                        name,
                        arguments,
                    } => Some(OpenAiToolCallReq {
                        id: id.clone(),
                        kind: "function",
                        function: OpenAiFunctionReq {
                            name: name.clone(),
                            arguments: arguments.to_string(),
                        },
                    }),
                    _ => None,
                })
                .collect(),
        )
    } else {
        None
    };

    let tool_call_id = msg.content.iter().find_map(|block| match block {
        ContentBlock::ToolResult { tool_call_id, .. } => Some(tool_call_id.as_str()),
        _ => None,
    });

    OpenAiMessage {
        role,
        content,
        tool_calls,
        tool_call_id,
    }
}

fn convert_message_owned(msg: &ChatMessage) -> OpenAiMessageOwned {
    let role = match msg.role {
        Role::System => "system".to_string(),
        Role::User => "user".to_string(),
        Role::Assistant => "assistant".to_string(),
        Role::Tool => "tool".to_string(),
    };

    let content = if msg.role == Role::Tool {
        msg.content.iter().find_map(|b| match b {
            ContentBlock::ToolResult { content, .. } => Some(content.clone()),
            _ => None,
        })
    } else {
        msg.text().filter(|s| !s.is_empty()).map(|s| s.to_string())
    };

    let tool_calls: Option<Vec<OpenAiToolCallReq>> = if msg.has_tool_calls() {
        Some(
            msg.tool_calls()
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::ToolCall {
                        id,
                        name,
                        arguments,
                    } => Some(OpenAiToolCallReq {
                        id: id.clone(),
                        kind: "function",
                        function: OpenAiFunctionReq {
                            name: name.clone(),
                            arguments: arguments.to_string(),
                        },
                    }),
                    _ => None,
                })
                .collect(),
        )
    } else {
        None
    };

    let tool_call_id = msg.content.iter().find_map(|block| match block {
        ContentBlock::ToolResult { tool_call_id, .. } => Some(tool_call_id.clone()),
        _ => None,
    });

    OpenAiMessageOwned {
        role,
        content,
        tool_calls,
        tool_call_id,
    }
}

fn convert_tool_def(def: &ToolDef) -> OpenAiToolDef {
    OpenAiToolDef {
        kind: "function",
        function: OpenAiToolFunction {
            name: def.name.clone(),
            description: def.description.clone(),
            parameters: def.parameters.clone(),
        },
    }
}

#[async_trait]
impl ChatProvider for OpenAiChatProvider {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDef],
        config: &LlmConfig,
    ) -> Result<ChatResponse, LlmError> {
        let openai_messages: Vec<OpenAiMessage<'_>> =
            messages.iter().map(convert_message).collect();
        let openai_tools: Vec<OpenAiToolDef> = tools.iter().map(convert_tool_def).collect();

        let response_format = if config.json_mode {
            Some(serde_json::json!({"type": "json_object"}))
        } else {
            None
        };

        let request = OpenAiRequest {
            model: &config.model,
            messages: openai_messages,
            tools: openai_tools,
            response_format,
            temperature: config.temperature,
            max_tokens: config.max_tokens,
            top_p: config.top_p,
            stop: config.stop.clone(),
        };

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.endpoint))
            .bearer_auth(&self.api_key)
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
            if status.as_u16() == 400 && body.contains("context_length") {
                return Err(LlmError::ContextLengthExceeded);
            }
            return Err(LlmError::Http(format!("OpenAI {}: {}", status, body)));
        }

        let raw: OpenAiResponse = resp
            .json()
            .await
            .map_err(|e| LlmError::Parse(e.to_string()))?;

        let choice = raw
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| LlmError::Parse("OpenAI returned empty choices".to_string()))?;

        let mut content_blocks: Vec<ContentBlock> = Vec::new();

        if let Some(text) = choice.message.content {
            if !text.is_empty() {
                content_blocks.push(ContentBlock::text(text));
            }
        }

        if let Some(tool_calls) = choice.message.tool_calls {
            for tc in tool_calls {
                let arguments: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                    .unwrap_or_else(|e| {
                        tracing::warn!(
                            "OpenAI returned malformed tool arguments for {}: {e}",
                            tc.function.name
                        );
                        serde_json::json!({ "_parse_error": &tc.function.arguments })
                    });
                content_blocks.push(ContentBlock::tool_call(tc.id, tc.function.name, arguments));
            }
        }

        let usage = raw
            .usage
            .map(|u| Usage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            })
            .unwrap_or_default();

        Ok(ChatResponse {
            message: ChatMessage {
                role: Role::Assistant,
                content: content_blocks,
            },
            usage,
            model: config.model.clone(),
            finish_reason: choice.finish_reason,
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
        let json_mode = config.json_mode;
        let temperature = config.temperature;
        let max_tokens = config.max_tokens;
        let top_p = config.top_p;
        let stop = config.stop.clone();

        let openai_messages: Vec<OpenAiMessageOwned> =
            messages.iter().map(convert_message_owned).collect();
        let openai_tools: Vec<OpenAiToolDef> = tools.iter().map(convert_tool_def).collect();

        let response_format = if json_mode {
            Some(serde_json::json!({"type": "json_object"}))
        } else {
            None
        };

        let request = OpenAiRequestOwned {
            model,
            messages: openai_messages,
            tools: openai_tools,
            response_format,
            temperature,
            max_tokens,
            top_p,
            stop,
            stream: true,
        };

        let response_future = client
            .post(format!("{}/chat/completions", endpoint))
            .bearer_auth(&api_key)
            .json(&request)
            .send();

        struct OpenAiStreamState {
            last_tool_index: Option<usize>,
        }

        super::ndjson_stream::spawn_line_stream(
            OpenAiStreamState {
                last_tool_index: None,
            },
            response_future,
            |state, line| {
                let line = line.trim();
                if !line.starts_with("data: ") {
                    return Vec::new();
                }
                let data = &line[6..];
                if data == "[DONE]" {
                    let mut events = Vec::new();
                    if let Some(idx) = state.last_tool_index.take() {
                        events.push(StreamEvent::ToolCallEnd { index: idx });
                    }
                    events.push(StreamEvent::Done);
                    return events;
                }

                let parsed: OpenAiStreamChunk = match serde_json::from_str(data) {
                    Ok(p) => p,
                    Err(_) => return Vec::new(),
                };

                let mut events = Vec::new();

                if let Some(usage) = parsed.usage {
                    events.push(StreamEvent::Usage(Usage {
                        prompt_tokens: usage.prompt_tokens,
                        completion_tokens: usage.completion_tokens,
                        total_tokens: usage.total_tokens,
                    }));
                }

                for choice in parsed.choices {
                    if let Some(content) = &choice.delta.content {
                        if !content.is_empty() {
                            events.push(StreamEvent::Token(content.clone()));
                        }
                    }
                    if let Some(tool_calls) = choice.delta.tool_calls {
                        for tc in tool_calls {
                            let idx = tc.index.unwrap_or(0);
                            if let Some(id) = tc.id {
                                if let Some(prev_idx) = state.last_tool_index.take() {
                                    events.push(StreamEvent::ToolCallEnd { index: prev_idx });
                                }
                                let name = tc
                                    .function
                                    .as_ref()
                                    .and_then(|f| f.name.clone())
                                    .unwrap_or_default();
                                events.push(StreamEvent::ToolCallStart {
                                    index: idx,
                                    id,
                                    name,
                                });
                                state.last_tool_index = Some(idx);
                            }
                            if let Some(func) = &tc.function {
                                if let Some(args) = &func.arguments {
                                    if !args.is_empty() {
                                        events.push(StreamEvent::ToolCallArgumentDelta {
                                            index: idx,
                                            delta: args.clone(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                    if choice.finish_reason.is_some() {
                        if let Some(idx) = state.last_tool_index.take() {
                            events.push(StreamEvent::ToolCallEnd { index: idx });
                        }
                    }
                }

                events
            },
        )
    }
}

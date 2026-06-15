use std::sync::Arc;

use futures::Stream;
use futures::StreamExt;
use std::pin::Pin;
use tracing::{debug, info, info_span, warn};

use crate::chat::events::EventBus;
use crate::chat::hooks::{HookContext, HookEvent, HookRunner};
use crate::chat::providers::ChatProvider;
use crate::chat::retrieval::CodeRetriever;
use crate::chat::step::StepEvent;
use crate::chat::stream::{ReactStreamEvent, StreamEvent};
use crate::chat::tools::registry::ToolRegistry;
use crate::chat::tools::types::ToolCall;
use crate::chat::types::{ChatMessage, ContentBlock, LlmError, Usage};
use crate::llm::LlmConfig;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("maximum iterations exceeded")]
    MaxIterations,

    #[error("provider error: {0}")]
    Provider(#[from] LlmError),

    #[error("tool error: {0}")]
    Tool(String),

    #[error("hook blocked: {0}")]
    HookBlocked(String),

    #[error("ReAct failed: {0}")]
    ReActFailed(String),
}

pub type VerifierFn = Arc<dyn Fn(&str) -> bool + Send + Sync>;

type StepSender = futures::channel::mpsc::UnboundedSender<StepEvent>;

struct LlmResponse {
    text: String,
    tool_calls: Vec<ContentBlock>,
    usage: Usage,
}

pub struct ReActLoop<R: ToolRegistry> {
    provider: Arc<dyn ChatProvider>,
    registry: R,
    config: LlmConfig,
    max_iterations: usize,
    step_retries: usize,
    system_prompt: Option<String>,
    hook_runner: Option<HookRunner>,
    verifier: Option<VerifierFn>,
    retriever: Option<Arc<dyn CodeRetriever>>,
    retrieval_top_k: usize,
    event_bus: Option<EventBus>,
}

impl<R: ToolRegistry> ReActLoop<R> {
    pub fn new(provider: Arc<dyn ChatProvider>, registry: R, config: LlmConfig) -> Self {
        ReActLoop {
            provider,
            registry,
            config,
            max_iterations: 10,
            step_retries: 2,
            system_prompt: None,
            hook_runner: None,
            verifier: None,
            retriever: None,
            retrieval_top_k: 5,
            event_bus: None,
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

    pub fn with_hooks(mut self, runner: HookRunner) -> Self {
        self.hook_runner = Some(runner);
        self
    }

    pub fn with_step_retries(mut self, retries: usize) -> Self {
        self.step_retries = retries;
        self
    }

    pub fn with_verifier(mut self, verifier: VerifierFn) -> Self {
        self.verifier = Some(verifier);
        self
    }

    pub fn with_retriever(mut self, retriever: Arc<dyn CodeRetriever>) -> Self {
        self.retriever = Some(retriever);
        self
    }

    pub fn with_retrieval_top_k(mut self, k: usize) -> Self {
        self.retrieval_top_k = k;
        self
    }

    pub fn with_event_bus(mut self, bus: EventBus) -> Self {
        self.event_bus = Some(bus);
        self
    }

    async fn emit(&self, tx: &StepSender, event: StepEvent) {
        if let Some(ref bus) = self.event_bus {
            if let Some(agent_event) = event.to_agent_event() {
                bus.emit(&agent_event).await;
            }
        }
        let _ = tx.unbounded_send(event);
    }

    async fn check_pre_tool_hooks(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Option<String> {
        if let Some(ref hooks) = self.hook_runner {
            let command = arguments.get("command").and_then(|v| v.as_str());
            let ctx = HookContext::for_tool_call(tool_name, command);
            let results = hooks.run_hooks(&HookEvent::PreToolUse, &ctx).await;
            let blocked: Vec<&str> = results
                .iter()
                .filter_map(|r| match r {
                    crate::chat::hooks::HookResult::Blocked(msg) => Some(msg.as_str()),
                    _ => None,
                })
                .collect();
            if !blocked.is_empty() {
                return Some(blocked.join("; "));
            }
        }
        None
    }

    async fn run_post_tool_hooks(&self, tool_name: &str, output: &str) {
        if let Some(ref hooks) = self.hook_runner {
            let ctx = HookContext::for_tool_call(tool_name, Some(output));
            hooks.run_hooks(&HookEvent::PostToolUse, &ctx).await;
        }
    }

    async fn run_stop_hooks(&self, answer: Option<&str>) {
        if let Some(ref hooks) = self.hook_runner {
            let ctx = HookContext::for_stop(answer);
            hooks.run_hooks(&HookEvent::Stop, &ctx).await;
        }
    }

    async fn run_session_start_hooks(&self) {
        if let Some(ref hooks) = self.hook_runner {
            let ctx = HookContext::for_session_start();
            hooks.run_hooks(&HookEvent::SessionStart, &ctx).await;
        }
    }

    async fn call_llm_batch(
        &self,
        messages: &[ChatMessage],
        tools: &[crate::chat::tools::types::ToolDef],
    ) -> Result<LlmResponse, LlmError> {
        let response = self.provider.chat(messages, tools, &self.config).await?;
        let text = response.message.text().unwrap_or_default().to_string();
        let tool_calls: Vec<ContentBlock> = response
            .message
            .content
            .into_iter()
            .filter(|b| matches!(b, ContentBlock::ToolCall { .. }))
            .collect();
        Ok(LlmResponse {
            text,
            tool_calls,
            usage: response.usage,
        })
    }

    async fn call_llm_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[crate::chat::tools::types::ToolDef],
        tx: &StepSender,
    ) -> Result<LlmResponse, String> {
        let mut event_stream = self.provider.chat_stream(messages, tools, &self.config);

        let mut collected_tokens = String::new();
        let mut collected_tool_calls: Vec<ContentBlock> = Vec::new();
        let mut current_tool_id: Option<String> = None;
        let mut current_tool_name: Option<String> = None;
        let mut current_tool_args = String::new();
        let mut accumulated_usage = Usage {
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
        };

        while let Some(event) = event_stream.next().await {
            match &event {
                StreamEvent::Token(token) => {
                    collected_tokens.push_str(token);
                }
                StreamEvent::ToolCallStart { id, name, .. } => {
                    if let Some(prev_id) = current_tool_id.take() {
                        let empty = serde_json::Value::Object(serde_json::Map::new());
                        let args: serde_json::Value =
                            serde_json::from_str(&current_tool_args).unwrap_or(empty);
                        collected_tool_calls.push(ContentBlock::tool_call(
                            prev_id,
                            current_tool_name.take().unwrap_or_default(),
                            args,
                        ));
                        current_tool_args.clear();
                    }
                    current_tool_id = Some(id.clone());
                    current_tool_name = Some(name.clone());
                }
                StreamEvent::ToolCallArgumentDelta { delta, .. } => {
                    current_tool_args.push_str(delta);
                }
                StreamEvent::Error(e) => {
                    return Err(e.clone());
                }
                StreamEvent::Usage(ref u) => {
                    if let Some(pt) = u.prompt_tokens {
                        accumulated_usage.prompt_tokens =
                            Some(accumulated_usage.prompt_tokens.unwrap_or(0) + pt);
                    }
                    if let Some(ct) = u.completion_tokens {
                        accumulated_usage.completion_tokens =
                            Some(accumulated_usage.completion_tokens.unwrap_or(0) + ct);
                    }
                    if let Some(tt) = u.total_tokens {
                        accumulated_usage.total_tokens =
                            Some(accumulated_usage.total_tokens.unwrap_or(0) + tt);
                    }
                }
                StreamEvent::Done | StreamEvent::ToolCallEnd { .. } => {}
            }

            self.emit(tx, StepEvent::LlmStreamEvent { event }).await;
        }

        if let Some(id) = current_tool_id.take() {
            let empty = serde_json::Value::Object(serde_json::Map::new());
            let args: serde_json::Value = serde_json::from_str(&current_tool_args).unwrap_or(empty);
            collected_tool_calls.push(ContentBlock::tool_call(
                id,
                current_tool_name.take().unwrap_or_default(),
                args,
            ));
        }

        Ok(LlmResponse {
            text: collected_tokens,
            tool_calls: collected_tool_calls,
            usage: accumulated_usage,
        })
    }

    async fn execute_tools(
        &self,
        iteration: usize,
        tool_calls: &[ContentBlock],
        conversation: &mut crate::chat::conversation::Conversation,
        tx: &StepSender,
    ) {
        let max_tool_output_bytes = self.config.max_tool_output_bytes;

        for block in tool_calls {
            if let ContentBlock::ToolCall {
                id,
                name,
                arguments,
            } = block
            {
                if let Some(reason) = self.check_pre_tool_hooks(name, arguments).await {
                    self.emit(
                        tx,
                        StepEvent::PreToolHookBlocked {
                            tool_name: name.clone(),
                            reason,
                        },
                    )
                    .await;
                    conversation.push(ChatMessage::tool_error(id, "Blocked by hook policy"));
                    continue;
                }

                self.emit(
                    tx,
                    StepEvent::ToolCallStarted {
                        iteration,
                        tool_name: name.clone(),
                        tool_call_id: id.clone(),
                    },
                )
                .await;

                let call = ToolCall::new(id.clone(), name.clone(), arguments.clone());
                let output = self.registry.execute(&call).await;
                let was_truncated = output.content.len() > max_tool_output_bytes;
                let output = output.truncated(max_tool_output_bytes);

                let preview: String = output.content.chars().take(200).collect();
                self.emit(
                    tx,
                    StepEvent::ToolCallCompleted {
                        iteration,
                        tool_name: name.clone(),
                        tool_call_id: id.clone(),
                        success: !output.is_error,
                        output_bytes: output.content.len(),
                        truncated: was_truncated,
                        output_preview: preview,
                    },
                )
                .await;

                debug!(
                    iteration,
                    tool = name.as_str(),
                    success = !output.is_error,
                    output_bytes = output.content.len(),
                    "tool call completed"
                );

                self.run_post_tool_hooks(name, &output.content).await;

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

    async fn run_core(
        &self,
        prompt: &str,
        tx: &StepSender,
        use_streaming: bool,
    ) -> Result<String, AgentError> {
        let span = info_span!("react_loop", max_iterations = self.max_iterations);
        let _enter = span.enter();

        let mut conversation = crate::chat::conversation::Conversation::new();
        if let Some(ref sys) = self.system_prompt {
            conversation.push(ChatMessage::system(sys.clone()));
        }

        if let Some(ref retriever) = self.retriever {
            let snippets = retriever.retrieve(prompt, self.retrieval_top_k).await;
            if !snippets.is_empty() {
                let mut context = String::from("Relevant code context:\n");
                for snippet in &snippets {
                    context.push_str(&format!("---\n{}\n", snippet));
                }
                conversation.push(ChatMessage::system(context));
                self.emit(
                    tx,
                    StepEvent::RetrievalInjected {
                        num_snippets: snippets.len(),
                    },
                )
                .await;
            }
        }

        conversation.push(ChatMessage::user(prompt));

        self.run_session_start_hooks().await;

        let session_id = conversation.session_id().unwrap_or("unknown").to_string();
        self.emit(tx, StepEvent::SessionStarted { session_id })
            .await;

        let mut consecutive_errors: usize = 0;

        for iteration in 0..self.max_iterations {
            debug!(iteration, "starting iteration");

            self.emit(
                tx,
                StepEvent::IterationStarted {
                    iteration,
                    max_iterations: self.max_iterations,
                },
            )
            .await;

            let tools = self.registry.definitions();

            let llm_result = if use_streaming {
                match self
                    .call_llm_stream(conversation.messages(), &tools, tx)
                    .await
                {
                    Ok(resp) => resp,
                    Err(error_msg) => {
                        consecutive_errors += 1;
                        self.emit(
                            tx,
                            StepEvent::LlmError {
                                iteration,
                                consecutive_errors,
                                error: error_msg.clone(),
                            },
                        )
                        .await;
                        if consecutive_errors > self.step_retries {
                            return Err(AgentError::Provider(LlmError::Http(error_msg)));
                        }
                        conversation.push(ChatMessage::assistant(format!(
                            "I encountered an error: {error_msg}. Let me try again."
                        )));
                        continue;
                    }
                }
            } else {
                match self.call_llm_batch(conversation.messages(), &tools).await {
                    Ok(resp) => resp,
                    Err(e) => {
                        consecutive_errors += 1;
                        let error_msg = format!("{e}");
                        warn!(iteration, consecutive_errors, "LLM error, retrying");
                        self.emit(
                            tx,
                            StepEvent::LlmError {
                                iteration,
                                consecutive_errors,
                                error: error_msg.clone(),
                            },
                        )
                        .await;
                        if consecutive_errors > self.step_retries {
                            return Err(AgentError::Provider(e));
                        }
                        conversation.push(ChatMessage::assistant(format!(
                            "I encountered an error: {error_msg}. Let me try again."
                        )));
                        continue;
                    }
                }
            };

            consecutive_errors = 0;

            self.emit(
                tx,
                StepEvent::LlmResponseReceived {
                    iteration,
                    usage: llm_result.usage.clone(),
                    has_tool_calls: !llm_result.tool_calls.is_empty(),
                },
            )
            .await;

            if use_streaming {
                let mut assistant_content: Vec<ContentBlock> = Vec::new();
                if !llm_result.text.is_empty() {
                    assistant_content.push(ContentBlock::text(&llm_result.text));
                }
                assistant_content.extend(llm_result.tool_calls.clone());
                conversation.push(ChatMessage {
                    role: crate::chat::types::Role::Assistant,
                    content: assistant_content,
                });
            } else {
                let mut content = vec![ContentBlock::text(&llm_result.text)];
                content.extend(llm_result.tool_calls.clone());
                conversation.push(ChatMessage {
                    role: crate::chat::types::Role::Assistant,
                    content,
                });
            }

            if !llm_result.tool_calls.is_empty() {
                self.execute_tools(iteration, &llm_result.tool_calls, &mut conversation, tx)
                    .await;
                continue;
            }

            let answer = conversation
                .messages()
                .last()
                .and_then(|m| m.text())
                .unwrap_or("")
                .to_string();

            if let Some(ref verifier) = self.verifier {
                if !verifier(&answer) {
                    self.emit(tx, StepEvent::VerificationFailed { iteration })
                        .await;
                    conversation.push(ChatMessage::user(
                        "Your previous answer did not pass verification. Please try a different approach.",
                    ));
                    continue;
                }
            }

            info!(iteration, answer_length = answer.len(), "answer produced");
            self.run_stop_hooks(Some(&answer)).await;
            self.emit(tx, StepEvent::AnswerProduced { iteration, answer })
                .await;
            return Ok(conversation
                .messages()
                .last()
                .and_then(|m| m.text())
                .unwrap_or("")
                .to_string());
        }

        warn!(iterations = self.max_iterations, "max iterations reached");
        self.run_stop_hooks(None).await;
        self.emit(
            tx,
            StepEvent::MaxIterationsReached {
                iterations: self.max_iterations,
            },
        )
        .await;
        Err(AgentError::MaxIterations)
    }

    pub async fn run(&self, prompt: &str) -> Result<String, AgentError> {
        let (tx, _rx) = futures::channel::mpsc::unbounded::<StepEvent>();
        self.run_core(prompt, &tx, false).await
    }

    pub fn run_stream(
        self,
        prompt: impl Into<String>,
    ) -> Pin<Box<dyn Stream<Item = ReactStreamEvent> + Send>>
    where
        R: 'static,
    {
        let (tx, rx) = futures::channel::mpsc::unbounded::<StepEvent>();
        let prompt = prompt.into();

        tokio::spawn(async move {
            let _ = self.run_core(&prompt, &tx, true).await;
            drop(tx);
        });

        let stream = rx.filter_map(|step| async move { step.to_stream_event() });

        Box::pin(stream)
    }
}

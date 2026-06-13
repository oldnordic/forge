use crate::chat::stream::StreamEvent;
use crate::chat::types::Usage;

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum StepEvent {
    SessionStarted {
        session_id: String,
    },
    IterationStarted {
        iteration: usize,
        max_iterations: usize,
    },
    RetrievalInjected {
        num_snippets: usize,
    },
    LlmError {
        iteration: usize,
        consecutive_errors: usize,
        error: String,
    },
    LlmResponseReceived {
        iteration: usize,
        usage: Usage,
        has_tool_calls: bool,
    },
    LlmStreamEvent {
        event: StreamEvent,
    },
    PreToolHookBlocked {
        tool_name: String,
        reason: String,
    },
    ToolCallStarted {
        iteration: usize,
        tool_name: String,
        tool_call_id: String,
    },
    ToolCallCompleted {
        iteration: usize,
        tool_name: String,
        tool_call_id: String,
        success: bool,
        output_bytes: usize,
        truncated: bool,
        output_preview: String,
    },
    VerificationFailed {
        iteration: usize,
    },
    AnswerProduced {
        iteration: usize,
        answer: String,
    },
    MaxIterationsReached {
        iterations: usize,
    },
}

impl StepEvent {
    pub fn iteration(&self) -> Option<usize> {
        match self {
            StepEvent::IterationStarted { iteration, .. }
            | StepEvent::LlmError { iteration, .. }
            | StepEvent::LlmResponseReceived { iteration, .. }
            | StepEvent::ToolCallStarted { iteration, .. }
            | StepEvent::ToolCallCompleted { iteration, .. }
            | StepEvent::VerificationFailed { iteration, .. }
            | StepEvent::AnswerProduced { iteration, .. } => Some(*iteration),
            _ => None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            StepEvent::AnswerProduced { .. } | StepEvent::MaxIterationsReached { .. }
        )
    }

    pub fn to_agent_event(&self) -> Option<crate::chat::events::AgentEvent> {
        use crate::chat::events::AgentEvent;
        match self {
            StepEvent::SessionStarted { session_id } => Some(AgentEvent::SessionStarted {
                session_id: session_id.clone(),
            }),
            StepEvent::IterationStarted {
                iteration,
                max_iterations,
            } => Some(AgentEvent::IterationStarted {
                iteration: *iteration,
                max_iterations: *max_iterations,
            }),
            StepEvent::RetrievalInjected { num_snippets } => Some(AgentEvent::RetrievalInjected {
                num_snippets: *num_snippets,
            }),
            StepEvent::LlmError {
                iteration,
                consecutive_errors,
                error,
            } => Some(AgentEvent::LlmError {
                iteration: *iteration,
                consecutive_errors: *consecutive_errors,
                error: error.clone(),
            }),
            StepEvent::LlmResponseReceived {
                iteration,
                usage,
                has_tool_calls,
            } => Some(AgentEvent::LlmResponseReceived {
                iteration: *iteration,
                usage: Some(usage.clone()),
                has_tool_calls: *has_tool_calls,
            }),
            StepEvent::ToolCallStarted {
                iteration,
                tool_name,
                tool_call_id,
            } => Some(AgentEvent::ToolCallStarted {
                iteration: *iteration,
                tool_name: tool_name.clone(),
                tool_call_id: tool_call_id.clone(),
            }),
            StepEvent::ToolCallCompleted {
                iteration,
                tool_name,
                tool_call_id,
                success,
                output_bytes,
                truncated,
                output_preview: _,
            } => Some(AgentEvent::ToolCallCompleted {
                iteration: *iteration,
                tool_name: tool_name.clone(),
                tool_call_id: tool_call_id.clone(),
                success: *success,
                output_bytes: *output_bytes,
                truncated: *truncated,
            }),
            StepEvent::VerificationFailed { iteration } => Some(AgentEvent::VerificationFailed {
                iteration: *iteration,
            }),
            StepEvent::AnswerProduced { iteration, answer } => Some(AgentEvent::AnswerProduced {
                iteration: *iteration,
                answer_length: answer.len(),
            }),
            StepEvent::MaxIterationsReached { iterations } => {
                Some(AgentEvent::MaxIterationsReached {
                    iterations: *iterations,
                })
            }
            StepEvent::LlmStreamEvent { .. } | StepEvent::PreToolHookBlocked { .. } => None,
        }
    }

    pub fn to_stream_event(&self) -> Option<crate::chat::stream::ReactStreamEvent> {
        use crate::chat::stream::ReactStreamEvent;
        match self {
            StepEvent::IterationStarted { iteration, .. } => {
                Some(ReactStreamEvent::IterationStart {
                    iteration: *iteration,
                })
            }
            StepEvent::LlmStreamEvent { event } => Some(ReactStreamEvent::LlmEvent(event.clone())),
            StepEvent::ToolCallCompleted {
                tool_name,
                success,
                output_preview,
                ..
            } => Some(ReactStreamEvent::ToolExecuted {
                name: tool_name.clone(),
                success: *success,
                output_preview: output_preview.clone(),
            }),
            StepEvent::PreToolHookBlocked { tool_name, .. } => {
                Some(ReactStreamEvent::ToolExecuted {
                    name: tool_name.clone(),
                    success: false,
                    output_preview: "Blocked by hook policy".to_string(),
                })
            }
            StepEvent::AnswerProduced { answer, .. } => {
                Some(ReactStreamEvent::Answer(answer.clone()))
            }
            StepEvent::MaxIterationsReached { .. } => Some(ReactStreamEvent::MaxIterationsReached),
            _ => None,
        }
    }
}

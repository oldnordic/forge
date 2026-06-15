use crate::chat::types::Usage;

#[derive(Clone, Debug, PartialEq)]
pub enum StreamEvent {
    Token(String),
    ToolCallStart {
        index: usize,
        id: String,
        name: String,
    },
    ToolCallArgumentDelta {
        index: usize,
        delta: String,
    },
    ToolCallEnd {
        index: usize,
    },
    Usage(Usage),
    Done,
    Error(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ReactStreamEvent {
    LlmEvent(StreamEvent),
    IterationStart {
        iteration: usize,
    },
    ToolExecuted {
        name: String,
        success: bool,
        output_preview: String,
    },
    Answer(String),
    MaxIterationsReached,
}

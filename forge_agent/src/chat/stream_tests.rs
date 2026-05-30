use crate::chat::stream::StreamEvent;
use crate::chat::types::Usage;

#[test]
fn stream_event_token_equality() {
    let a = StreamEvent::Token("hello".to_string());
    let b = StreamEvent::Token("hello".to_string());
    assert_eq!(a, b);
}

#[test]
fn stream_event_tool_call_start_fields() {
    let e = StreamEvent::ToolCallStart {
        index: 0,
        id: "call_1".to_string(),
        name: "file_read".to_string(),
    };
    match e {
        StreamEvent::ToolCallStart { index, id, name } => {
            assert_eq!(index, 0);
            assert_eq!(id, "call_1");
            assert_eq!(name, "file_read");
        }
        _ => panic!("expected ToolCallStart"),
    }
}

#[test]
fn stream_event_tool_call_delta() {
    let e = StreamEvent::ToolCallArgumentDelta {
        index: 0,
        delta: "{\"path\":".to_string(),
    };
    match e {
        StreamEvent::ToolCallArgumentDelta { index, delta } => {
            assert_eq!(index, 0);
            assert_eq!(delta, "{\"path\":");
        }
        _ => panic!("expected ToolCallArgumentDelta"),
    }
}

#[test]
fn stream_event_tool_call_end() {
    let e = StreamEvent::ToolCallEnd { index: 2 };
    match e {
        StreamEvent::ToolCallEnd { index } => assert_eq!(index, 2),
        _ => panic!("expected ToolCallEnd"),
    }
}

#[test]
fn stream_event_usage() {
    let usage = Usage {
        prompt_tokens: Some(10),
        completion_tokens: Some(5),
        total_tokens: Some(15),
    };
    let e = StreamEvent::Usage(usage.clone());
    match e {
        StreamEvent::Usage(u) => {
            assert_eq!(u.prompt_tokens, Some(10));
            assert_eq!(u.completion_tokens, Some(5));
        }
        _ => panic!("expected Usage"),
    }
}

#[test]
fn stream_event_done() {
    assert_eq!(StreamEvent::Done, StreamEvent::Done);
}

#[test]
fn stream_event_error() {
    let e = StreamEvent::Error("boom".to_string());
    match e {
        StreamEvent::Error(msg) => assert_eq!(msg, "boom"),
        _ => panic!("expected Error"),
    }
}

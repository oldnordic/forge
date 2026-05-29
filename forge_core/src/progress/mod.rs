use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ProgressEvent {
    pub operation: String,
    pub phase: ProgressPhase,
    pub current: u64,
    pub total: Option<u64>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressPhase {
    Started,
    Progress,
    Completed,
    Failed(String),
}

pub trait ProgressSink: Send + Sync {
    fn emit(&self, event: ProgressEvent);
}

pub struct NoopProgress;

impl ProgressSink for NoopProgress {
    fn emit(&self, _event: ProgressEvent) {}
}

pub struct ChannelProgress {
    tx: tokio::sync::mpsc::UnboundedSender<ProgressEvent>,
}

impl ChannelProgress {
    pub fn new() -> (Self, tokio::sync::mpsc::UnboundedReceiver<ProgressEvent>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (Self { tx }, rx)
    }
}

impl ProgressSink for ChannelProgress {
    fn emit(&self, event: ProgressEvent) {
        let _ = self.tx.send(event);
    }
}

pub struct ProgressEmitter {
    sink: Option<Arc<dyn ProgressSink>>,
}

impl ProgressEmitter {
    pub fn new(sink: Option<Arc<dyn ProgressSink>>) -> Self {
        Self { sink }
    }

    pub fn started(&self, operation: &str, message: &str) {
        self.emit(ProgressEvent {
            operation: operation.to_string(),
            phase: ProgressPhase::Started,
            current: 0,
            total: None,
            message: message.to_string(),
        });
    }

    pub fn progress(&self, operation: &str, current: u64, total: Option<u64>, message: &str) {
        self.emit(ProgressEvent {
            operation: operation.to_string(),
            phase: ProgressPhase::Progress,
            current,
            total,
            message: message.to_string(),
        });
    }

    pub fn completed(&self, operation: &str, message: &str) {
        self.emit(ProgressEvent {
            operation: operation.to_string(),
            phase: ProgressPhase::Completed,
            current: 0,
            total: None,
            message: message.to_string(),
        });
    }

    pub fn failed(&self, operation: &str, error: &str) {
        self.emit(ProgressEvent {
            operation: operation.to_string(),
            phase: ProgressPhase::Failed(error.to_string()),
            current: 0,
            total: None,
            message: error.to_string(),
        });
    }

    fn emit(&self, event: ProgressEvent) {
        if let Some(ref sink) = self.sink {
            sink.emit(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_does_not_panic() {
        let noop = NoopProgress;
        noop.emit(ProgressEvent {
            operation: "test".to_string(),
            phase: ProgressPhase::Started,
            current: 0,
            total: None,
            message: "test".to_string(),
        });
    }

    #[test]
    fn test_channel_receives_events() {
        let (progress, mut rx) = ChannelProgress::new();
        progress.emit(ProgressEvent {
            operation: "index".to_string(),
            phase: ProgressPhase::Started,
            current: 0,
            total: Some(100),
            message: "starting".to_string(),
        });

        let event = rx.try_recv().unwrap();
        assert_eq!(event.operation, "index");
        assert_eq!(event.phase, ProgressPhase::Started);
        assert_eq!(event.total, Some(100));
    }

    #[test]
    fn test_emitter_with_no_sink() {
        let emitter = ProgressEmitter::new(None);
        emitter.started("test", "starting");
        emitter.progress("test", 50, Some(100), "halfway");
        emitter.completed("test", "done");
    }

    #[test]
    fn test_emitter_with_channel() {
        let (progress, mut rx) = ChannelProgress::new();
        let emitter = ProgressEmitter::new(Some(Arc::new(progress)));

        emitter.started("build", "compiling");
        emitter.progress("build", 5, Some(10), "file 5");
        emitter.completed("build", "success");

        let e1 = rx.try_recv().unwrap();
        assert_eq!(e1.phase, ProgressPhase::Started);
        let e2 = rx.try_recv().unwrap();
        assert_eq!(e2.current, 5);
        let e3 = rx.try_recv().unwrap();
        assert_eq!(e3.phase, ProgressPhase::Completed);
    }

    #[test]
    fn test_emitter_failed() {
        let (progress, mut rx) = ChannelProgress::new();
        let emitter = ProgressEmitter::new(Some(Arc::new(progress)));

        emitter.failed("build", "compile error");

        let event = rx.try_recv().unwrap();
        assert!(matches!(event.phase, ProgressPhase::Failed(ref e) if e == "compile error"));
    }

    #[test]
    fn test_channel_dropped_receiver_does_not_panic() {
        let (progress, rx) = ChannelProgress::new();
        drop(rx);
        progress.emit(ProgressEvent {
            operation: "test".to_string(),
            phase: ProgressPhase::Started,
            current: 0,
            total: None,
            message: "should not panic".to_string(),
        });
    }
}

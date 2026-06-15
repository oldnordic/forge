use crate::chat::stream::StreamEvent;
use futures::Stream;
use std::pin::Pin;

pub(crate) fn spawn_line_stream<S: Send + 'static>(
    initial_state: S,
    response_future: impl std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>
        + Send
        + 'static,
    mut parse_line: impl FnMut(&mut S, &str) -> Vec<StreamEvent> + Send + 'static,
) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send>> {
    let (tx, rx) = futures::channel::mpsc::unbounded::<StreamEvent>();

    tokio::spawn(async move {
        let resp = match response_future.await {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.unbounded_send(StreamEvent::Error(e.to_string()));
                return;
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let _ = tx.unbounded_send(StreamEvent::Error(format!("HTTP {}: {}", status, body)));
            return;
        }

        let mut byte_stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut state = initial_state;

        use futures::StreamExt;

        while let Some(chunk_result) = byte_stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    buffer.push_str(&String::from_utf8_lossy(&chunk));
                    while let Some(pos) = buffer.find('\n') {
                        let line = buffer[..pos].trim_end().to_string();
                        buffer = buffer[pos + 1..].to_string();
                        for event in parse_line(&mut state, &line) {
                            if tx.unbounded_send(event).is_err() {
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.unbounded_send(StreamEvent::Error(e.to_string()));
                    return;
                }
            }
        }

        if !buffer.trim().is_empty() {
            for event in parse_line(&mut state, buffer.trim()) {
                if tx.unbounded_send(event).is_err() {
                    return;
                }
            }
        }

        let _ = tx.unbounded_send(StreamEvent::Done);
    });

    Box::pin(rx)
}

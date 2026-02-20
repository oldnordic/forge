//! WebSocket API server for checkpointing
//!
//! Provides real-time remote access to checkpoint operations

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::errors::{ReasoningError, Result};
use crate::service::{CheckpointEvent, CheckpointService, ServiceMetrics};
use crate::{CheckpointSummary, SessionId};

/// WebSocket server configuration
#[derive(Clone, Debug)]
pub struct WebSocketConfig {
    pub require_auth: bool,
    pub auth_token: Option<String>,
    pub max_connections: usize,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            require_auth: false,
            auth_token: None,
            max_connections: 100,
        }
    }
}

/// WebSocket command from client
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WebSocketCommand {
    pub id: String,
    pub method: String,
    pub params: serde_json::Value,
}

/// WebSocket response to client
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WebSocketResponse {
    pub id: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl WebSocketResponse {
    pub fn success(id: String, result: impl serde::Serialize) -> Self {
        Self {
            id,
            success: true,
            result: serde_json::to_value(result).ok(),
            error: None,
        }
    }

    pub fn error(id: String, message: impl Into<String>) -> Self {
        Self {
            id,
            success: false,
            result: None,
            error: Some(message.into()),
        }
    }
}

/// WebSocket event broadcast
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WebSocketEvent {
    pub event_type: String,
    pub data: serde_json::Value,
}

impl WebSocketEvent {
    pub fn checkpoint_created(checkpoint_id: impl ToString, session_id: impl ToString) -> Self {
        Self {
            event_type: "checkpoint_created".to_string(),
            data: serde_json::json!({
                "checkpoint_id": checkpoint_id.to_string(),
                "session_id": session_id.to_string(),
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
        }
    }

    pub fn checkpoint_restored(checkpoint_id: impl ToString, session_id: impl ToString) -> Self {
        Self {
            event_type: "checkpoint_restored".to_string(),
            data: serde_json::json!({
                "checkpoint_id": checkpoint_id.to_string(),
                "session_id": session_id.to_string(),
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
        }
    }

    pub fn checkpoint_deleted(checkpoint_id: impl ToString, session_id: impl ToString) -> Self {
        Self {
            event_type: "checkpoint_deleted".to_string(),
            data: serde_json::json!({
                "checkpoint_id": checkpoint_id.to_string(),
                "session_id": session_id.to_string(),
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
        }
    }

    pub fn checkpoints_compacted(session_id: impl ToString, remaining: usize) -> Self {
        Self {
            event_type: "checkpoints_compacted".to_string(),
            data: serde_json::json!({
                "session_id": session_id.to_string(),
                "remaining": remaining,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
        }
    }

    /// Convert from CheckpointEvent to WebSocketEvent
    pub fn from_checkpoint_event(event: &CheckpointEvent) -> Self {
        match event {
            CheckpointEvent::Created { checkpoint_id, session_id, .. } => {
                Self::checkpoint_created(checkpoint_id.to_string(), session_id.to_string())
            }
            CheckpointEvent::Restored { checkpoint_id, session_id } => {
                Self::checkpoint_restored(checkpoint_id.to_string(), session_id.to_string())
            }
            CheckpointEvent::Deleted { checkpoint_id, session_id } => {
                Self::checkpoint_deleted(checkpoint_id.to_string(), session_id.to_string())
            }
            CheckpointEvent::Compacted { session_id, remaining } => {
                Self::checkpoints_compacted(session_id.to_string(), *remaining)
            }
        }
    }
}

/// Client connection state
#[derive(Debug, Clone)]
struct ClientState {
    _id: String,
    authenticated: bool,
    subscriptions: Vec<SessionId>,
}

/// WebSocket server for checkpointing
pub struct CheckpointWebSocketServer {
    bind_addr: String,
    service: Arc<CheckpointService>,
    config: WebSocketConfig,
    shutdown_tx: Option<broadcast::Sender<()>>,
    clients: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Message>>>>,
}

impl CheckpointWebSocketServer {
    /// Create a new WebSocket server with default config
    pub fn new(bind_addr: impl Into<String>, service: Arc<CheckpointService>) -> Self {
        Self::with_config(bind_addr, service, WebSocketConfig::default())
    }

    /// Create a new WebSocket server with custom config
    pub fn with_config(
        bind_addr: impl Into<String>,
        service: Arc<CheckpointService>,
        config: WebSocketConfig,
    ) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            service,
            config,
            shutdown_tx: None,
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the server and return the bound address
    pub async fn start(&mut self) -> Result<SocketAddr> {
        let listener = TcpListener::bind(&self.bind_addr).await
            .map_err(|e| ReasoningError::Io(std::io::Error::new(
                std::io::ErrorKind::AddrNotAvailable,
                format!("Failed to bind: {}", e)
            )))?;

        let addr = listener.local_addr()
            .map_err(|e| ReasoningError::Io(e))?;

        let (shutdown_tx, mut shutdown_rx) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        let service = Arc::clone(&self.service);
        let clients = Arc::clone(&self.clients);
        let config = self.config.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Ok((stream, peer_addr)) = listener.accept() => {
                        let service = Arc::clone(&service);
                        let clients = Arc::clone(&clients);
                        let config = config.clone();

                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(
                                stream,
                                peer_addr,
                                service,
                                clients,
                                config,
                            ).await {
                                tracing::warn!("WebSocket connection error: {}", e);
                            }
                        });
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("WebSocket server shutting down");
                        break;
                    }
                }
            }
        });

        tracing::info!("WebSocket server started on {}", addr);
        Ok(addr)
    }

    /// Stop the server
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        
        // Clear all clients
        let mut clients = self.clients.write().await;
        clients.clear();
        
        Ok(())
    }

    /// Get number of connected clients
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }
}

/// Commands sent from message handler to event forwarding task
type SubscribeCommand = (SessionId, tokio::sync::mpsc::UnboundedSender<WebSocketEvent>);

async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    service: Arc<CheckpointService>,
    clients: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Message>>>>,
    config: WebSocketConfig,
) -> Result<()> {
    let ws_stream = accept_async(stream).await
        .map_err(|e| ReasoningError::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            format!("WebSocket handshake failed: {}", e)
        )))?;

    let client_id = uuid::Uuid::new_v4().to_string();
    tracing::info!("New WebSocket connection: {} from {}", client_id, peer_addr);

    let (mut ws_tx, mut ws_rx) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Register client
    {
        let mut clients_guard = clients.write().await;
        if clients_guard.len() >= config.max_connections {
            let _ = ws_tx.send(Message::Text(
                serde_json::to_string(&WebSocketResponse::error(
                    "init".to_string(),
                    "Server at capacity"
                )).unwrap()
            )).await;
            return Ok(());
        }
        clients_guard.insert(client_id.clone(), tx);
    }

    let mut state = ClientState {
        _id: client_id.clone(),
        authenticated: !config.require_auth,
        subscriptions: Vec::new(),
    };

    // Channel for coordinating subscriptions between message handler and event task
    let (sub_tx, mut sub_rx) = mpsc::unbounded_channel::<SubscribeCommand>();

    // Spawn task to forward messages from channel to WebSocket
    let client_id_clone = client_id.clone();
    let clients_clone = Arc::clone(&clients);
    let forward_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
        // Remove client on disconnect
        clients_clone.write().await.remove(&client_id_clone);
    });

    // Spawn task to forward service events to WebSocket client
    let service_for_events = Arc::clone(&service);
    let clients_for_events = Arc::clone(&clients);
    let client_id_for_events = client_id.clone();
    let event_forward_task = tokio::spawn(async move {
        let mut event_receivers: HashMap<SessionId, mpsc::Receiver<CheckpointEvent>> = HashMap::new();
        
        loop {
            tokio::select! {
                // Handle new subscription requests
                Some((session_id, notify_tx)) = sub_rx.recv() => {
                    // Subscribe to service events for this session
                    match service_for_events.subscribe(&session_id) {
                        Ok(rx) => {
                            event_receivers.insert(session_id, rx);
                            // Notify that subscription is active
                            let _ = notify_tx.send(WebSocketEvent {
                                event_type: "subscribed".to_string(),
                                data: serde_json::json!({
                                    "session_id": session_id.to_string(),
                                }),
                            });
                        }
                        Err(e) => {
                            let _ = notify_tx.send(WebSocketEvent {
                                event_type: "subscribe_error".to_string(),
                                data: serde_json::json!({
                                    "session_id": session_id.to_string(),
                                    "error": e.to_string(),
                                }),
                            });
                        }
                    }
                }
                
                // Listen for events from all subscribed sessions
                Some((_session_id, event)) = async {
                    // Poll all receivers
                    for (session_id, rx) in &mut event_receivers {
                        if let Ok(event) = rx.try_recv() {
                            return Some((*session_id, event));
                        }
                    }
                    None
                } => {
                    let ws_event = WebSocketEvent::from_checkpoint_event(&event);
                    let msg = Message::Text(serde_json::to_string(&ws_event).unwrap_or_default());
                    
                    // Send to the client's message channel
                    if let Some(client_tx) = clients_for_events.read().await.get(&client_id_for_events) {
                        let _ = client_tx.send(msg);
                    }
                }
                
                // Small sleep to prevent busy-waiting when no events
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {}
            }
        }
    });

    // Handle incoming messages
    while let Some(msg) = ws_rx.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let response = handle_message(
                    &text,
                    &mut state,
                    &service,
                    &config,
                    &sub_tx,
                ).await;

                let response_text = serde_json::to_string(&response)?;
                let tx = clients.read().await.get(&client_id).cloned();
                if let Some(tx) = tx {
                    let _ = tx.send(Message::Text(response_text));
                }
            }
            Ok(Message::Close(_)) => {
                tracing::info!("Client {} disconnected", client_id);
                break;
            }
            Ok(Message::Ping(data)) => {
                let tx = clients.read().await.get(&client_id).cloned();
                if let Some(tx) = tx {
                    let _ = tx.send(Message::Pong(data));
                }
            }
            Err(e) => {
                tracing::warn!("WebSocket error from {}: {}", client_id, e);
                break;
            }
            _ => {}
        }
    }

    // Cleanup
    event_forward_task.abort();
    forward_task.abort();
    clients.write().await.remove(&client_id);
    tracing::info!("Client {} removed", client_id);

    Ok(())
}

async fn handle_message(
    text: &str,
    state: &mut ClientState,
    service: &Arc<CheckpointService>,
    config: &WebSocketConfig,
    sub_tx: &mpsc::UnboundedSender<SubscribeCommand>,
) -> WebSocketResponse {
    // Parse command
    let cmd: WebSocketCommand = match serde_json::from_str(text) {
        Ok(cmd) => cmd,
        Err(e) => {
            return WebSocketResponse::error(
                "unknown".to_string(),
                format!("Invalid JSON: {}", e)
            );
        }
    };

    // Check authentication
    if config.require_auth && !state.authenticated && cmd.method != "authenticate" {
        return WebSocketResponse::error(
            cmd.id,
            "Authentication required"
        );
    }

    // Handle command
    match cmd.method.as_str() {
        "authenticate" => handle_authenticate(&cmd, state, config).await,
        "create_session" => handle_create_session(&cmd, service).await,
        "list_checkpoints" => handle_list_checkpoints(&cmd, service).await,
        "checkpoint" => handle_checkpoint(&cmd, service).await,
        "subscribe" => handle_subscribe(&cmd, state, sub_tx).await,
        "metrics" => handle_metrics(&cmd, service).await,
        _ => WebSocketResponse::error(
            cmd.id,
            format!("Unknown method: {}", cmd.method)
        ),
    }
}

async fn handle_authenticate(
    cmd: &WebSocketCommand,
    state: &mut ClientState,
    config: &WebSocketConfig,
) -> WebSocketResponse {
    let token = cmd.params.get("token").and_then(|v| v.as_str());
    
    match (&config.auth_token, token) {
        (Some(expected), Some(provided)) if expected == provided => {
            state.authenticated = true;
            WebSocketResponse::success(cmd.id.clone(), serde_json::json!({ "authenticated": true }))
        }
        _ => {
            WebSocketResponse::error(cmd.id.clone(), "Invalid authentication token")
        }
    }
}

async fn handle_create_session(
    cmd: &WebSocketCommand,
    service: &Arc<CheckpointService>,
) -> WebSocketResponse {
    let name = cmd.params.get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unnamed");

    match service.create_session(name) {
        Ok(session_id) => {
            WebSocketResponse::success(cmd.id.clone(), session_id.to_string())
        }
        Err(e) => {
            WebSocketResponse::error(cmd.id.clone(), e.to_string())
        }
    }
}

async fn handle_list_checkpoints(
    cmd: &WebSocketCommand,
    service: &Arc<CheckpointService>,
) -> WebSocketResponse {
    let session_id_str = match cmd.params.get("session_id").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return WebSocketResponse::error(cmd.id.clone(), "Missing session_id parameter");
        }
    };

    let session_id: SessionId = match uuid::Uuid::parse_str(session_id_str) {
        Ok(uuid) => SessionId(uuid),
        Err(_) => {
            return WebSocketResponse::error(cmd.id.clone(), "Invalid session_id format");
        }
    };

    match service.list_checkpoints(&session_id) {
        Ok(checkpoints) => {
            WebSocketResponse::success(cmd.id.clone(), checkpoints)
        }
        Err(e) => {
            WebSocketResponse::error(cmd.id.clone(), e.to_string())
        }
    }
}

async fn handle_checkpoint(
    cmd: &WebSocketCommand,
    service: &Arc<CheckpointService>,
) -> WebSocketResponse {
    let session_id_str = match cmd.params.get("session_id").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return WebSocketResponse::error(cmd.id.clone(), "Missing session_id parameter");
        }
    };

    let session_id: SessionId = match uuid::Uuid::parse_str(session_id_str) {
        Ok(uuid) => SessionId(uuid),
        Err(_) => {
            return WebSocketResponse::error(cmd.id.clone(), "Invalid session_id format");
        }
    };

    let message = cmd.params.get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("Checkpoint");

    match service.checkpoint(&session_id, message) {
        Ok(checkpoint_id) => {
            WebSocketResponse::success(cmd.id.clone(), checkpoint_id.to_string())
        }
        Err(e) => {
            WebSocketResponse::error(cmd.id.clone(), e.to_string())
        }
    }
}

async fn handle_subscribe(
    cmd: &WebSocketCommand,
    state: &mut ClientState,
    sub_tx: &mpsc::UnboundedSender<SubscribeCommand>,
) -> WebSocketResponse {
    let session_id_str = match cmd.params.get("session_id").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return WebSocketResponse::error(cmd.id.clone(), "Missing session_id parameter");
        }
    };

    let session_id: SessionId = match uuid::Uuid::parse_str(session_id_str) {
        Ok(uuid) => SessionId(uuid),
        Err(_) => {
            return WebSocketResponse::error(cmd.id.clone(), "Invalid session_id format");
        }
    };

    state.subscriptions.push(session_id);
    
    // Channel to receive subscription confirmation from event task
    let (notify_tx, mut notify_rx) = mpsc::unbounded_channel();
    
    // Send subscription request to event forwarding task
    if let Err(e) = sub_tx.send((session_id, notify_tx)) {
        return WebSocketResponse::error(
            cmd.id.clone(),
            format!("Failed to setup subscription: {}", e)
        );
    }
    
    // Wait for subscription confirmation (with timeout)
    match tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        notify_rx.recv()
    ).await {
        Ok(Some(event)) if event.event_type == "subscribed" => {
            WebSocketResponse::success(cmd.id.clone(), serde_json::json!({
                "subscribed": true,
                "session_id": session_id.to_string()
            }))
        }
        Ok(Some(event)) if event.event_type == "subscribe_error" => {
            WebSocketResponse::error(cmd.id.clone(), 
                event.data.get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Subscription failed"))
        }
        _ => {
            WebSocketResponse::error(cmd.id.clone(), "Subscription timeout")
        }
    }
}

async fn handle_metrics(
    cmd: &WebSocketCommand,
    service: &Arc<CheckpointService>,
) -> WebSocketResponse {
    match service.metrics() {
        Ok(metrics) => {
            WebSocketResponse::success(cmd.id.clone(), metrics)
        }
        Err(e) => {
            WebSocketResponse::error(cmd.id.clone(), e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_websocket_config_default() {
        let config = WebSocketConfig::default();
        assert!(!config.require_auth);
        assert_eq!(config.max_connections, 100);
    }

    #[tokio::test]
    async fn test_websocket_response_success() {
        let response = WebSocketResponse::success("test-id".to_string(), "hello");
        assert!(response.success);
        assert_eq!(response.id, "test-id");
        assert!(response.error.is_none());
    }

    #[tokio::test]
    async fn test_websocket_response_error() {
        let response = WebSocketResponse::error("test-id".to_string(), "something went wrong");
        assert!(!response.success);
        assert_eq!(response.id, "test-id");
        assert_eq!(response.error.unwrap(), "something went wrong");
    }
}

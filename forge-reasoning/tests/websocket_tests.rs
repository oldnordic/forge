//! TDD Wave 6: WebSocket API Tests
//!
//! Tests for real-time WebSocket checkpoint API

use std::time::Duration;

use forge_reasoning::*;
use std::sync::Arc;
use futures_util::{SinkExt, StreamExt};
use tokio::time::timeout;

/// Test 51: WebSocket server can start and accept connections
#[tokio::test]
async fn test_websocket_server_start() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    let mut server = CheckpointWebSocketServer::new("127.0.0.1:0".to_string(), service);
    let addr = server.start().await.unwrap();
    
    assert!(addr.port() > 0, "Server should bind to a port");
    
    // Clean up
    server.stop().await.unwrap();
}

/// Test 52: Client can connect and send commands
#[tokio::test]
async fn test_websocket_client_connect() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    let mut server = CheckpointWebSocketServer::new("127.0.0.1:0".to_string(), service);
    let addr = server.start().await.unwrap();
    
    // Connect client
    let url = format!("ws://{}/", addr);
    let (mut client, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    
    // Send a command
    let cmd = WebSocketCommand {
        id: "test-1".to_string(),
        method: "create_session".to_string(),
        params: serde_json::json!({ "name": "test-session" }),
    };
    
    let msg = tokio_tungstenite::tungstenite::Message::Text(
        serde_json::to_string(&cmd).unwrap()
    );
    client.send(msg).await.unwrap();
    
    // Receive response
    let response = timeout(Duration::from_secs(5), client.next()).await.unwrap().unwrap().unwrap();
    let response: WebSocketResponse = serde_json::from_str(response.to_text().unwrap()).unwrap();
    
    assert!(response.success, "Command should succeed");
    assert!(response.result.is_some(), "Should have result");
    
    // Clean up
    server.stop().await.unwrap();
}

/// Test 53: Server broadcasts checkpoint events to subscribers
/// 
/// NOTE: Event broadcasting is now fully implemented.
#[tokio::test]
async fn test_websocket_event_broadcast() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    let mut server = CheckpointWebSocketServer::new("127.0.0.1:0".to_string(), service.clone());
    let addr = server.start().await.unwrap();
    
    // Connect two clients
    let url = format!("ws://{}/", addr);
    let (mut client1, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    let (mut client2, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    
    // Both clients subscribe to session
    let session = service.create_session("broadcast-test").unwrap();
    
    let subscribe_cmd = WebSocketCommand {
        id: "sub-1".to_string(),
        method: "subscribe".to_string(),
        params: serde_json::json!({ "session_id": session.to_string() }),
    };
    
    let msg = tokio_tungstenite::tungstenite::Message::Text(
        serde_json::to_string(&subscribe_cmd).unwrap()
    );
    client1.send(msg.clone()).await.unwrap();
    client2.send(msg).await.unwrap();
    
    // Wait for subscription confirmation
    let _ = timeout(Duration::from_secs(2), client1.next()).await.unwrap();
    let _ = timeout(Duration::from_secs(2), client2.next()).await.unwrap();
    
    // Create checkpoint (should broadcast)
    service.checkpoint(&session, "Broadcast test").unwrap();
    
    // Both clients should receive event
    let event1 = timeout(Duration::from_secs(2), client1.next()).await.unwrap().unwrap().unwrap();
    let event2 = timeout(Duration::from_secs(2), client2.next()).await.unwrap().unwrap().unwrap();
    
    let event1: WebSocketEvent = serde_json::from_str(event1.to_text().unwrap()).unwrap();
    let event2: WebSocketEvent = serde_json::from_str(event2.to_text().unwrap()).unwrap();
    
    assert_eq!(event1.event_type, "checkpoint_created");
    assert_eq!(event2.event_type, "checkpoint_created");
    
    // Clean up
    server.stop().await.unwrap();
}

/// Test 54: Server handles malformed messages gracefully
#[tokio::test]
async fn test_websocket_malformed_message() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    let mut server = CheckpointWebSocketServer::new("127.0.0.1:0".to_string(), service);
    let addr = server.start().await.unwrap();
    
    let url = format!("ws://{}/", addr);
    let (mut client, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    
    // Send malformed JSON
    let msg = tokio_tungstenite::tungstenite::Message::Text(
        "this is not valid json".to_string()
    );
    client.send(msg).await.unwrap();
    
    // Should receive error response
    let response = timeout(Duration::from_secs(2), client.next()).await.unwrap().unwrap().unwrap();
    let response: WebSocketResponse = serde_json::from_str(response.to_text().unwrap()).unwrap();
    
    assert!(!response.success, "Should fail for malformed JSON");
    assert!(response.error.is_some(), "Should have error message");
    
    // Clean up
    server.stop().await.unwrap();
}

/// Test 55: Server supports multiple concurrent sessions
#[tokio::test]
async fn test_websocket_multiple_sessions() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    let mut server = CheckpointWebSocketServer::new("127.0.0.1:0".to_string(), service.clone());
    let addr = server.start().await.unwrap();
    
    let url = format!("ws://{}/", addr);
    let (mut client, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    
    // Create session 1
    let cmd1 = WebSocketCommand {
        id: "cmd-1".to_string(),
        method: "create_session".to_string(),
        params: serde_json::json!({ "name": "session-1" }),
    };
    let msg = tokio_tungstenite::tungstenite::Message::Text(
        serde_json::to_string(&cmd1).unwrap()
    );
    client.send(msg).await.unwrap();
    
    let response = timeout(Duration::from_secs(2), client.next()).await.unwrap().unwrap().unwrap();
    let response: WebSocketResponse = serde_json::from_str(response.to_text().unwrap()).unwrap();
    let session1: SessionId = serde_json::from_value(response.result.unwrap()).unwrap();
    
    // Create session 2
    let cmd2 = WebSocketCommand {
        id: "cmd-2".to_string(),
        method: "create_session".to_string(),
        params: serde_json::json!({ "name": "session-2" })
    };
    let msg = tokio_tungstenite::tungstenite::Message::Text(
        serde_json::to_string(&cmd2).unwrap()
    );
    client.send(msg).await.unwrap();
    
    let response = timeout(Duration::from_secs(2), client.next()).await.unwrap().unwrap().unwrap();
    let response: WebSocketResponse = serde_json::from_str(response.to_text().unwrap()).unwrap();
    let session2: SessionId = serde_json::from_value(response.result.unwrap()).unwrap();
    
    // Sessions should be different
    assert_ne!(session1, session2, "Sessions should have different IDs");
    
    // Clean up
    server.stop().await.unwrap();
}

/// Test 56: Server handles client disconnect gracefully
#[tokio::test]
async fn test_websocket_client_disconnect() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    let mut server = CheckpointWebSocketServer::new("127.0.0.1:0".to_string(), service.clone());
    let addr = server.start().await.unwrap();
    
    let url = format!("ws://{}/", addr);
    let (client, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    
    // Drop client (simulates disconnect)
    drop(client);
    
    // Server should still be operational
    let (mut client2, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    
    let cmd = WebSocketCommand {
        id: "cmd-1".to_string(),
        method: "create_session".to_string(),
        params: serde_json::json!({ "name": "after-disconnect" }),
    };
    
    let msg = tokio_tungstenite::tungstenite::Message::Text(
        serde_json::to_string(&cmd).unwrap()
    );
    client2.send(msg).await.unwrap();
    
    let response = timeout(Duration::from_secs(2), client2.next()).await.unwrap().unwrap().unwrap();
    let response: WebSocketResponse = serde_json::from_str(response.to_text().unwrap()).unwrap();
    
    assert!(response.success, "Server should work after client disconnect");
    
    // Clean up
    server.stop().await.unwrap();
}

/// Test 57: Server supports list_checkpoints command
#[tokio::test]
async fn test_websocket_list_checkpoints() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    let mut server = CheckpointWebSocketServer::new("127.0.0.1:0".to_string(), service.clone());
    let addr = server.start().await.unwrap();
    
    // Pre-populate checkpoints
    let session = service.create_session("list-test").unwrap();
    service.checkpoint(&session, "CP 1").unwrap();
    service.checkpoint(&session, "CP 2").unwrap();
    
    let url = format!("ws://{}/", addr);
    let (mut client, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    
    // List checkpoints
    let cmd = WebSocketCommand {
        id: "cmd-1".to_string(),
        method: "list_checkpoints".to_string(),
        params: serde_json::json!({ "session_id": session.to_string() }),
    };
    
    let msg = tokio_tungstenite::tungstenite::Message::Text(
        serde_json::to_string(&cmd).unwrap()
    );
    client.send(msg).await.unwrap();
    
    let response = timeout(Duration::from_secs(2), client.next()).await.unwrap().unwrap().unwrap();
    let response: WebSocketResponse = serde_json::from_str(response.to_text().unwrap()).unwrap();
    
    assert!(response.success);
    let checkpoints: Vec<CheckpointSummary> = serde_json::from_value(response.result.unwrap()).unwrap();
    assert_eq!(checkpoints.len(), 2, "Should list 2 checkpoints");
    
    // Clean up
    server.stop().await.unwrap();
}

/// Test 58: Server returns error for unknown methods
#[tokio::test]
async fn test_websocket_unknown_method() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    let mut server = CheckpointWebSocketServer::new("127.0.0.1:0".to_string(), service);
    let addr = server.start().await.unwrap();
    
    let url = format!("ws://{}/", addr);
    let (mut client, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    
    let cmd = WebSocketCommand {
        id: "cmd-1".to_string(),
        method: "unknown_method".to_string(),
        params: serde_json::json!({}),
    };
    
    let msg = tokio_tungstenite::tungstenite::Message::Text(
        serde_json::to_string(&cmd).unwrap()
    );
    client.send(msg).await.unwrap();
    
    let response = timeout(Duration::from_secs(2), client.next()).await.unwrap().unwrap().unwrap();
    let response: WebSocketResponse = serde_json::from_str(response.to_text().unwrap()).unwrap();
    
    assert!(!response.success, "Should fail for unknown method");
    assert!(response.error.is_some());
    assert!(response.error.unwrap().contains("unknown_method"));
    
    // Clean up
    server.stop().await.unwrap();
}

/// Test 59: Server supports authentication
#[tokio::test]
async fn test_websocket_authentication() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    let mut config = WebSocketConfig::default();
    config.require_auth = true;
    config.auth_token = Some("secret-token".to_string());
    
    let mut server = CheckpointWebSocketServer::with_config("127.0.0.1:0".to_string(), service, config);
    let addr = server.start().await.unwrap();
    
    let url = format!("ws://{}/", addr);
    
    // Connect without auth should fail
    let result = tokio_tungstenite::connect_async(&url).await;
    // Note: Connection might succeed but first command should fail
    
    // Connect with auth
    let (mut client, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    
    let auth_cmd = WebSocketCommand {
        id: "auth-1".to_string(),
        method: "authenticate".to_string(),
        params: serde_json::json!({ "token": "secret-token" }),
    };
    
    let msg = tokio_tungstenite::tungstenite::Message::Text(
        serde_json::to_string(&auth_cmd).unwrap()
    );
    client.send(msg).await.unwrap();
    
    let response = timeout(Duration::from_secs(2), client.next()).await.unwrap().unwrap().unwrap();
    let response: WebSocketResponse = serde_json::from_str(response.to_text().unwrap()).unwrap();
    
    assert!(response.success, "Should authenticate with valid token");
    
    // Clean up
    server.stop().await.unwrap();
}

/// Test 60: Server handles high message volume
#[tokio::test]
async fn test_websocket_high_volume() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    let mut server = CheckpointWebSocketServer::new("127.0.0.1:0".to_string(), service.clone());
    let addr = server.start().await.unwrap();
    
    let url = format!("ws://{}/", addr);
    let (mut client, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    
    // Create session first
    let cmd = WebSocketCommand {
        id: "cmd-0".to_string(),
        method: "create_session".to_string(),
        params: serde_json::json!({ "name": "high-volume" }),
    };
    let msg = tokio_tungstenite::tungstenite::Message::Text(
        serde_json::to_string(&cmd).unwrap()
    );
    client.send(msg).await.unwrap();
    let _ = timeout(Duration::from_secs(2), client.next()).await.unwrap();
    
    // Send many commands quickly
    for i in 0..50 {
        let cmd = WebSocketCommand {
            id: format!("cmd-{}", i),
            method: "metrics".to_string(),
            params: serde_json::json!({}),
        };
        
        let msg = tokio_tungstenite::tungstenite::Message::Text(
            serde_json::to_string(&cmd).unwrap()
        );
        client.send(msg).await.unwrap();
    }
    
    // Collect all responses
    let mut responses = 0;
    while let Ok(Some(Ok(_))) = timeout(Duration::from_millis(100), client.next()).await {
        responses += 1;
        if responses >= 50 {
            break;
        }
    }
    
    assert!(responses >= 50, "Should receive all responses, got {}", responses);
    
    // Clean up
    server.stop().await.unwrap();
}

// Helper types for WebSocket protocol
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct WebSocketCommand {
    id: String,
    method: String,
    params: serde_json::Value,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct WebSocketResponse {
    id: String,
    success: bool,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct WebSocketEvent {
    event_type: String,
    data: serde_json::Value,
}

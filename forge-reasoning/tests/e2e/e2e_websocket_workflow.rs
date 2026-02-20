//! E2E Test: WebSocket API Workflows
//!
//! Tests for real-time remote access and event broadcasting.

use forge_reasoning::*;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

/// E2E Test 11: WebSocket client full workflow
#[tokio::test]
async fn e2e_websocket_client_workflow() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    // Start WebSocket server
    let mut server = CheckpointWebSocketServer::new("127.0.0.1:0", service.clone());
    let addr = server.start().await.unwrap();
    
    // Connect client
    let url = format!("ws://{}/", addr);
    let (mut client, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    
    // 1. Create session via WebSocket
    let create_cmd = WebSocketCommand {
        id: "cmd-1".to_string(),
        method: "create_session".to_string(),
        params: serde_json::json!({ "name": "websocket-test" }),
    };
    
    client.send(Message::Text(
        serde_json::to_string(&create_cmd).unwrap()
    )).await.unwrap();
    
    let response = timeout(Duration::from_secs(5), client.next()).await
        .unwrap().unwrap().unwrap();
    
    let response: WebSocketResponse = serde_json::from_str(response.to_text().unwrap()).unwrap();
    assert!(response.success, "Create session should succeed");
    
    let session_id: String = serde_json::from_value(response.result.unwrap()).unwrap();
    
    // 2. Subscribe to session events
    let subscribe_cmd = WebSocketCommand {
        id: "cmd-2".to_string(),
        method: "subscribe".to_string(),
        params: serde_json::json!({ "session_id": session_id }),
    };
    
    client.send(Message::Text(
        serde_json::to_string(&subscribe_cmd).unwrap()
    )).await.unwrap();
    
    let response = timeout(Duration::from_secs(5), client.next()).await
        .unwrap().unwrap().unwrap();
    
    let response: WebSocketResponse = serde_json::from_str(response.to_text().unwrap()).unwrap();
    assert!(response.success, "Subscribe should succeed");
    
    // 3. Create checkpoint via service (should trigger event)
    let sid = SessionId(uuid::Uuid::parse_str(&session_id).unwrap());
    let _ = service.checkpoint(&sid, "Test from service").unwrap();
    
    // 4. Receive event broadcast
    let event = timeout(Duration::from_secs(5), client.next()).await
        .unwrap().unwrap().unwrap();
    
    let event: WebSocketEvent = serde_json::from_str(event.to_text().unwrap()).unwrap();
    assert_eq!(event.event_type, "checkpoint_created");
    
    // Cleanup
    server.stop().await.unwrap();
}

/// E2E Test 12: Multiple WebSocket clients receiving events
#[tokio::test]
async fn e2e_websocket_multi_client() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    // Start server
    let mut server = CheckpointWebSocketServer::new("127.0.0.1:0", service.clone());
    let addr = server.start().await.unwrap();
    
    // Connect multiple clients
    let url = format!("ws://{}/", addr);
    let (mut client1, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    let (mut client2, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    
    // Create session and subscribe both clients
    let session = service.create_session("multi-client-test").unwrap();
    
    let subscribe_cmd = WebSocketCommand {
        id: "sub".to_string(),
        method: "subscribe".to_string(),
        params: serde_json::json!({ "session_id": session.to_string() }),
    };
    
    let msg = Message::Text(
        serde_json::to_string(&subscribe_cmd).unwrap()
    );
    
    client1.send(msg.clone()).await.unwrap();
    client2.send(msg).await.unwrap();
    
    // Wait for subscription confirmations
    let _ = timeout(Duration::from_secs(2), client1.next()).await;
    let _ = timeout(Duration::from_secs(2), client2.next()).await;
    
    // Create checkpoint
    let _ = service.checkpoint(&session, "Broadcast test").unwrap();
    
    // Both clients should receive the event
    let event1 = timeout(Duration::from_secs(5), client1.next()).await
        .unwrap().unwrap().unwrap();
    let event2 = timeout(Duration::from_secs(5), client2.next()).await
        .unwrap().unwrap().unwrap();
    
    let event1: WebSocketEvent = serde_json::from_str(event1.to_text().unwrap()).unwrap();
    let event2: WebSocketEvent = serde_json::from_str(event2.to_text().unwrap()).unwrap();
    
    assert_eq!(event1.event_type, "checkpoint_created");
    assert_eq!(event2.event_type, "checkpoint_created");
    assert_eq!(event1.data.get("checkpoint_id"), event2.data.get("checkpoint_id"));
    
    // Cleanup
    server.stop().await.unwrap();
}

/// E2E Test 13: WebSocket with authentication
#[tokio::test]
async fn e2e_websocket_authentication() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    // Configure server with auth
    let config = WebSocketConfig {
        require_auth: true,
        auth_token: Some("secret-token-123".to_string()),
        max_connections: 10,
    };
    
    let mut server = CheckpointWebSocketServer::with_config(
        "127.0.0.1:0", 
        service.clone(),
        config
    );
    let addr = server.start().await.unwrap();
    
    // Connect without auth - should fail for protected operations
    let url = format!("ws://{}/", addr);
    let (mut client, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    
    // Try to create session without auth
    let cmd = WebSocketCommand {
        id: "cmd-1".to_string(),
        method: "create_session".to_string(),
        params: serde_json::json!({ "name": "test" }),
    };
    
    client.send(Message::Text(
        serde_json::to_string(&cmd).unwrap()
    )).await.unwrap();
    
    let response = timeout(Duration::from_secs(5), client.next()).await
        .unwrap().unwrap().unwrap();
    
    let response: WebSocketResponse = serde_json::from_str(response.to_text().unwrap()).unwrap();
    assert!(!response.success, "Should fail without auth");
    assert!(response.error.unwrap().contains("Authentication"));
    
    // Authenticate
    let auth_cmd = WebSocketCommand {
        id: "auth".to_string(),
        method: "authenticate".to_string(),
        params: serde_json::json!({ "token": "secret-token-123" }),
    };
    
    client.send(Message::Text(
        serde_json::to_string(&auth_cmd).unwrap()
    )).await.unwrap();
    
    let response = timeout(Duration::from_secs(5), client.next()).await
        .unwrap().unwrap().unwrap();
    
    let response: WebSocketResponse = serde_json::from_str(response.to_text().unwrap()).unwrap();
    assert!(response.success, "Auth should succeed with valid token");
    
    // Cleanup
    server.stop().await.unwrap();
}

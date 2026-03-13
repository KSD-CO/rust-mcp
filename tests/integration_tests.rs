//! Integration tests for mcp-kit: client-server communication.
//!
//! Tests end-to-end communication between McpClient and McpServer.

use mcp_kit::{CallToolResult, McpServer, ServeSseExt, ServeWebSocketExt, Tool};
use mcp_kit_client::McpClient;
use schemars::JsonSchema;
use serde::Deserialize;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::timeout;

// ─── Test Helpers ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct GreetInput {
    name: String,
}

/// Create a test server with basic tools.
fn create_test_server() -> McpServer {
    let schema = serde_json::to_value(schemars::schema_for!(GreetInput)).unwrap();

    McpServer::builder()
        .name("test-server")
        .version("1.0.0")
        .instructions("Test server for integration tests")
        .tool(
            Tool::new("greet", "Greet a person", schema.clone()),
            |params: GreetInput| async move {
                CallToolResult::text(format!("Hello, {}!", params.name))
            },
        )
        .tool(
            Tool::new("echo", "Echo input", serde_json::json!({"type": "object"})),
            |params: serde_json::Value| async move { CallToolResult::text(params.to_string()) },
        )
        .tool(
            Tool::new("add", "Add two numbers", serde_json::json!({"type": "object"})),
            |params: serde_json::Value| async move {
                let a = params["a"].as_i64().unwrap_or(0);
                let b = params["b"].as_i64().unwrap_or(0);
                CallToolResult::text(format!("{}", a + b))
            },
        )
        .build()
}

// ─── WebSocket Tests ─────────────────────────────────────────────────────────

#[tokio::test]
#[cfg(feature = "websocket")]
async fn test_websocket_initialize() {
    let server = create_test_server();
    let port = 19001;
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

    // Start server in background
    let server_handle = tokio::spawn(async move {
        server.serve_websocket(addr).await.ok();
    });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Connect client
    let url = format!("ws://127.0.0.1:{}/ws", port);
    let client = McpClient::websocket(&url).await.expect("Failed to connect");

    // Initialize
    let result = timeout(
        Duration::from_secs(5),
        client.initialize("test-client", "1.0.0"),
    )
    .await;

    assert!(result.is_ok(), "Initialize timed out");
    let server_info = result.unwrap().expect("Initialize failed");
    assert_eq!(server_info.name, "test-server");
    assert_eq!(server_info.version, "1.0.0");

    // Cleanup
    client.close().await.ok();
    server_handle.abort();
}

#[tokio::test]
#[cfg(feature = "websocket")]
async fn test_websocket_list_tools() {
    let server = create_test_server();
    let port = 19002;
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

    let server_handle = tokio::spawn(async move {
        server.serve_websocket(addr).await.ok();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let url = format!("ws://127.0.0.1:{}/ws", port);
    let client = McpClient::websocket(&url).await.expect("Failed to connect");
    client
        .initialize("test-client", "1.0.0")
        .await
        .expect("Initialize failed");

    let tools = client.list_tools().await.expect("list_tools failed");

    assert_eq!(tools.len(), 3);
    let tool_names: Vec<_> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(tool_names.contains(&"greet"));
    assert!(tool_names.contains(&"echo"));
    assert!(tool_names.contains(&"add"));

    client.close().await.ok();
    server_handle.abort();
}

#[tokio::test]
#[cfg(feature = "websocket")]
async fn test_websocket_call_tool() {
    let server = create_test_server();
    let port = 19003;
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

    let server_handle = tokio::spawn(async move {
        server.serve_websocket(addr).await.ok();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let url = format!("ws://127.0.0.1:{}/ws", port);
    let client = McpClient::websocket(&url).await.expect("Failed to connect");
    client
        .initialize("test-client", "1.0.0")
        .await
        .expect("Initialize failed");

    // Test greet tool
    let result = client
        .call_tool("greet", serde_json::json!({"name": "World"}))
        .await
        .expect("call_tool failed");

    assert!(!result.content.is_empty());
    if let mcp_kit::Content::Text(text) = &result.content[0] {
        assert_eq!(text.text, "Hello, World!");
    } else {
        panic!("Expected text content");
    }

    // Test add tool
    let result = client
        .call_tool("add", serde_json::json!({"a": 10, "b": 25}))
        .await
        .expect("call_tool failed");

    if let mcp_kit::Content::Text(text) = &result.content[0] {
        assert_eq!(text.text, "35");
    } else {
        panic!("Expected text content");
    }

    client.close().await.ok();
    server_handle.abort();
}

// ─── SSE Tests ───────────────────────────────────────────────────────────────

#[tokio::test]
#[cfg(feature = "sse")]
async fn test_sse_initialize() {
    let server = create_test_server();
    let port = 19011;
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

    let server_handle = tokio::spawn(async move {
        server.serve_sse(addr).await.ok();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let url = format!("http://127.0.0.1:{}", port);
    let client = McpClient::sse(&url).await.expect("Failed to connect");

    let result = timeout(
        Duration::from_secs(5),
        client.initialize("test-client", "1.0.0"),
    )
    .await;

    assert!(result.is_ok(), "Initialize timed out");
    let server_info = result.unwrap().expect("Initialize failed");
    assert_eq!(server_info.name, "test-server");

    client.close().await.ok();
    server_handle.abort();
}

#[tokio::test]
#[cfg(feature = "sse")]
async fn test_sse_call_tool() {
    let server = create_test_server();
    let port = 19012;
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

    let server_handle = tokio::spawn(async move {
        server.serve_sse(addr).await.ok();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let url = format!("http://127.0.0.1:{}", port);
    let client = McpClient::sse(&url).await.expect("Failed to connect");
    client
        .initialize("test-client", "1.0.0")
        .await
        .expect("Initialize failed");

    let result = client
        .call_tool("greet", serde_json::json!({"name": "SSE"}))
        .await
        .expect("call_tool failed");

    if let mcp_kit::Content::Text(text) = &result.content[0] {
        assert_eq!(text.text, "Hello, SSE!");
    } else {
        panic!("Expected text content");
    }

    client.close().await.ok();
    server_handle.abort();
}

// ─── Multiple Tools Test ─────────────────────────────────────────────────────

#[tokio::test]
#[cfg(feature = "websocket")]
async fn test_multiple_tool_calls() {
    let server = create_test_server();
    let port = 19031;
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

    let server_handle = tokio::spawn(async move {
        server.serve_websocket(addr).await.ok();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let url = format!("ws://127.0.0.1:{}/ws", port);
    let client = McpClient::websocket(&url).await.expect("Failed to connect");
    client
        .initialize("test-client", "1.0.0")
        .await
        .expect("Initialize failed");

    // Call multiple tools sequentially
    for i in 0..10 {
        let result = client
            .call_tool("add", serde_json::json!({"a": i, "b": i}))
            .await
            .unwrap_or_else(|_| panic!("call_tool failed on iteration {}", i));

        if let mcp_kit::Content::Text(text) = &result.content[0] {
            assert_eq!(text.text, format!("{}", i + i));
        }
    }

    client.close().await.ok();
    server_handle.abort();
}

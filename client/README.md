# mcp-kit-client

MCP (Model Context Protocol) Client SDK for Rust.

Connect to MCP servers using multiple transport mechanisms.

## Features

- **Multiple Transports**: Stdio (subprocess), SSE (HTTP), WebSocket, Streamable HTTP
- **Async/Await**: Built on Tokio for high-performance async operations
- **Type-Safe**: Full type safety with MCP protocol types from `mcp-kit`
- **Easy to Use**: Simple, ergonomic API

## Installation

```toml
[dependencies]
mcp-kit-client = "0.2"
```

## Quick Start

### Connect via WebSocket

```rust
use mcp_kit_client::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Connect to WebSocket server
    let client = McpClient::websocket("ws://localhost:3001/ws").await?;
    
    // Initialize connection
    let server_info = client.initialize("my-app", "1.0.0").await?;
    println!("Connected to: {}", server_info.name);
    
    // List tools
    let tools = client.list_tools().await?;
    for tool in tools {
        println!("Tool: {}", tool.name);
    }
    
    // Call a tool
    let result = client.call_tool("greet", serde_json::json!({
        "name": "World"
    })).await?;
    println!("Result: {:?}", result);
    
    Ok(())
}
```

### Connect via Stdio (Subprocess)

```rust
use mcp_kit_client::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Spawn and connect to MCP server subprocess
    let client = McpClient::stdio("/path/to/mcp-server").await?;
    
    // Or with arguments and environment variables
    let client = McpClient::stdio_with_args(
        "/path/to/mcp-server",
        &["--config", "config.toml"],
        &[("RUST_LOG", "debug")],
    ).await?;
    
    // Initialize and use...
    let server_info = client.initialize("my-app", "1.0.0").await?;
    
    Ok(())
}
```

### Connect via SSE (HTTP)

```rust
use mcp_kit_client::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Connect via SSE
    let client = McpClient::sse("http://localhost:3000").await?;
    
    // Initialize and use...
    let server_info = client.initialize("my-app", "1.0.0").await?;
    
    Ok(())
}
```

### Connect via Streamable HTTP (MCP 2025-03-26)

```rust
use mcp_kit_client::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Connect via Streamable HTTP (single /mcp endpoint)
    let client = McpClient::streamable_http("http://localhost:3000").await?;
    
    // Or with custom endpoint
    let client = McpClient::streamable_http_with_endpoint(
        "http://localhost:3000",
        "/api/mcp"
    ).await?;
    
    // Initialize and use...
    let server_info = client.initialize("my-app", "1.0.0").await?;
    
    Ok(())
}
```

## Available Operations

| Method | Description |
|--------|-------------|
| `initialize()` | Initialize the MCP connection (required first) |
| `list_tools()` | List available tools |
| `call_tool()` | Call a tool with arguments |
| `list_resources()` | List available resources |
| `read_resource()` | Read a resource by URI |
| `list_prompts()` | List available prompts |
| `get_prompt()` | Get a prompt by name |
| `complete()` | Request completion suggestions |
| `subscribe()` | Subscribe to resource updates |
| `unsubscribe()` | Unsubscribe from resource updates |
| `recv_notification()` | Receive server notifications |

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `stdio` | Stdio transport (subprocess) | ✅ |
| `sse` | SSE transport (HTTP) | ✅ |
| `websocket` | WebSocket transport | ✅ |
| `streamable-http` | Streamable HTTP transport | ✅ |
| `full` | All transports | ✅ |

## Error Handling

```rust
use mcp_kit_client::{ClientError, ClientResult};

async fn example(client: &McpClient) -> ClientResult<()> {
    match client.call_tool("unknown", serde_json::json!({})).await {
        Ok(result) => println!("Success: {:?}", result),
        Err(ClientError::ServerError { code, message }) => {
            println!("Server error {}: {}", code, message);
        }
        Err(ClientError::NotInitialized) => {
            println!("Call initialize() first!");
        }
        Err(e) => println!("Error: {}", e),
    }
    Ok(())
}
```

## License

MIT

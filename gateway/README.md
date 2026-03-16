# mcp-kit-gateway

[![Crates.io](https://img.shields.io/crates/v/mcp-kit-gateway.svg)](https://crates.io/crates/mcp-kit-gateway)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../LICENSE)

**MCP Gateway — proxy and aggregate tools, resources, and prompts from multiple upstream MCP servers.**

`mcp-kit-gateway` connects to one or more upstream MCP servers, discovers their capabilities, and exposes them through a single gateway endpoint with namespace prefixing to avoid collisions.

---

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
mcp-kit-gateway = "0.1"
mcp-kit = { version = "0.3", features = ["sse"] }  # Pick transport(s) for serving
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

**Minimum Supported Rust Version (MSRV):** 1.85

---

## Quick Start

```rust
use mcp_kit::prelude::*;
use mcp_kit_gateway::{GatewayManager, UpstreamConfig, UpstreamTransport};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Configure upstreams
    let mut gw = GatewayManager::new();
    gw.add_upstream(UpstreamConfig {
        name: "weather".into(),
        transport: UpstreamTransport::Sse("http://localhost:3001/sse".into()),
        prefix: Some("weather".into()),
        client_name: None,
        client_version: None,
    });

    // 2. Build the gateway server
    let server = gw.build_server(
        McpServer::builder()
            .name("my-gateway")
            .version("1.0.0")
    ).await?;

    // 3. Serve via any transport
    server.serve_sse(([0, 0, 0, 0], 3000)).await?;
    Ok(())
}
```

---

## How It Works

```
┌──────────┐         ┌─────────────────────┐         ┌──────────────────┐
│  Client   │ ──────> │   Gateway Server    │ ──────> │ Upstream Server A│
│ (Claude,  │         │  (mcp-kit-gateway)  │         │ (weather tools)  │
│  Cursor)  │         │                     │ ──────> │ Upstream Server B│
└──────────┘         └─────────────────────┘         │ (database tools) │
                                                      └──────────────────┘
```

1. **Configure** — Define upstream servers and their transport (SSE, WebSocket, Streamable HTTP, or stdio)
2. **Connect** — The gateway connects to each upstream via `McpClient` and calls `initialize()`
3. **Discover** — Lists all tools, resources, and prompts from each upstream
4. **Namespace** — Prefixes each capability to avoid collisions (e.g., `get_weather` becomes `weather/get_weather`)
5. **Register** — Adds proxy handlers to the local `McpServer` router
6. **Proxy** — When a client calls a proxied tool, the gateway forwards to the appropriate upstream and returns the result

---

## Configuration

### UpstreamConfig

```rust
pub struct UpstreamConfig {
    /// Unique name for this upstream (used as default prefix).
    pub name: String,
    /// How to connect to this upstream.
    pub transport: UpstreamTransport,
    /// Optional prefix for namespacing (defaults to `name`).
    /// Set to empty string to disable prefixing.
    pub prefix: Option<String>,
    /// Client name reported during initialization (defaults to "mcp-kit-gateway").
    pub client_name: Option<String>,
    /// Client version reported during initialization (defaults to crate version).
    pub client_version: Option<String>,
}
```

### Namespacing

By default, upstream capabilities are prefixed with `prefix/original_name`:

| Upstream prefix | Original tool | Exposed as |
|-----------------|--------------|------------|
| `"weather"` | `get_forecast` | `weather/get_forecast` |
| `"db"` | `query` | `db/query` |
| `""` (empty) | `search` | `search` (no prefix) |

Set `prefix: Some("".into())` to expose tools without prefixing (be careful of name collisions).

---

## Upstream Transports

### SSE (HTTP Server-Sent Events)

```rust
UpstreamTransport::Sse("http://localhost:3001/sse".into())
```

### WebSocket

```rust
UpstreamTransport::WebSocket("ws://localhost:3002/ws".into())
```

### Streamable HTTP (MCP 2025-03-26)

```rust
UpstreamTransport::StreamableHttp("http://localhost:3003/mcp".into())
```

### Stdio (spawn subprocess)

```rust
UpstreamTransport::Stdio {
    program: "/path/to/mcp-server".into(),
    args: vec!["--port".into(), "3001".into()],
    env: vec![("API_KEY".into(), "my-secret".into())],
}
```

Each transport variant is feature-gated. See [Feature Flags](#feature-flags) below.

---

## Mixing Local and Proxied Tools

You can add local tools alongside proxied upstream tools:

```rust
let server = gw.build_server(
    McpServer::builder()
        .name("my-gateway")
        .version("1.0.0")
        // Local tool — registered directly
        .tool(
            Tool::no_params("gateway/health", "Health check"),
            |_args: serde_json::Value| async move {
                CallToolResult::text("OK")
            },
        )
        // Macro-based local tool
        .tool_def(my_local_tool_def())
).await?;
```

---

## Multiple Upstreams

```rust
let mut gw = GatewayManager::new();

// Weather service
gw.add_upstream(UpstreamConfig {
    name: "weather".into(),
    transport: UpstreamTransport::Sse("http://localhost:3001/sse".into()),
    prefix: Some("weather".into()),
    client_name: None,
    client_version: None,
});

// Database service
gw.add_upstream(UpstreamConfig {
    name: "database".into(),
    transport: UpstreamTransport::WebSocket("ws://localhost:3002/ws".into()),
    prefix: Some("db".into()),
    client_name: None,
    client_version: None,
});

// File system tool (stdio)
gw.add_upstream(UpstreamConfig {
    name: "filesystem".into(),
    transport: UpstreamTransport::Stdio {
        program: "/usr/local/bin/fs-server".into(),
        args: vec![],
        env: vec![],
    },
    prefix: Some("fs".into()),
    client_name: None,
    client_version: None,
});

let server = gw.build_server(
    McpServer::builder().name("multi-gateway").version("1.0.0")
).await?;
```

Clients see all tools aggregated:
- `weather/get_forecast`
- `weather/get_alerts`
- `db/query`
- `db/list_tables`
- `fs/read_file`
- `fs/list_directory`

---

## Lower-Level API

If you need more control, use `connect_and_discover()` directly:

```rust
let mut gw = GatewayManager::new();
gw.add_upstream(config);

// Connect and get raw definitions
let (tools, resources, prompts) = gw.connect_and_discover().await?;

// Inspect or filter before registering
let filtered_tools: Vec<_> = tools.into_iter()
    .filter(|t| !t.tool.name.contains("dangerous"))
    .collect();

// Manually register into builder
let mut builder = McpServer::builder().name("gateway").version("1.0.0");
for tool_def in filtered_tools {
    builder = builder.tool_def(tool_def);
}
let server = builder.build();
```

---

## Error Handling

The gateway is designed to be resilient:

- **Connection failures** — Upstreams that fail to connect are logged as warnings and skipped. The gateway still starts with the remaining upstreams.
- **Discovery failures** — If listing tools/resources/prompts fails for a connected upstream, that category is skipped (but other categories are still registered).
- **Runtime failures** — If an upstream tool call fails at runtime, the error is propagated back to the client as an `McpError::InternalError`.

---

## Lifecycle Management

```rust
let mut gw = GatewayManager::new();
// ... configure and build ...

// Check connection status
println!("Configured: {}", gw.configured_count());
println!("Connected: {}", gw.connected_count());

// Clean shutdown — close all upstream connections
gw.close_all().await;
```

---

## Feature Flags

```toml
[dependencies]
mcp-kit-gateway = { version = "0.1", default-features = false, features = ["sse", "websocket"] }
```

| Feature | Description |
|---------|-------------|
| `full` (default) | All transport features |
| `sse` | SSE upstream support |
| `websocket` | WebSocket upstream support |
| `streamable-http` | Streamable HTTP upstream support |
| `stdio` | Stdio subprocess upstream support |

Features map directly to `mcp-kit-client` transport features.

---

## Running the Example

```bash
# 1. Start an upstream server (e.g., the showcase example)
cargo run --example showcase -- --sse  # Starts on port 3001

# 2. In another terminal, start the gateway
UPSTREAM_URL=http://localhost:3001/sse cargo run -p mcp-kit-gateway --example gateway

# The gateway exposes all upstream tools on http://localhost:3000/sse
# Tools are namespaced: greet -> upstream/greet, echo -> upstream/echo, etc.
```

Environment variables:
- `UPSTREAM_URL` — URL of the upstream server (default: `http://localhost:3001/sse`)
- `UPSTREAM_NAME` — Name/prefix for the upstream (default: `upstream`)

---

## License

MIT — see [LICENSE](../LICENSE).

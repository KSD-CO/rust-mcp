# rust-mcp

A Rust library for building [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) servers.

[![Crates.io](https://img.shields.io/crates/v/mcp.svg)](https://crates.io/crates/mcp)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/KSD-CO/rust-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/KSD-CO/rust-mcp/actions/workflows/ci.yml)

```toml
[dependencies]
rust-mcp = "0.1"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
schemars = "0.8"
```

---

## Quick Start

```rust
use mcp::prelude::*;
use serde::Deserialize;
use schemars::JsonSchema;

#[derive(Deserialize, JsonSchema)]
struct AddInput {
    /// First operand
    a: f64,
    /// Second operand
    b: f64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let schema = serde_json::to_value(schemars::schema_for!(AddInput))?;

    McpServer::builder()
        .name("calculator")
        .version("1.0.0")
        .tool(
            Tool::new("add", "Add two numbers", schema),
            |p: AddInput| async move {
                CallToolResult::text(format!("{}", p.a + p.b))
            },
        )
        .build()
        .serve_stdio()
        .await?;

    Ok(())
}
```

---

## Table of Contents

- [Tools](#tools)
- [Resources](#resources)
- [Prompts](#prompts)
- [Transports](#transports)
- [Error Handling](#error-handling)
- [Tracing / Logging](#tracing--logging)
- [Builder Reference](#builder-reference)
- [Crate Layout](#crate-layout)

---

## Tools

Tools are functions the AI model can invoke. Each tool has a name, description, and a JSON Schema describing its input parameters.

### Typed handler (recommended)

Define a struct that implements `Deserialize` + `JsonSchema`. The schema is generated automatically:

```rust
use mcp::prelude::*;
use serde::Deserialize;
use schemars::JsonSchema;

#[derive(Deserialize, JsonSchema)]
struct SearchInput {
    /// Search query
    query: String,
    /// Maximum number of results
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 { 10 }

let schema = serde_json::to_value(schemars::schema_for!(SearchInput))?;

McpServer::builder()
    .tool(
        Tool::new("search", "Search the documentation", schema),
        |p: SearchInput| async move {
            let results = do_search(&p.query, p.limit).await;
            CallToolResult::text(results.join("\n"))
        },
    )
    .build();
```

### Raw JSON handler

For cases where you need direct access to the arguments object:

```rust
.tool(
    Tool::new("ping", "Check connectivity", serde_json::json!({
        "type": "object",
        "properties": {}
    })),
    |_args: serde_json::Value| async move {
        CallToolResult::text("pong")
    },
)
```

### Return types

Handlers can return any type that implements `IntoToolResult`:

```rust
// Plain string
|_| async move { "hello world".to_string() }

// Full CallToolResult
|_| async move { CallToolResult::text("ok") }

// Result<T, E> — errors are automatically converted to in-band tool errors
|p: Input| async move -> anyhow::Result<CallToolResult> {
    let data = fetch(&p.url).await?;
    Ok(CallToolResult::text(data))
}

// Multiple content items (text, images, etc.)
|_| async move {
    CallToolResult::success(vec![
        Content::text("Result:"),
        Content::image(base64_data, "image/png"),
    ])
}
```

### In-band errors

Return a tool-level error without raising a protocol-level failure:

```rust
|p: Input| async move {
    if p.n < 0.0 {
        return CallToolResult::error("n must be non-negative");
    }
    CallToolResult::text(format!("{}", p.n.sqrt()))
}
```

---

## Resources

Resources are data sources the model can read — files, databases, API responses, etc.

### Static resource (fixed URI)

```rust
use mcp::{prelude::*, ReadResourceRequest};

McpServer::builder()
    .resource(
        Resource::new("config://app", "App Config")
            .with_description("Application configuration")
            .with_mime_type("application/json"),
        |_req: ReadResourceRequest| async move {
            Ok(ReadResourceResult::text(
                "config://app",
                r#"{"version": "1.0", "debug": false}"#,
            ))
        },
    )
```

### URI Template resource (RFC 6570)

Use `{variable}` placeholders to handle parameterised URIs:

```rust
use mcp::{prelude::*, ReadResourceRequest};

McpServer::builder()
    .resource_template(
        ResourceTemplate::new("file://{path}", "File System"),
        |req: ReadResourceRequest| async move {
            let path = req.uri.trim_start_matches("file://");
            let content = tokio::fs::read_to_string(path).await
                .map_err(|e| McpError::ResourceNotFound(e.to_string()))?;
            Ok(ReadResourceResult::text(req.uri.clone(), content))
        },
    )
```

Clients can request `file:///home/user/notes.txt`, `file:///etc/config`, etc.

### Binary resource

```rust
|req: ReadResourceRequest| async move {
    let bytes = tokio::fs::read(&path).await?;
    let b64 = base64::encode(&bytes);
    Ok(ReadResourceResult {
        contents: vec![ResourceContents::blob(req.uri, b64, "image/png")],
    })
}
```

---

## Prompts

Prompts are parameterised message templates clients can use to start conversations.

```rust
use mcp::{prelude::*, GetPromptRequest};

McpServer::builder()
    .prompt(
        Prompt::new("code-review")
            .with_description("Generate a code review for a given snippet")
            .with_arguments(vec![
                PromptArgument::required("code")
                    .with_description("The code to review"),
                PromptArgument::optional("language")
                    .with_description("Programming language"),
            ]),
        |req: GetPromptRequest| async move {
            let code = req.arguments.get("code").cloned().unwrap_or_default();
            let lang = req.arguments.get("language")
                .cloned()
                .unwrap_or_else(|| "unknown".into());

            Ok(GetPromptResult::new(vec![
                PromptMessage::user_text(format!(
                    "Review the following {lang} code:\n\n```{lang}\n{code}\n```"
                )),
            ]))
        },
    )
```

---

## Transports

### stdio

The server is launched as a subprocess and communicates over stdin/stdout. This is the standard transport for local MCP servers.

```rust
server.serve_stdio().await?;
```

Configure in Claude Desktop (`claude_desktop_config.json`):
```json
{
  "mcpServers": {
    "my-server": {
      "command": "/path/to/my-server",
      "args": []
    }
  }
}
```

> **Note:** Always log to stderr when using the stdio transport:
> ```rust
> tracing_subscriber::fmt().with_writer(std::io::stderr).init();
> ```

### SSE / HTTP

The server exposes an HTTP endpoint. Clients open a `GET /sse` stream and send messages via `POST /message`.

```rust
let addr: std::net::SocketAddr = "0.0.0.0:3000".parse()?;
server.serve_sse(addr).await?;
```

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/sse` | Open SSE stream; the first event contains the session endpoint |
| `POST` | `/message?sessionId=<id>` | Send a JSON-RPC message |

```bash
# Open SSE stream
curl -N http://localhost:3000/sse

# Send a message
curl -X POST "http://localhost:3000/message?sessionId=<id>" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}'
```

---

## Error Handling

### McpError variants

```rust
McpError::InvalidParams("missing field 'name'".into())
McpError::ResourceNotFound("file://missing.txt".into())
McpError::ToolNotFound("unknown-tool".into())
McpError::InternalError("database connection failed".into())
McpError::Unauthorized("invalid API key".into())
```

### In a tool handler

```rust
// Explicit error type
|p: Input| async move -> Result<CallToolResult, McpError> {
    if p.url.is_empty() {
        return Err(McpError::InvalidParams("url must not be empty".into()));
    }
    Ok(CallToolResult::text("done"))
}

// With anyhow — the ? operator works out of the box
|p: Input| async move -> anyhow::Result<CallToolResult> {
    let resp = reqwest::get(&p.url).await?.text().await?;
    Ok(CallToolResult::text(resp))
}
```

### In a resource handler

```rust
|req: ReadResourceRequest| async move {
    let data = db.get(&req.uri).await
        .ok_or_else(|| McpError::ResourceNotFound(req.uri.clone()))?;
    Ok(ReadResourceResult::text(req.uri, data))
}
```

---

## Tracing / Logging

The library emits structured logs via the [`tracing`](https://docs.rs/tracing) crate.

```rust
tracing_subscriber::fmt()
    .with_writer(std::io::stderr)
    .with_env_filter(
        tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("my_server=debug".parse()?)
            .add_directive("mcp=info".parse()?),
    )
    .init();
```

```bash
RUST_LOG=my_server=debug,mcp=debug cargo run --bin my-server
```

---

## Builder Reference

```rust
McpServer::builder()
    .name("my-server")                      // name sent to clients on handshake
    .version("1.0.0")
    .instructions("What this server does")  // optional guidance for the model

    // Tools
    .tool(tool_def, handler)                // typed or raw JSON handler
    .tool_def(tool_def_from_macro)          // produced by the #[tool] macro
    .tool_fn("name", "desc", handler)       // shorthand without a custom schema

    // Resources
    .resource(resource_def, handler)        // exact URI match
    .resource_template(template, handler)   // URI with {variable} placeholders

    // Prompts
    .prompt(prompt_def, handler)

    .build()
    .serve_stdio().await?
    // or
    .serve_sse(addr).await?
```

---

## Crate Layout

| Crate | Purpose |
|-------|---------|
| `mcp` | Main entry point — re-exports everything; import this crate only |
| `mcp-core` | JSON-RPC 2.0 implementation and all MCP protocol types |
| `mcp-server` | `McpServer`, builder, handler traits, router |
| `mcp-transport` | stdio and SSE/HTTP transports |
| `mcp-macros` | `#[tool]` procedural macro |

---

## Cloudflare Workers

The [`deploy/cloudflare/`](deploy/cloudflare/) directory contains a self-contained MCP server
that runs on [Cloudflare Workers](https://workers.cloudflare.com/) using the **Streamable HTTP**
transport — no long-lived connections, no Durable Objects required.

### Transport

```
POST /mcp   accepts a JSON-RPC message, returns a JSON response
GET  /mcp   returns server name, version, and endpoint metadata
```

Each request is fully stateless. The `initialize` handshake succeeds on every request.

### Prerequisites

```bash
cargo install worker-build
npm install -g wrangler
wrangler login
```

### Local development

```bash
cd deploy/cloudflare
wrangler dev
```

```bash
# Initialise
curl -X POST http://localhost:8787/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'

# List tools
curl -X POST http://localhost:8787/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'

# Call a tool
curl -X POST http://localhost:8787/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"add","arguments":{"a":3,"b":4}}}'
```

### Deploy

```bash
wrangler deploy
```

Or push to `main` — the GitHub Actions workflow in
[`.github/workflows/deploy-cloudflare.yml`](.github/workflows/deploy-cloudflare.yml)
deploys automatically.

**Required secret:** add `CF_API_TOKEN` to your repository's Actions secrets
(Settings → Secrets → Actions).

### Defining your own tools

Edit the `build_server()` function in [`deploy/cloudflare/src/lib.rs`](deploy/cloudflare/src/lib.rs):

```rust
fn build_server() -> CloudflareServer {
    #[derive(serde::Deserialize)]
    struct MyInput { text: String }

    CloudflareServerBuilder::default()
        .name("my-server")
        .version("1.0.0")
        .tool(
            Tool::new("reverse", "Reverse a string", serde_json::json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"]
            })),
            |p: MyInput| async move {
                Ok(CallToolResult::text(p.text.chars().rev().collect::<String>()))
            },
        )
        .build()
}
```

---

## Examples

```bash
# Calculator — add, subtract, multiply, divide, power, sqrt, factorial
cargo run --bin calculator

# Everything — tools, resources, and prompts over stdio
cargo run --bin everything

# Everything — SSE mode on port 3000
cargo run --bin everything -- --sse
```

Source: [`examples/`](examples/)

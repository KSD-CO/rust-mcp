# mcp-rs

A high-quality Rust library for building [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) servers — designed in the spirit of **axum**: ergonomic, type-safe, async-first.

```toml
[dependencies]
mcp = { path = "crates/mcp" }
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

Run:
```bash
cargo run --bin my-server
```

---

## Table of Contents

- [Tools](#tools)
- [Resources](#resources)
- [Prompts](#prompts)
- [Transports](#transports)
- [Error Handling](#error-handling)
- [Tracing / Logging](#tracing--logging)
- [Crate Layout](#crate-layout)

---

## Tools

Tools are functions the AI model can invoke. Each tool has a name, description, and a JSON Schema for its input parameters.

### Typed handler (recommended)

Define a struct that implements `Deserialize` + `JsonSchema` — the schema is generated automatically:

```rust
use mcp::prelude::*;
use serde::Deserialize;
use schemars::JsonSchema;

#[derive(Deserialize, JsonSchema)]
struct SearchInput {
    /// Search query
    query: String,
    /// Maximum number of results (default 10)
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

When you need direct access to the arguments object:

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

Handlers can return several types — all implement `IntoToolResult`:

```rust
// Plain string
|_| async move { "hello world".to_string() }

// Full control
|_| async move { CallToolResult::text("ok") }

// Result<T, E> — errors become in-band tool errors automatically
|p: Input| async move -> anyhow::Result<CallToolResult> {
    let data = fetch(&p.url).await?;
    Ok(CallToolResult::text(data))
}

// Multiple content items
|_| async move {
    CallToolResult::success(vec![
        Content::text("Here is the image:"),
        Content::image(base64_data, "image/png"),
    ])
}
```

### In-band errors

Return an error result without failing at the protocol level:

```rust
|p: Input| async move {
    if p.n < 0.0 {
        // Client receives isError: true
        return CallToolResult::error("n must be non-negative");
    }
    CallToolResult::text(format!("{}", p.n.sqrt()))
}
```

---

## Resources

Resources are data sources the model can read — files, databases, APIs, etc.

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

Use `{variable}` placeholders to serve parameterised resources:

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

Clients can then request `file:///home/user/notes.txt`, `file:///etc/config`, etc.

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

Prompts are parameterised message templates that clients can use to start conversations.

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

### stdio (default — for subprocess servers)

Claude Desktop and most MCP clients launch the server as a subprocess and communicate over stdin/stdout:

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

> **Important:** log to stderr so it doesn't pollute the stdio transport:
> ```rust
> tracing_subscriber::fmt().with_writer(std::io::stderr).init();
> ```

### SSE / HTTP (for web servers)

Uses `SSE + HTTP POST`. The client opens a `GET /sse` stream and sends messages via `POST /message`:

```rust
let addr: std::net::SocketAddr = "0.0.0.0:3000".parse()?;
server.serve_sse(addr).await?;
```

Endpoints:
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/sse` | Open SSE stream; receives session ID as the first event |
| `POST` | `/message?sessionId=<id>` | Send a JSON-RPC message |

Manual test:
```bash
# Terminal 1 — open SSE stream
curl -N http://localhost:3000/sse

# Terminal 2 — send a message
curl -X POST "http://localhost:3000/message?sessionId=<id>" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}'
```

---

## Error Handling

### McpError

```rust
use mcp::McpError;

McpError::InvalidParams("missing field 'name'".into())
McpError::ResourceNotFound("file://missing.txt".into())
McpError::ToolNotFound("unknown-tool".into())
McpError::InternalError("database connection failed".into())
McpError::Unauthorized("invalid API key".into())
```

### In a tool handler

```rust
// Using McpError directly
|p: Input| async move -> Result<CallToolResult, McpError> {
    if p.url.is_empty() {
        return Err(McpError::InvalidParams("url must not be empty".into()));
    }
    Ok(CallToolResult::text("done"))
}

// Using anyhow (? operator works)
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

The library uses the [`tracing`](https://docs.rs/tracing) crate. Set up a subscriber before starting the server:

```rust
tracing_subscriber::fmt()
    .with_writer(std::io::stderr)   // must be stderr for stdio transport
    .with_env_filter(
        tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("my_server=debug".parse()?)
            .add_directive("mcp=info".parse()?),
    )
    .init();
```

Or use the `RUST_LOG` environment variable:
```bash
RUST_LOG=my_server=debug,mcp=debug cargo run --bin my-server
```

---

## Builder Reference

```rust
McpServer::builder()
    // Server identity
    .name("my-server")                      // shown to clients during handshake
    .version("1.0.0")
    .instructions("What this server does")  // hint for the model

    // Tools
    .tool(tool_def, handler)                // typed or raw JSON handler
    .tool_def(tool_def_from_macro)          // from #[tool] macro
    .tool_fn("name", "desc", handler)       // shorthand (no custom schema)

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
| `mcp` | Main entry point — re-exports everything; this is the only crate you need |
| `mcp-core` | JSON-RPC 2.0 + all MCP protocol types (Tool, Resource, Prompt, Content, …) |
| `mcp-server` | `McpServer`, builder, handler traits, router, extractors |
| `mcp-transport` | stdio and SSE/HTTP transports |
| `mcp-macros` | `#[tool]` proc-macro |

In your `Cargo.toml`:
```toml
[dependencies]
mcp = { path = "path/to/crates/mcp" }
```

---

## Examples

See the [`examples/`](examples/) directory:

```bash
# Calculator — add, subtract, multiply, divide, power, sqrt, factorial
cargo run --bin calculator

# Everything — tools + static/template resources + prompts (stdio)
cargo run --bin everything

# Everything — SSE mode on port 3000
cargo run --bin everything -- --sse
```

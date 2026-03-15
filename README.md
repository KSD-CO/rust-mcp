# mcp-kit

[![Crates.io](https://img.shields.io/crates/v/mcp-kit.svg)](https://crates.io/crates/mcp-kit)
[![Documentation](https://docs.rs/mcp-kit/badge.svg)](https://docs.rs/mcp-kit)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/KSD-CO/mcp-kit/actions/workflows/ci.yml/badge.svg)](https://github.com/KSD-CO/mcp-kit/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue)](https://www.rust-lang.org)

**An ergonomic, type-safe Rust library for building [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) servers.**

MCP enables AI assistants to securely access tools, data sources, and prompts through a standardized protocol. This library provides a modern, async-first implementation with powerful procedural macros for rapid development.

---

## 🎉 What's New in v0.2.0

**🧩 Plugin System**
- Dynamic loading of tools, resources, and prompts
- Native plugin support (.so, .dylib, .dll)
- In-process plugin registration
- Plugin configuration and lifecycle management

**📦 Real API Integrations**
- ✅ **GitHub Plugin** - Create issues, list repos, manage PRs (4 tools)
- ✅ **Jira Plugin** - Create/search issues, add comments (4 tools)
- ✅ **Confluence Plugin** - Create/search wiki pages (4 tools)
- ✅ **ClickHouse Plugin** - Run queries, generate reports, analytics (6 tools)
- All with working REST API implementations!

**🔧 Enhanced Developer Experience**
- Production-ready plugin examples
- Comprehensive plugin documentation
- Easy integration: just `export API_TOKEN` and run

---

## Features

- 🚀 **Async-first** — Built on Tokio for high-performance concurrent operations
- 🛡️ **Type-safe** — Leverage Rust's type system with automatic JSON Schema generation
- 🎯 **Ergonomic macros** — `#[tool]`, `#[resource]`, `#[prompt]` attributes for minimal boilerplate
- 🔌 **Multiple transports** — stdio, SSE/HTTP, Streamable HTTP, WebSocket, and HTTPS/TLS
- 🔐 **Authentication** — Bearer, API Key, Basic, OAuth 2.0, and mTLS support
- 🧩 **Plugin system** — Dynamic loading of tools, resources, and prompts from native libraries
- 📦 **Real API integrations** — Production-ready plugins for GitHub, Jira, and Confluence
- 📝 **Completion** — Auto-complete argument values for prompts and resources
- 📊 **Progress tracking** — Report progress for long-running operations
- 📢 **Notifications** — Push updates to clients (resource changes, log messages)
- 🔄 **Subscriptions** — Subscribe to resource changes for real-time updates
- ⛔ **Cancellation** — Cancel long-running requests
- 🤖 **Sampling** — Server-initiated LLM requests to clients
- 💬 **Elicitation** — Request user input from clients during tool execution
- 📁 **Roots** — File system sandboxing with client-provided roots
- 🧩 **Modular** — Feature-gated architecture, WASM-compatible core
- 📦 **Batteries included** — State management, error handling, tracing integration
- 🎨 **Flexible APIs** — Choose between macro-based or manual builder patterns
- 📡 **Client SDK** — `mcp-kit-client` crate for connecting to MCP servers

### 🌟 Highlights

**Plugin System** — Build modular, extensible MCP servers:
```rust
McpServer::builder()
    .load_plugin("./plugins/github.so")?     // Load from file
    .with_plugin_manager(manager)            // Or use plugin manager
    .build()
```

**Real API Integrations** — Production-ready plugins included:
```bash
# GitHub - manage repos, issues, PRs
export GITHUB_TOKEN=ghp_xxx
cargo run --example plugin_github --features plugin

# Jira - create/search issues, add comments  
export JIRA_API_TOKEN=xxx
cargo run --example plugin_jira --features plugin

# Confluence - create/search wiki pages
export CONFLUENCE_API_TOKEN=xxx
cargo run --example plugin_confluence --features plugin

# ClickHouse - run SQL queries and generate reports
export CLICKHOUSE_URL=http://localhost:8123
cargo run --example plugin_clickhouse --features plugin
```

**Type-Safe & Ergonomic** — Minimal boilerplate with macros:
```rust
#[tool(description = "Add numbers")]
async fn add(a: f64, b: f64) -> String {
    format!("{}", a + b)
}
```

---

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
mcp-kit = "0.2"  # Latest with plugin system
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
schemars = "0.8"
anyhow = "1"  # For error handling
```

For plugin development, add:
```toml
[dependencies]
mcp-kit = { version = "0.2", features = ["plugin", "plugin-native"] }
reqwest = { version = "0.12", features = ["json"] }  # For API calls
```

**Minimum Supported Rust Version (MSRV):** 1.85

---

## Quick Start

### Using Macros (Recommended)

The fastest way to build an MCP server with automatic schema generation:

```rust
use mcp_kit::prelude::*;

/// Add two numbers
#[tool(description = "Add two numbers and return the sum")]
async fn add(a: f64, b: f64) -> String {
    format!("{}", a + b)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    McpServer::builder()
        .name("calculator")
        .version("1.0.0")
        .tool_def(add_tool_def())  // Generated by #[tool] macro
        .build()
        .serve_stdio()
        .await?;
    Ok(())
}
```

### Manual API

For more control over schema and behavior:

```rust
use mcp_kit::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

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
            |params: AddInput| async move {
                CallToolResult::text(format!("{}", params.a + params.b))
            },
        )
        .build()
        .serve_stdio()
        .await?;
    Ok(())
}
```

---

## Core Concepts

### Tools

Tools are functions that AI models can invoke. Define them with the `#[tool]` macro or manually:

```rust
// Macro approach
#[tool(description = "Multiply two numbers")]
async fn multiply(x: f64, y: f64) -> String {
    format!("{}", x * y)
}

// Manual approach
let schema = serde_json::to_value(schemars::schema_for!(MultiplyInput))?;
builder.tool(
    Tool::new("multiply", "Multiply two numbers", schema),
    |params: MultiplyInput| async move {
        CallToolResult::text(format!("{}", params.x * params.y))
    }
);
```

**Error Handling:**
```rust
#[tool(description = "Divide two numbers")]
async fn divide(a: f64, b: f64) -> Result<String, String> {
    if b == 0.0 {
        return Err("Cannot divide by zero".to_string());
    }
    Ok(format!("{}", a / b))
}
```

### Resources

Resources expose data (files, APIs, databases) to AI models:

```rust
// Static resource
#[resource(
    uri = "config://app",
    name = "Application Config",
    mime_type = "application/json"
)]
async fn get_config(_req: ReadResourceRequest) -> McpResult<ReadResourceResult> {
    let config = serde_json::json!({"version": "1.0", "debug": false});
    Ok(ReadResourceResult::text(
        "config://app",
        serde_json::to_string_pretty(&config)?
    ))
}

// Template resource (dynamic URIs)
#[resource(uri = "file://{path}", name = "File System")]
async fn read_file(req: ReadResourceRequest) -> McpResult<ReadResourceResult> {
    let path = req.uri.trim_start_matches("file://");
    let content = tokio::fs::read_to_string(path).await
        .map_err(|e| McpError::ResourceNotFound(e.to_string()))?;
    Ok(ReadResourceResult::text(req.uri.clone(), content))
}
```

### Prompts

Prompts provide reusable templates for AI interactions:

```rust
#[prompt(
    name = "code-review",
    description = "Generate a code review prompt",
    arguments = ["code:required", "language:optional"]
)]
async fn code_review(req: GetPromptRequest) -> McpResult<GetPromptResult> {
    let code = req.arguments.get("code").cloned().unwrap_or_default();
    let lang = req.arguments.get("language").cloned().unwrap_or("".into());
    
    Ok(GetPromptResult::new(vec![
        PromptMessage::user_text(format!(
            "Review this {lang} code:\n\n```{lang}\n{code}\n```"
        ))
    ]))
}
```

---

## Transports

### Stdio (Default)

Standard input/output transport for local process communication:

```rust
server.serve_stdio().await?;
```

### SSE (Server-Sent Events)

HTTP-based transport for web clients:

```rust
// Requires the "sse" feature
server.serve_sse(([0, 0, 0, 0], 3000)).await?;
```

Enable in `Cargo.toml`:
```toml
[dependencies]
mcp-kit = { version = "0.1", features = ["sse"] }
```

### Streamable HTTP (MCP 2025-03-26)

Modern HTTP transport with a single endpoint that can return JSON or SSE:

```rust
// Requires the "sse" feature
server.serve_streamable(([0, 0, 0, 0], 3000)).await?;
```

**Protocol:**
```text
POST /mcp
Content-Type: application/json
Mcp-Session-Id: <optional session id>

{"jsonrpc":"2.0","method":"tools/list","id":1}

Response (JSON for simple requests):
200 OK
Content-Type: application/json
Mcp-Session-Id: <session id>

{"jsonrpc":"2.0","result":{"tools":[...]},"id":1}

Response (SSE for streaming):
200 OK
Content-Type: text/event-stream
Mcp-Session-Id: <session id>

data: {"jsonrpc":"2.0","method":"notifications/progress",...}
data: {"jsonrpc":"2.0","result":{...},"id":1}
```

**Advantages over SSE:**
- Single endpoint (instead of `/sse` + `/message`)
- Server chooses JSON or stream per-request
- Better for serverless/edge deployments
- Session management via `Mcp-Session-Id` header

### TLS/HTTPS

Secure HTTPS transport with optional mTLS:

```rust
use mcp_kit::transport::tls::{TlsConfig, ServeSseTlsExt};

let tls = TlsConfig::builder()
    .cert_pem("server.crt")
    .key_pem("server.key")
    .client_auth_ca_pem("ca.crt")  // Enable mTLS
    .build()?;

server.serve_tls("0.0.0.0:8443".parse()?, tls).await?;
```

### WebSocket

Bidirectional WebSocket transport for real-time communication:

```rust
// Requires the "websocket" feature
server.serve_websocket("0.0.0.0:3001".parse()?).await?;
```

Enable in `Cargo.toml`:
```toml
[dependencies]
mcp-kit = { version = "0.1", features = ["websocket"] }
```

---

## Authentication

Protect your MCP server with various authentication methods. All auth features are composable and can be combined.

### Bearer Token Authentication

```rust
use mcp_kit::prelude::*;
use mcp_kit::auth::{BearerTokenProvider, IntoDynProvider};
use mcp_kit::Auth;
use std::sync::Arc;

// Protected tool - requires auth parameter
#[tool(description = "Say hello to the authenticated user")]
async fn greet(message: String, auth: Auth) -> McpResult<CallToolResult> {
    Ok(CallToolResult::text(format!(
        "Hello, {}! Message: {}", auth.subject, message
    )))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let provider = Arc::new(BearerTokenProvider::new(["my-secret-token"]));

    McpServer::builder()
        .name("secure-server")
        .version("1.0.0")
        .auth(provider.into_dyn())
        .tool_def(greet_tool_def())
        .build()
        .serve_sse("0.0.0.0:3000".parse()?)
        .await?;
    Ok(())
}
```

Test with: `curl -H "Authorization: Bearer my-secret-token" http://localhost:3000/sse`

### API Key Authentication

```rust
use mcp_kit::auth::{ApiKeyProvider, IntoDynProvider};

// Supports both header and query param
let provider = Arc::new(ApiKeyProvider::new(["api-key-123", "api-key-456"]));

McpServer::builder()
    .auth(provider.into_dyn())
    // ...
```

Test with:
- Header: `curl -H "X-Api-Key: api-key-123" http://localhost:3000/sse`
- Query: `curl "http://localhost:3000/sse?api_key=api-key-123"`

### Basic Authentication

```rust
use mcp_kit::auth::{AuthenticatedIdentity, BasicAuthProvider, IntoDynProvider};

let provider = Arc::new(BasicAuthProvider::new(|username, password| {
    Box::pin(async move {
        if username == "admin" && password == "secret" {
            Ok(AuthenticatedIdentity::new("admin")
                .with_scopes(["read", "write", "admin"]))
        } else {
            Err(McpError::Unauthorized("invalid credentials".into()))
        }
    })
}));
```

Test with: `curl -u admin:secret http://localhost:3000/sse`

### OAuth 2.0 (JWT/JWKS)

```rust
use mcp_kit::auth::oauth2::{OAuth2Config, OAuth2Provider};

// JWT validation with JWKS endpoint
let provider = Arc::new(OAuth2Provider::new(OAuth2Config::Jwt {
    jwks_url: "https://auth.example.com/.well-known/jwks.json".to_owned(),
    required_audience: Some("https://my-api.example.com".to_owned()),
    required_issuer: Some("https://auth.example.com/".to_owned()),
    jwks_refresh_secs: 3600,
}));

// Or token introspection (RFC 7662)
let provider = Arc::new(OAuth2Provider::new(OAuth2Config::Introspection {
    introspection_url: "https://auth.example.com/introspect".to_owned(),
    client_id: "my-client".to_owned(),
    client_secret: "my-secret".to_owned(),
    cache_ttl_secs: 60,
}));
```

### mTLS (Mutual TLS)

```rust
use mcp_kit::auth::mtls::MtlsProvider;
use mcp_kit::transport::tls::{TlsConfig, ServeSseTlsExt};

let mtls = MtlsProvider::new(|cert_der: &[u8]| {
    // Validate client certificate, extract subject
    Ok(AuthenticatedIdentity::new("client-cn"))
});

let tls = TlsConfig::builder()
    .cert_pem("server.crt")
    .key_pem("server.key")
    .client_auth_ca_pem("ca.crt")
    .build()?;

McpServer::builder()
    .auth(Arc::new(mtls))
    .build()
    .serve_tls("0.0.0.0:8443".parse()?, tls)
    .await?;
```

### Composite Authentication

Combine multiple auth methods:

```rust
use mcp_kit::auth::{
    BearerTokenProvider, ApiKeyProvider, BasicAuthProvider,
    CompositeAuthProvider, IntoDynProvider,
};

let composite = CompositeAuthProvider::new(vec![
    BearerTokenProvider::new(["service-token"]).into_dyn(),
    ApiKeyProvider::new(["api-key"]).into_dyn(),
    BasicAuthProvider::new(/* validator */).into_dyn(),
]);

McpServer::builder()
    .auth(Arc::new(composite))
    // ...
```

### Auth Extractor in Tools

Access authentication info in tool handlers:

```rust
use mcp_kit::Auth;

#[tool(description = "Protected operation")]
async fn secure_op(data: String, auth: Auth) -> McpResult<CallToolResult> {
    // Access authenticated identity
    println!("User: {}", auth.subject);
    println!("Scopes: {:?}", auth.scopes);
    println!("Metadata: {:?}", auth.metadata);
    
    // Check scopes
    if !auth.has_scope("write") {
        return Err(McpError::Unauthorized("write scope required".into()));
    }
    
    Ok(CallToolResult::text("Success!"))
}
```

---

## Completion

Provide auto-complete suggestions for prompt and resource arguments:

```rust
use mcp_kit::prelude::*;
use mcp_kit::types::messages::{CompleteRequest, CompletionReference};

McpServer::builder()
    .name("completion-demo")
    .version("1.0.0")
    // Prompt with completion handler
    .prompt_with_completion(
        Prompt::new("search")
            .with_description("Search with auto-complete")
            .with_arguments(vec![
                PromptArgument::required("query"),
                PromptArgument::optional("category"),
            ]),
        // Prompt handler
        |req: mcp_kit::types::messages::GetPromptRequest| async move {
            Ok(GetPromptResult::new(vec![
                PromptMessage::user_text(format!("Search: {}", req.arguments.get("query").unwrap()))
            ]))
        },
        // Completion handler
        |req: CompleteRequest| async move {
            let values = match req.argument.name.as_str() {
                "category" => vec!["books", "movies", "music", "games"],
                _ => vec![],
            };
            Ok(CompleteResult::new(values))
        },
    )
    // Global completion for resources
    .completion(|req: CompleteRequest| async move {
        match &req.reference {
            CompletionReference::Resource { uri } if uri.starts_with("file://") => {
                Ok(CompleteResult::new(vec!["file:///src/", "file:///docs/"]))
            }
            _ => Ok(CompleteResult::empty()),
        }
    })
    .build();
```

---

## Notifications

Push updates from server to clients:

```rust
use mcp_kit::prelude::*;

// Create notification channel
let (notifier, mut receiver) = NotificationSender::channel(100);

// In a tool handler - notify about resource changes
async fn update_data(notifier: NotificationSender) {
    // ... update data ...
    
    // Notify clients the resource changed
    notifier.resource_updated("data://config").await.ok();
    
    // Notify about list changes
    notifier.resources_list_changed().await.ok();
    notifier.tools_list_changed().await.ok();
    notifier.prompts_list_changed().await.ok();
    
    // Send log messages
    notifier.log_info("update", "Data updated successfully").await.ok();
    notifier.log_warning("update", "Some items skipped").await.ok();
}
```

**Available Notifications:**
- `resource_updated(uri)` — A specific resource's content changed
- `resources_list_changed()` — Available resources list changed
- `tools_list_changed()` — Available tools list changed  
- `prompts_list_changed()` — Available prompts list changed
- `log_debug/info/warning/error()` — Log messages

---

## Progress Tracking

Report progress for long-running operations:

```rust
use mcp_kit::prelude::*;

async fn process_files(notifier: NotificationSender, files: Vec<String>) {
    let tracker = ProgressTracker::new(notifier, Some("token-123".into()));
    
    for (i, file) in files.iter().enumerate() {
        // Process file...
        
        // Report progress
        tracker.update_with_message(
            i as f64 + 1.0,
            files.len() as f64,
            format!("Processing {}", file),
        ).await;
    }
    
    tracker.complete("All files processed").await;
}
```

**ProgressTracker Methods:**
- `update(progress, total, message)` — Send progress update
- `update_percent(0.0..1.0, message)` — Progress as percentage
- `complete(message)` — Mark operation complete
- `is_tracking()` — Check if progress token was provided
```

---

## Elicitation

Request user input from clients during tool execution:

```rust
use mcp_kit::prelude::*;

// Create elicitation client
let (client, mut rx) = ChannelElicitationClient::channel(10);

// Simple yes/no confirmation
let confirmed = client.confirm("Delete all temporary files?").await?;
if confirmed {
    // User confirmed, proceed
}

// Request text input
if let Some(name) = client.prompt_text("Enter project name").await? {
    println!("Creating project: {}", name);
}

// Multiple choice
let options = vec!["small".into(), "medium".into(), "large".into()];
if let Some(size) = client.choose("Select deployment size", options).await? {
    println!("Selected: {}", size);
}

// Complex form with builder
let request = ElicitationRequestBuilder::new("Configure your project")
    .text_required("name", "Project Name")
    .boolean("private", "Private Repository")
    .number("port", "Port Number")
    .select("language", "Language", &["rust", "python", "javascript"])
    .build();

let result = client.elicit(request).await?;
if result.is_accepted() {
    // Process user input from result.content
}
```

**ElicitationClientExt Methods:**
- `confirm(message)` — Yes/no confirmation dialog
- `prompt_text(message)` — Request text input
- `prompt_number(message)` — Request numeric input
- `choose(message, options)` — Multiple choice selection
- `elicit(request)` — Send custom elicitation request

---

## Advanced Features

### State Management

Share state across tool invocations:

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
struct AppState {
    counter: Arc<Mutex<i32>>,
}

// In your tool handler
let state = AppState { counter: Arc::new(Mutex::new(0)) };

builder.tool(
    Tool::new("increment", "Increment counter", schema),
    {
        let state = state.clone();
        move |_: serde_json::Value| {
            let state = state.clone();
            async move {
                let mut counter = state.counter.lock().await;
                *counter += 1;
                CallToolResult::text(format!("Counter: {}", *counter))
            }
        }
    }
);
```

### Logging

Integrate with `tracing` for structured logging:

```rust
tracing_subscriber::fmt()
    .with_writer(std::io::stderr)  // Log to stderr for stdio transport
    .with_env_filter("my_server=debug,mcp_kit=info")
    .init();

tracing::info!("Server starting");
```

Set log level:
```bash
RUST_LOG=my_server=debug cargo run
```

### Error Handling

The library uses `McpResult<T>` and `McpError`:

```rust
use mcp_kit::{McpError, McpResult};

async fn my_tool() -> McpResult<CallToolResult> {
    // Automatic conversion from std::io::Error, serde_json::Error, etc.
    let data = tokio::fs::read_to_string("file.txt").await?;
    
    // Custom errors
    if data.is_empty() {
        return Err(McpError::InvalidParams("File is empty".into()));
    }
    
    Ok(CallToolResult::text(data))
}
```

---

## Plugin System

The plugin system allows you to dynamically load and manage tools, resources, and prompts from external libraries or in-process modules.

### Quick Start

```rust
use mcp_kit::prelude::*;
use mcp_kit::plugin::{McpPlugin, PluginConfig, PluginManager, ToolDefinition};

// Define a plugin
struct WeatherPlugin;

impl McpPlugin for WeatherPlugin {
    fn name(&self) -> &str { "weather" }
    fn version(&self) -> &str { "1.0.0" }
    
    fn register_tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition::new(
                Tool::new("get_weather", "Get current weather", schema),
                |params: WeatherInput| async move {
                    CallToolResult::text(format!("Weather: {}", params.city))
                },
            ),
        ]
    }
    
    fn on_load(&mut self, config: &PluginConfig) -> McpResult<()> {
        // Initialize from config
        Ok(())
    }
}

// Load plugin into server
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut plugin_manager = PluginManager::new();
    
    // Register in-process plugin
    plugin_manager.register_plugin(WeatherPlugin, PluginConfig::default())?;
    
    // Or load from dynamic library
    // plugin_manager.load_from_path("./plugins/weather.so")?;
    
    let server = McpServer::builder()
        .name("my-server")
        .with_plugin_manager(plugin_manager)
        .build()
        .serve_stdio()
        .await?;
    
    Ok(())
}
```

### Real-World Plugin Examples

The library includes **PRODUCTION-READY plugins with REAL API integration**:

**✅ Weather Plugin** — Fully working example with mock API
```bash
cargo run --example plugin_weather --features plugin,plugin-native
```
- Get current weather for cities
- Get multi-day forecasts
- Mock implementation (works out of the box)

**✅ GitHub Plugin (Real API)** — Production-ready GitHub REST API v3
```bash
export GITHUB_TOKEN=ghp_your_token_here
cargo run --example plugin_github --features plugin,plugin-native
```
- ✅ Get repository info with live data
- ✅ List user repositories
- ✅ Create issues
- ✅ List pull requests

**✅ Jira Plugin (Real API)** — Production-ready Jira REST API v3
```bash
export JIRA_BASE_URL="https://your-domain.atlassian.net"
export JIRA_EMAIL="your-email@example.com"
export JIRA_API_TOKEN="your-api-token"
export JIRA_PROJECT_KEY="PROJ"
cargo run --example plugin_jira --features plugin,plugin-native
```
- ✅ Create issues with real data
- ✅ Get issue details
- ✅ Search issues with JQL
- ✅ Add comments

**✅ Confluence Plugin (Real API)** — Production-ready Confluence REST API
```bash
export CONFLUENCE_BASE_URL="https://your-domain.atlassian.net"
export CONFLUENCE_EMAIL="your-email@example.com"
export CONFLUENCE_API_TOKEN="your-api-token"
export CONFLUENCE_SPACE_KEY="TEAM"
cargo run --example plugin_confluence --features plugin,plugin-native
```
- ✅ Create wiki pages
- ✅ Get page content
- ✅ Search with CQL
- ✅ List pages in space

**✅ ClickHouse Plugin (Real Database)** — Production-ready ClickHouse integration
```bash
# Start ClickHouse (Docker):
docker run -d -p 8123:8123 clickhouse/clickhouse-server

# Configure and run:
export CLICKHOUSE_URL="http://localhost:8123"
export CLICKHOUSE_DATABASE="default"
cargo run --example plugin_clickhouse --features plugin,plugin-native
```
- ✅ Execute SQL queries
- ✅ Get table schema and stats
- ✅ Generate analytics reports (daily/hourly/top users)
- ✅ List all tables
- ✅ Database statistics
- ✅ Insert data

See [`examples/PLUGINS.md`](examples/PLUGINS.md) for detailed setup guides.

---
```bash
cargo run --example plugin_jira --features plugin,plugin-native
```
- Create, update, search issues
- Manage sprints & transitions
- Add comments & attachments
- Ready for Jira REST API integration

**🚧 Confluence Plugin** — Complete template (8 tools, 559 lines)
```bash
cargo run --example plugin_confluence --features plugin,plugin-native
```
- Create/update wiki pages
- Search with CQL
- Manage spaces & attachments
- Ready for Confluence REST API integration

**✅ GitHub Plugin (Mock)** — Extended template with 8 tools
```bash
cargo run --example plugin_github --features plugin,plugin-native
```
- Manage repos, issues, PRs
- List commits & branches
- Trigger GitHub Actions
- Uses mock data (for reference and learning)

See [`examples/PLUGINS.md`](examples/PLUGINS.md) for detailed guides on each plugin.

### Plugin Configuration

Pass configuration at load time:

```rust
let config = PluginConfig {
    config: serde_json::json!({
        "api_key": "secret-key-123",
        "base_url": "https://api.example.com"
    }),
    enabled: true,
    priority: 10,  // Higher = loads first
    permissions: PluginPermissions {
        network: true,
        filesystem: false,
        ..Default::default()
    },
};

plugin_manager.register_plugin(MyPlugin::new(), config)?;
```

### Builder Integration

Load plugins directly in the builder:

```rust
McpServer::builder()
    .name("my-server")
    .load_plugin("./plugins/jira.so")?       // Load from file
    .load_plugin("./plugins/github.so")?     // Chain multiple
    .build()
```

### Plugin Management

```rust
// List all loaded plugins
for plugin in plugin_manager.list_plugins() {
    println!("{} v{}: {} tools, {} resources",
        plugin.name, plugin.version,
        plugin.tool_count, plugin.resource_count);
}

// Get plugin metadata
if let Some(meta) = plugin_manager.get_metadata("weather") {
    println!("Weather plugin: {:?}", meta);
}

// Unload a plugin
plugin_manager.unload("weather")?;
```

### Native Plugin (Shared Library)

Create a plugin as a `.so`, `.dylib`, or `.dll`:

```rust
// Plugin crate: lib.rs
use mcp_kit::plugin::McpPlugin;

struct MyPlugin;
impl McpPlugin for MyPlugin { /* ... */ }

// Export constructor
#[no_mangle]
pub extern "C" fn _mcp_plugin_create() -> *mut dyn McpPlugin {
    Box::into_raw(Box::new(MyPlugin))
}
```

Build as dynamic library:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
mcp-kit = { version = "0.1", features = ["plugin"] }
```

```bash
cargo build --release
# Produces: target/release/libmy_plugin.so
```

Load in server:

```rust
plugin_manager.load_from_path("./target/release/libmy_plugin.so")?;
```

### Feature Flags

```toml
[dependencies]
mcp-kit = { version = "0.1", features = ["plugin", "plugin-native"] }
```

**Available plugin features:**
- `plugin` — Core plugin system (required)
- `plugin-native` — Load native shared libraries
- `plugin-wasm` — Load WASM plugins (coming soon)
- `plugin-hot-reload` — Development hot reload (coming soon)

### Plugin Resources

- 📖 [Plugin System Documentation](docs/PLUGINS.md) — Complete guide
- 📦 [Plugin Examples](examples/PLUGINS.md) — Jira, Confluence, GitHub templates
- 🔌 Example: [`examples/plugin_weather.rs`](examples/plugin_weather.rs) — Working example

---

## Macro Reference

### `#[tool]`

Generate tools from async functions:

```rust
#[tool(description = "Description here")]
async fn my_tool(param: Type) -> ReturnType {
    // Implementation
}
```

**Attributes:**
- `description = "..."` — Tool description (required)
- `name = "..."` — Tool name (optional, defaults to function name)

**Supported return types:**
- `String` → Converted to `CallToolResult::text`
- `CallToolResult` → Used directly
- `Result<T, E>` → Error handling support

### `#[resource]`

Generate resource handlers:

```rust
#[resource(
    uri = "scheme://path",
    name = "Resource Name",
    description = "Optional description",
    mime_type = "text/plain"
)]
async fn handler(req: ReadResourceRequest) -> McpResult<ReadResourceResult> {
    // Implementation
}
```

**URI Templates:**
Use `{variable}` syntax for dynamic resources:
```rust
#[resource(uri = "file://{path}", name = "Files")]
```

### `#[prompt]`

Generate prompt handlers:

```rust
#[prompt(
    name = "prompt-name",
    description = "Prompt description",
    arguments = ["arg1:required", "arg2:optional"]
)]
async fn handler(req: GetPromptRequest) -> McpResult<GetPromptResult> {
    // Implementation
}
```

---

## Builder API Reference

```rust
McpServer::builder()
    // Server metadata
    .name("server-name")
    .version("1.0.0")
    .instructions("What this server does")

    // Register tools
    .tool(tool, handler)           // Manual API
    .tool_def(macro_generated_def) // From #[tool] macro

    // Register resources
    .resource(resource, handler)           // Static resource
    .resource_template(template, handler)  // URI template
    .resource_def(macro_generated_def)     // From #[resource] macro

    // Register prompts
    .prompt(prompt, handler)
    .prompt_def(macro_generated_def)  // From #[prompt] macro

    .build()
```

---

## Examples

Run the included examples to see all features in action:

```bash
# Comprehensive showcase - all features
cargo run --example showcase

# Showcase with SSE transport on port 3000
cargo run --example showcase -- --sse

# WebSocket transport example
cargo run --example websocket

# Macro-specific examples
cargo run --example macros_demo

# Completion auto-complete example
cargo run --example completion

# Notifications and progress example
cargo run --example notifications

# Authentication examples
cargo run --example auth_bearer --features auth-full
cargo run --example auth_apikey --features auth-full
cargo run --example auth_basic --features auth-full
cargo run --example auth_composite --features auth-full
cargo run --example auth_oauth2 --features auth-oauth2
cargo run --example auth_mtls --features auth-mtls

# Plugin examples
cargo run --example plugin_weather --features plugin,plugin-native          # ✅ Working (mock)
cargo run --example plugin_github --features plugin,plugin-native      # ✅ Real GitHub API
cargo run --example plugin_jira --features plugin,plugin-native        # ✅ Real Jira API
cargo run --example plugin_confluence --features plugin,plugin-native  # ✅ Real Confluence API
cargo run --example plugin_clickhouse --features plugin,plugin-native       # ✅ Real ClickHouse DB

# Client SDK example (requires running server first)
cargo run -p mcp-kit-client --example client_demo
```

**Example Features:**
- ✅ Multiple tool types (math, async, state management)
- ✅ Static and template resources
- ✅ Prompts with arguments
- ✅ Argument completion (auto-complete)
- ✅ Notifications (resource updates, logging)
- ✅ Progress tracking for long operations
- ✅ Resource subscriptions
- ✅ Request cancellation
- ✅ Error handling patterns
- ✅ State sharing between requests
- ✅ JSON content types
- ✅ Stdio, SSE, and WebSocket transports
- ✅ Bearer, API Key, Basic, OAuth 2.0, mTLS authentication
- ✅ Composite authentication (multiple methods)
- ✅ Plugin system (native and WASM)
- ✅ Real API integrations (GitHub, Jira, Confluence, ClickHouse)
- ✅ Database integrations (ClickHouse)
- ✅ Client SDK for connecting to servers

Source code: [`examples/`](examples/)

---

## Feature Flags

Control which features to compile:

```toml
[dependencies]
mcp-kit = { version = "0.2", default-features = false, features = ["server", "stdio"] }
```

**Available features:**
- `full` (default) — All features enabled
- `server` — Core server functionality
- `stdio` — Standard I/O transport
- `sse` — HTTP Server-Sent Events transport
- `websocket` — WebSocket transport

**Authentication features:**
- `auth` — Core auth types and traits
- `auth-bearer` — Bearer token authentication
- `auth-apikey` — API key authentication
- `auth-basic` — HTTP Basic authentication
- `auth-oauth2` — OAuth 2.0 (JWT/JWKS + introspection)
- `auth-mtls` — Mutual TLS / client certificates
- `auth-full` — All auth features (bearer, apikey, basic)

**Plugin features:**
- `plugin` — Core plugin system (trait, manager, lifecycle)
- `plugin-native` — Load native shared libraries (.so, .dylib, .dll)
- `plugin-wasm` — Load WASM plugins (coming soon)
- `plugin-hot-reload` — Hot reload during development (coming soon)

**WASM compatibility:**
Use `default-features = false` for WASM targets (only core protocol types).

---

## Architecture

```
mcp-kit/
├── src/
│   ├── lib.rs           # Public API and re-exports
│   ├── error.rs         # Error types
│   ├── protocol.rs      # JSON-RPC 2.0 implementation
│   ├── types/           # MCP protocol types
│   ├── server/          # Server implementation [feature = "server"]
│   └── transport/       # Transport implementations
├── macros/              # Procedural macros crate
└── client/              # Client SDK crate
```

**Crate structure:**
- `mcp-kit` — Main server library
- `mcp-kit-macros` — Procedural macros (`#[tool]`, etc.)
- `mcp-kit-client` — Client SDK for connecting to MCP servers

---

## Client SDK

The `mcp-kit-client` crate provides a client library for connecting to MCP servers:

```toml
[dependencies]
mcp-kit-client = "0.1"
```

### Quick Start

```rust
use mcp_kit_client::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Connect via WebSocket
    let client = McpClient::websocket("ws://localhost:3001/ws").await?;
    
    // Initialize connection
    let server_info = client.initialize("my-app", "1.0.0").await?;
    println!("Connected to: {}", server_info.name);
    
    // List and call tools
    let tools = client.list_tools().await?;
    let result = client.call_tool("greet", serde_json::json!({
        "name": "World"
    })).await?;
    
    Ok(())
}
```

### Transport Options

```rust
// Stdio - spawn subprocess
let client = McpClient::stdio("/path/to/mcp-server").await?;

// SSE - HTTP Server-Sent Events
let client = McpClient::sse("http://localhost:3000").await?;

// WebSocket
let client = McpClient::websocket("ws://localhost:3001/ws").await?;
```

### Available Operations

| Method | Description |
|--------|-------------|
| `initialize()` | Initialize the MCP connection |
| `list_tools()` | List available tools |
| `call_tool()` | Call a tool with arguments |
| `list_resources()` | List available resources |
| `read_resource()` | Read a resource by URI |
| `list_prompts()` | List available prompts |
| `get_prompt()` | Get a prompt by name |
| `subscribe()` | Subscribe to resource updates |
| `unsubscribe()` | Unsubscribe from updates |

See [`client/README.md`](client/README.md) for full documentation.

---

## Testing

```bash
# Run all tests
cargo test --workspace --all-features

# Check formatting
cargo fmt --all -- --check

# Run lints
cargo clippy --workspace --all-features -- -D warnings

# Check MSRV
cargo check --workspace --all-features
```

---

## Resources

- **MCP Specification:** https://modelcontextprotocol.io/
- **Documentation:** https://docs.rs/mcp-kit
- **Repository:** https://github.com/KSD-CO/mcp-kit
- **Examples:** [`examples/`](examples/)
- **CI/CD:** [GitHub Actions](.github/workflows/)

---

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Ensure `cargo fmt` and `cargo clippy` pass
5. Submit a pull request

See [`AGENTS.md`](AGENTS.md) for development guidelines.

---

## License

This project is licensed under the [MIT License](LICENSE).

---

## Changelog

See [GitHub Releases](https://github.com/KSD-CO/mcp-kit/releases) for version history.

---

<div align="center">

**Built with ❤️ in Rust**

[⭐ Star on GitHub](https://github.com/KSD-CO/mcp-kit) • [📦 View on crates.io](https://crates.io/crates/mcp-kit) • [📖 Read the docs](https://docs.rs/mcp-kit)

</div>

# AGENTS.md

Essential information for AI coding agents working in the `mcp-kit` repository.

## Project Overview

**mcp-kit** is a Rust library for building MCP (Model Context Protocol) servers with an ergonomic, type-safe, async-first API.

- **Language:** Rust (Edition 2021, MSRV 1.85)
- **License:** MIT
- **Workspace:** 3 crates — `mcp-kit` (main lib), `macros/` (proc-macros), `client/` (client SDK). `deploy/cloudflare/` is excluded from workspace.
- **Architecture:** Feature-gated library. Core types (`error`, `protocol`, `types/`) are always compiled and WASM-safe. `server/`, `transport/`, `auth/`, `plugin/` modules are feature-gated.
- **Core deps:** `serde`, `serde_json`, `thiserror`, `tracing`, `schemars`

## Build, Test & Lint Commands

```bash
# Quick local CI (preferred — uses Makefile)
make ci                                        # fmt + clippy + test
make check                                     # fmt + clippy only
make fix                                       # Auto-fix formatting and clippy

# Build
cargo build --workspace --all-features         # Full workspace build
cargo build --examples --all-features          # Build all examples
cargo build --example showcase                 # Build specific example

# Test
cargo test --workspace --all-features          # Run all tests (CI command)
cargo test test_websocket_call_tool            # Run a single test by name
cargo test test_sse                            # Run tests matching prefix
cargo test --test integration_tests            # Run only integration test file
cargo test -- --nocapture                      # Show println/tracing output

# Lint & Format (CI enforced)
cargo fmt --all -- --check                     # Check formatting
cargo fmt --all                                # Auto-format
cargo clippy --workspace --all-features -- -D warnings  # Clippy (deny warnings)

# Run Examples
cargo run --example showcase                   # Stdio transport
cargo run --example showcase -- --sse          # SSE transport on :3000
cargo run --example websocket                  # WebSocket transport
```

## CI Pipeline

GitHub Actions (`.github/workflows/ci.yml`) runs on **ubuntu-latest** only:
1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-features -- -D warnings`
3. `cargo test --workspace --all-features`
4. `cargo build --examples --all-features`

All PRs must pass these checks. No `rustfmt.toml` or `clippy.toml` — default configs are used.

## Code Style Guidelines

### Imports

```rust
use std::sync::Arc;                            // Std library first
use serde::{Deserialize, Serialize};           // External crates second
use crate::error::{McpError, McpResult};       // Internal crate last
use mcp_kit::prelude::*;                       // Prefer glob for prelude
```

### Naming Conventions

- **Types/Traits:** `PascalCase` — `McpServer`, `ToolHandler`, `AuthProvider`
- **Functions/Variables:** `snake_case` — `build_server`, `tool_name`
- **Constants:** `SCREAMING_SNAKE_CASE` — `MCP_PROTOCOL_VERSION`, `JSONRPC_VERSION`
- **Modules:** `snake_case` — `mod error;`, `mod auth_context;`
- **Features:** `kebab-case` — `auth-bearer`, `plugin-native`

### Types & Derives

```rust
// Public serializable types — comprehensive derives
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool { /* ... */ }

// Input types for tool/resource/prompt handlers — need JsonSchema for schema gen
#[derive(Deserialize, JsonSchema)]
struct ToolInput {
    /// Document all fields with rustdoc
    query: String,
}

// Common serde attributes used throughout
#[serde(rename_all = "camelCase")]
#[serde(skip_serializing_if = "Option::is_none")]
#[serde(tag = "type")]
```

### Error Handling

```rust
// Library errors use thiserror + McpResult alias
pub type McpResult<T> = Result<T, McpError>;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),          // #[from] for auto-conversion
}

// Tool handlers can return any Result<T, E> where E: Display
// Errors are auto-converted to CallToolResult::error(msg) via IntoToolResult trait
|params: Input| async move -> anyhow::Result<CallToolResult> {
    let data = fetch(&params.url).await?;   // ? works naturally
    Ok(CallToolResult::text(data))
}
```

### Documentation & Formatting

```rust
//! Module-level docs at top of file with examples

// Section separators for visual organization (used throughout codebase)
// ─── Section Name ────────────────────────────────────────────────────────

/// Document all public items with rustdoc.
/// Use [`TypeName`] cross-references for linking.
pub struct McpServer { /* ... */ }
```

- **Formatting:** Default `rustfmt` — 4 spaces, 100 char line width
- **Logging:** Always to **stderr** when using stdio transport. Use `tracing` macros with structured fields: `debug!(method = %req.method, "Dispatching request")`

### Feature Gates — Three-Tier Architecture

```rust
// Tier 1: Always compiled, WASM-safe (no feature gate)
pub mod error;
pub mod protocol;
pub mod types;

// Tier 2: Server (requires "server" feature, adds uuid dep)
#[cfg(feature = "server")]
pub mod server;

// Tier 3: Transport/Auth/Plugin (various features, adds tokio/axum)
#[cfg(any(feature = "stdio", feature = "sse", feature = "websocket"))]
pub mod transport;
#[cfg(feature = "auth")]
pub mod auth;
#[cfg(feature = "plugin")]
pub mod plugin;
```

### Async Patterns

- **Runtime:** Tokio (feature-gated, not in core types)
- **Entry point:** `#[tokio::main]` in examples
- **Handlers:** Async closures — `|params: T| async move { ... }`
- **Shared state:** `tokio::sync::RwLock`, `tokio::sync::mpsc` for channels
- **Cancellation:** `tokio_util::sync::CancellationToken`

## Common Patterns

### Builder Pattern

```rust
McpServer::builder()
    .name("my-server")
    .version("1.0.0")
    .instructions("Server description")
    .tool(Tool::new("name", "desc", schema), handler)       // Manual API
    .tool_def(my_tool_def())                                 // Macro-based
    .resource(Resource::new("uri", "name"), handler)
    .build()
    .serve_stdio().await?;
```

### Proc-Macro Handlers (preferred for new code)

```rust
use mcp_kit::prelude::*;

#[tool(name = "greet", description = "Greet a user")]
async fn greet(name: String) -> String {
    format!("Hello, {name}!")
}

#[resource(uri = "file:///{path}", name = "read_file", description = "Read a file")]
async fn read_file(path: String) -> String { /* ... */ }

#[prompt(name = "review", description = "Code review prompt")]
async fn review(code: String) -> String { /* ... */ }

// Register with .tool_def(), .resource_def(), .prompt_def()
```

### Logging Setup

```rust
tracing_subscriber::fmt()
    .with_writer(std::io::stderr)              // CRITICAL for stdio transport
    .with_env_filter("my_server=debug,mcp_kit=info")
    .init();
```

## Testing

- **Integration tests:** `tests/integration_tests.rs` — client-server tests over WebSocket/SSE
- **Unit tests:** Inline `#[cfg(test)] mod tests` in `src/types/elicitation.rs`, `src/server/cancellation.rs`
- **Port convention:** Integration tests use unique ports (19001-19031) to avoid conflicts
- **Test helper:** `create_test_server()` builds a server with greet/echo/add tools
- **All tests are async:** Use `#[tokio::test]`, feature-gated with `#[cfg(feature = "websocket")]` etc.

## Important Notes

1. **MSRV:** 1.85 — don't use features from newer Rust editions
2. **WASM-safe:** Core types (`error`, `protocol`, `types/`) must never depend on tokio/axum
3. **Dependencies:** Minimize new deps; prefer std/core when possible
4. **Breaking changes:** 0.2.x — API can change, follow semver conventions
5. **No Cursor/Copilot rules** — this file is the sole agent reference

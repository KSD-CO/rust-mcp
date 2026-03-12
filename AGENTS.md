# AGENTS.md

Essential information for AI coding agents working in the `rust-mcp` repository.

## Project Overview

**rust-mcp** is a Rust library for building MCP (Model Context Protocol) servers with an ergonomic, type-safe, async-first API.

- **Language:** Rust (Edition 2021, MSRV 1.75)
- **License:** MIT
- **Architecture:** Modular, feature-gated library (core types are WASM-safe)
- **Core deps:** `serde`, `serde_json`, `thiserror`, `tracing`, `schemars`

## Build, Test & Lint Commands

```bash
# Build
cargo build                                    # Basic build
cargo build --all-features                     # Build with all features
cargo build --examples                         # Build examples
cargo build --example calculator               # Build specific example

# Test (no unit tests yet; examples serve as integration tests)
cargo test --workspace --all-features          # Run all tests
cargo test test_name                           # Run a single test
cargo test module_name::                       # Run tests in a module
cargo test -- --nocapture                      # Show test output

# Lint & Format (CI enforced)
cargo fmt --all -- --check                     # Check formatting
cargo fmt --all                                # Auto-format
cargo clippy --workspace --all-features -- -D warnings  # Run Clippy
cargo clippy --fix --workspace --all-features  # Auto-fix warnings

# Run Examples
cargo run --example calculator                 # Stdio transport
cargo run --example everything -- --sse        # SSE transport on :3000
```

## Code Style Guidelines

### Imports
```rust
use std::io;                                   // Std library first
use serde::{Deserialize, Serialize};           // External crates
use crate::error::{McpError, McpResult};       // Internal crate
use rust_mcp::prelude::*;                           // Prefer glob for prelude
```

### Naming Conventions
- **Types/Traits:** `PascalCase` (e.g., `McpServer`, `ToolHandler`)
- **Functions/Variables:** `snake_case` (e.g., `build_server`, `tool_name`)
- **Constants:** `SCREAMING_SNAKE_CASE` (e.g., `MCP_PROTOCOL_VERSION`)
- **Modules:** `snake_case` (e.g., `mod error;`)
- **Features:** `kebab-case` (e.g., `feature = "stdio"`)

### Types & Error Handling
```rust
// Type aliases for Results
pub type McpResult<T> = Result<T, McpError>;

// Comprehensive derives for public types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool { /* ... */ }

// JsonSchema for input types
#[derive(Deserialize, JsonSchema)]
struct ToolInput {
    /// Document all fields
    query: String,
}

// Use thiserror for errors
#[derive(Debug, Error)]
pub enum McpError {
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// Tool handlers can return any Result type
|params: Input| async move -> anyhow::Result<CallToolResult> {
    let data = fetch(&params.url).await?;
    Ok(CallToolResult::text(data))
}
```

### Documentation & Formatting
```rust
//! Module-level docs at top of file with examples

// Use section separators for visual organization
// ─── Section Name ────────────────────────────────────────────────────────

/// Document all public items with rustdoc comments.
/// Include examples for complex APIs.
pub struct McpServer { /* ... */ }
```

- **Formatting:** Default `rustfmt` (4 spaces, 100 char line length)
- **Indentation:** 4 spaces (Rust standard)
- **CI enforcement:** All code must pass `cargo fmt --all -- --check`

### Feature Gates
```rust
// Core types always compiled (no feature gate)
pub mod error;
pub mod protocol;
pub mod types;

// Optional features
#[cfg(feature = "server")]
pub mod server;

#[cfg(any(feature = "stdio", feature = "sse"))]
pub mod transport;
```

### Async Patterns
```rust
// Use #[tokio::main] for async main
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Server setup...
    Ok(())
}

// Handlers are async closures
|params: Input| async move {
    CallToolResult::text(do_work(&params).await)
}
```

## Common Patterns

### Builder Pattern
```rust
McpServer::builder()
    .name("my-server")
    .version("1.0.0")
    .instructions("Server description")
    .tool(Tool::new("name", "desc", schema), handler)
    .resource(Resource::new("uri", "name"), handler)
    .build()
    .serve_stdio().await?;
```

### Handler Definition
```rust
// Typed handler (recommended)
#[derive(Deserialize, JsonSchema)]
struct Input { query: String }

let schema = serde_json::to_value(schemars::schema_for!(Input))?;

.tool(
    Tool::new("search", "Search docs", schema),
    |params: Input| async move {
        CallToolResult::text(format!("Results: {}", params.query))
    },
)

// Raw JSON handler
.tool(
    Tool::new("ping", "Check conn", serde_json::json!({"type": "object"})),
    |_: serde_json::Value| async move { CallToolResult::text("pong") },
)
```

## Logging & Debugging

```rust
// Always log to stderr when using stdio transport
tracing_subscriber::fmt()
    .with_writer(std::io::stderr)
    .with_env_filter("my_server=debug,mcp=info")
    .init();
```

```bash
RUST_LOG=my_server=debug,mcp=debug cargo run
RUST_BACKTRACE=1 cargo run
```

## Important Notes

1. **MSRV:** 1.75 - don't use features from newer Rust versions
2. **WASM-safe:** Core types (`error`, `protocol`, `types`) must remain WASM-safe
3. **Dependencies:** Minimize new deps; prefer std/core when possible
4. **CI:** All PRs must pass fmt, clippy, tests on Ubuntu/macOS/Windows
5. **Breaking changes:** 0.1.x - API can change, follow semver conventions
6. **Documentation:** Update README.md for new features; use rustdoc liberally

## Resources

- [MCP Specification](https://modelcontextprotocol.io/)
- [Project README](README.md) - Comprehensive examples and API docs
- [Examples](examples/) - Calculator and comprehensive examples
- [GitHub Repository](https://github.com/KSD-CO/rust-mcp)

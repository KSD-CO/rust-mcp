//! # mcp — Rust library for building MCP servers
//!
//! A high-quality, type-safe, async-first library for the
//! [Model Context Protocol](https://modelcontextprotocol.io/).
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use mcp::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     McpServer::builder()
//!         .name("my-server")
//!         .version("1.0.0")
//!         .tool(
//!             Tool::new("greet", "Say hello", serde_json::json!({
//!                 "type": "object",
//!                 "properties": { "name": { "type": "string" } },
//!                 "required": ["name"]
//!             })),
//!             |args: serde_json::Value| async move {
//!                 CallToolResult::text(format!("Hello, {}!", args["name"]))
//!             },
//!         )
//!         .build()
//!         .serve_stdio()
//!         .await?;
//!     Ok(())
//! }
//! ```

// ─── Core modules (always compiled, WASM-safe) ────────────────────────────────

pub mod error;
pub mod protocol;
pub mod types;

// ─── Server module (requires `server` feature) ───────────────────────────────

#[cfg(feature = "server")]
pub mod server;

// ─── Transport module (requires `stdio` or `sse` feature) ────────────────────

#[cfg(any(feature = "stdio", feature = "sse"))]
pub mod transport;

// ─── Top-level re-exports ─────────────────────────────────────────────────────

pub use error::{ErrorCode, ErrorData, McpError, McpResult};

pub use protocol::{
    JsonRpcError, JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
    MCP_PROTOCOL_VERSION, JSONRPC_VERSION, ProgressToken, RequestId,
};

pub use types::{
    content::{
        AudioContent, BlobResourceContents, Content, EmbeddedResource, ImageContent,
        ResourceContents, TextContent, TextResourceContents,
    },
    messages::{
        CallToolRequest, GetPromptRequest, GetPromptResult, InitializeRequest, InitializeResult,
        ListPromptsResult, ListResourcesResult, ListToolsResult, ReadResourceRequest,
        ReadResourceResult,
    },
    prompt::{Prompt, PromptArgument, PromptMessage, PromptMessageRole},
    resource::{Resource, ResourceTemplate},
    sampling::{CreateMessageRequest, CreateMessageResult, ModelPreferences, SamplingMessage},
    tool::{CallToolResult, Tool, ToolAnnotations},
    ClientCapabilities, ClientInfo, Implementation, LoggingLevel, ServerCapabilities, ServerInfo,
};

#[cfg(feature = "server")]
pub use server::{
    builder::{McpServerBuilder, ToolDef},
    extract::{Extension, Json, State},
    handler::{IntoToolResult, ToolHandler},
    server::McpServer,
    session::Session,
};

#[cfg(feature = "stdio")]
pub use transport::stdio::{ServeStdioExt, StdioTransport};

#[cfg(feature = "sse")]
pub use transport::sse::{ServeSseExt, SseTransport};

// Re-export proc macros
pub use mcp_macros::{prompt, resource, tool};

// Re-export commonly-needed external crates
pub use serde::{Deserialize, Serialize};
pub use serde_json;
pub use schemars::{self, JsonSchema};

#[cfg(feature = "stdio")]
pub use tokio;

// ─── Prelude ─────────────────────────────────────────────────────────────────

/// Everything you need to build an MCP server — import with `use mcp::prelude::*`.
pub mod prelude {
    pub use crate::{
        CallToolResult, Content, GetPromptResult, ImageContent, McpError, McpResult, Prompt,
        PromptArgument, PromptMessage, PromptMessageRole, ReadResourceResult, Resource,
        ResourceContents, ResourceTemplate, TextContent, Tool, ToolAnnotations,
    };
    pub use crate::{Deserialize, JsonSchema, Serialize, serde_json};
    pub use mcp_macros::tool;

    #[cfg(feature = "server")]
    pub use crate::{Json, McpServer, McpServerBuilder, Session, State, ToolDef};

    #[cfg(feature = "stdio")]
    pub use crate::ServeStdioExt;

    #[cfg(feature = "sse")]
    pub use crate::ServeSseExt;
}

// ─── Private re-exports for proc-macro use ────────────────────────────────────

#[doc(hidden)]
pub mod __private {
    pub use crate::error::{McpError, McpResult};
    pub use crate::types::{messages::CallToolRequest, tool::CallToolResult};
    pub use schemars;
    pub use serde_json;

    #[cfg(feature = "server")]
    pub use crate::server::handler::{BoxFuture, IntoToolResult};
}

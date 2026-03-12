//! # mcp — Rust library for building MCP servers
//!
//! A high-quality, axum-inspired library for the
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
//!         .name("calculator")
//!         .version("1.0.0")
//!         .tool(
//!             Tool::new("add", "Add two numbers", serde_json::json!({
//!                 "type": "object",
//!                 "properties": {
//!                     "a": { "type": "number" },
//!                     "b": { "type": "number" }
//!                 },
//!                 "required": ["a", "b"]
//!             })),
//!             |args: serde_json::Value| async move {
//!                 let a = args["a"].as_f64().unwrap_or(0.0);
//!                 let b = args["b"].as_f64().unwrap_or(0.0);
//!                 CallToolResult::text(format!("{}", a + b))
//!             },
//!         )
//!         .build()
//!         .serve_stdio()
//!         .await?;
//!
//!     Ok(())
//! }
//! ```

// ─── Public re-exports ────────────────────────────────────────────────────────

pub use mcp_core::{
    error::{McpError, McpResult},
    protocol::{JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, MCP_PROTOCOL_VERSION},
    types::{
        // Content types
        content::{
            AudioContent, BlobResourceContents, Content, EmbeddedResource, ImageContent,
            ResourceContents, TextContent, TextResourceContents,
        },
        // Messages
        messages::{
            CallToolRequest, GetPromptRequest, GetPromptResult, InitializeRequest,
            InitializeResult, ListPromptsResult, ListResourcesResult, ListToolsResult,
            ReadResourceRequest, ReadResourceResult,
        },
        // Prompt
        prompt::{Prompt, PromptArgument, PromptMessage, PromptMessageRole},
        // Resource
        resource::{Resource, ResourceTemplate},
        // Sampling
        sampling::{CreateMessageRequest, CreateMessageResult, ModelPreferences, SamplingMessage},
        // Tool
        tool::{CallToolResult, Tool, ToolAnnotations},
        // Core
        ClientCapabilities, ClientInfo, Implementation, LoggingLevel, ServerCapabilities,
        ServerInfo,
    },
};

pub use mcp_server::{
    builder::{McpServerBuilder, ToolDef},
    extract::{Extension, Json, State},
    handler::{IntoToolResult, ToolHandler},
    server::McpServer,
    session::Session,
};

pub use mcp_transport::{ServeSseExt, ServeStdioExt, SseTransport, StdioTransport};

// Re-export proc macros
pub use mcp_macros::{prompt, resource, tool};

// Re-export common external types used in public API
pub use serde::{Deserialize, Serialize};
pub use serde_json;
pub use schemars::{self, JsonSchema};
pub use tokio;

// ─── Prelude ─────────────────────────────────────────────────────────────────

/// Everything you need to build an MCP server — import with `use mcp::prelude::*`.
pub mod prelude {
    pub use crate::{
        CallToolResult, Content, GetPromptResult, ImageContent, Json, McpError, McpResult,
        McpServer, McpServerBuilder, Prompt, PromptArgument, PromptMessage, PromptMessageRole,
        ReadResourceResult, Resource, ResourceContents, ResourceTemplate, ServeSseExt,
        ServeStdioExt, Session, State, TextContent, Tool, ToolAnnotations, ToolDef,
    };
    pub use crate::{Deserialize, JsonSchema, Serialize};
    pub use crate::serde_json;
    pub use mcp_macros::tool;
}

// ─── Private re-exports for proc-macro use ────────────────────────────────────

#[doc(hidden)]
pub mod __private {
    pub use mcp_core::{
        error::{McpError, McpResult},
        types::{
            messages::CallToolRequest,
            tool::CallToolResult,
        },
    };
    pub use mcp_server::handler::{BoxFuture, IntoToolResult};
    pub use schemars;
    pub use serde_json;
}

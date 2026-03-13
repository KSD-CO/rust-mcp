//! # mcp — Rust library for building MCP servers
//!
//! A high-quality, type-safe, async-first library for the
//! [Model Context Protocol](https://modelcontextprotocol.io/).
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use mcp_kit::prelude::*;
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

// ─── Auth module (requires `auth` feature) ───────────────────────────────────

#[cfg(feature = "auth")]
pub mod auth;

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
    ProgressToken, RequestId, JSONRPC_VERSION, MCP_PROTOCOL_VERSION,
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
    builder::{McpServerBuilder, PromptDef, ResourceDef, ToolDef},
    core::McpServer,
    extract::{Extension, Json, State},
    handler::{IntoToolResult, ToolHandler},
    session::Session,
};

#[cfg(all(feature = "server", feature = "auth"))]
pub use server::{Auth, AuthenticatedMarker};

#[cfg(feature = "auth")]
pub use auth::{
    AuthProvider, AuthenticatedIdentity, CompositeAuthProvider, Credentials, DynAuthProvider,
    IntoDynProvider,
};

#[cfg(feature = "auth-bearer")]
pub use auth::BearerTokenProvider;

#[cfg(feature = "auth-apikey")]
pub use auth::ApiKeyProvider;

#[cfg(feature = "auth-basic")]
pub use auth::BasicAuthProvider;

#[cfg(feature = "auth")]
pub use auth::CustomHeaderProvider;

#[cfg(feature = "stdio")]
pub use transport::stdio::{ServeStdioExt, StdioTransport};

#[cfg(feature = "sse")]
pub use transport::sse::{ServeSseExt, SseTransport};

// Re-export proc macros
pub use mcp_kit_macros::{prompt, resource, tool};

// Re-export commonly-needed external crates
pub use schemars::{self, JsonSchema};
pub use serde::{Deserialize, Serialize};
pub use serde_json;

#[cfg(feature = "stdio")]
pub use tokio;

// ─── Prelude ─────────────────────────────────────────────────────────────────

/// Everything you need to build an MCP server — import with `use mcp_kit::prelude::*`.
pub mod prelude {
    pub use crate::{serde_json, Deserialize, JsonSchema, Serialize};
    pub use crate::{
        CallToolResult, Content, GetPromptResult, ImageContent, McpError, McpResult, Prompt,
        PromptArgument, PromptMessage, PromptMessageRole, ReadResourceResult, Resource,
        ResourceContents, ResourceTemplate, TextContent, Tool, ToolAnnotations,
    };
    pub use mcp_kit_macros::tool;

    #[cfg(feature = "server")]
    pub use crate::{
        Json, McpServer, McpServerBuilder, PromptDef, ResourceDef, Session, State, ToolDef,
    };

    #[cfg(all(feature = "server", feature = "auth"))]
    pub use crate::Auth;

    #[cfg(feature = "auth")]
    pub use crate::{AuthProvider, AuthenticatedIdentity, CompositeAuthProvider, Credentials};

    #[cfg(feature = "auth-bearer")]
    pub use crate::BearerTokenProvider;

    #[cfg(feature = "auth-apikey")]
    pub use crate::ApiKeyProvider;

    #[cfg(feature = "auth-basic")]
    pub use crate::BasicAuthProvider;

    #[cfg(feature = "auth")]
    pub use crate::CustomHeaderProvider;

    #[cfg(feature = "stdio")]
    pub use crate::ServeStdioExt;

    #[cfg(feature = "sse")]
    pub use crate::ServeSseExt;
}

// ─── Private re-exports for proc-macro use ────────────────────────────────────

#[doc(hidden)]
pub mod __private {
    pub use crate::error::{McpError, McpResult};
    pub use crate::types::{
        messages::{CallToolRequest, GetPromptRequest, ReadResourceRequest},
        prompt::{GetPromptResult, Prompt, PromptArgument},
        resource::{ReadResourceResult, Resource, ResourceTemplate},
        tool::CallToolResult,
    };
    pub use schemars;
    pub use serde_json;

    #[cfg(feature = "server")]
    pub use crate::server::handler::{BoxFuture, IntoToolResult};

    #[cfg(feature = "server")]
    pub use crate::server::builder::{PromptDef, ResourceDef};

    #[cfg(all(feature = "server", feature = "auth"))]
    pub use crate::server::extract::Auth;
}

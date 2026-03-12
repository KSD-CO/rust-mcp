pub mod codec;
pub mod sse;
pub mod stdio;

pub use sse::{ServeSseExt, SseTransport};
pub use stdio::{ServeStdioExt, StdioTransport};

use mcp_core::error::McpResult;
use mcp_core::protocol::JsonRpcMessage;

/// A transport can send and receive JSON-RPC messages.
#[async_trait::async_trait]
pub trait Transport: Send + Sync + 'static {
    /// Receive the next message (blocks until one arrives)
    async fn recv(&mut self) -> McpResult<Option<JsonRpcMessage>>;
    /// Send a message
    async fn send(&mut self, msg: &JsonRpcMessage) -> McpResult<()>;
    /// Close the transport
    async fn close(self) -> McpResult<()>;
}

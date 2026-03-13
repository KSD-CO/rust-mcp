//! Transport layer abstraction.

use async_trait::async_trait;
use mcp_kit::protocol::{JsonRpcMessage, JsonRpcNotification};

use crate::error::ClientResult;

#[cfg(feature = "stdio")]
pub mod stdio;

#[cfg(feature = "sse")]
pub mod sse;

#[cfg(feature = "websocket")]
pub mod websocket;

#[cfg(feature = "streamable-http")]
pub mod streamable;

/// Transport trait for MCP communication.
///
/// Implementations handle the actual sending and receiving of JSON-RPC messages
/// over different transport mechanisms (stdio, SSE, WebSocket).
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a JSON-RPC message and wait for a response.
    async fn request(&self, message: JsonRpcMessage) -> ClientResult<JsonRpcMessage>;

    /// Send a notification (no response expected).
    async fn notify(&self, notification: JsonRpcNotification) -> ClientResult<()>;

    /// Receive a notification from the server.
    async fn recv_notification(&self) -> ClientResult<JsonRpcNotification>;

    /// Check if the transport is connected.
    fn is_connected(&self) -> bool;

    /// Close the transport connection.
    async fn close(&self) -> ClientResult<()>;
}

/// Boxed transport type for dynamic dispatch.
pub type BoxTransport = Box<dyn Transport>;

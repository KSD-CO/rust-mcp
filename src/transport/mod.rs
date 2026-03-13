#[cfg(feature = "stdio")]
pub(crate) mod codec;

#[cfg(feature = "stdio")]
pub mod stdio;

#[cfg(feature = "sse")]
pub mod sse;

#[cfg(feature = "sse")]
pub mod streamable;

#[cfg(feature = "websocket")]
pub mod websocket;

#[cfg(all(feature = "sse", feature = "auth"))]
pub mod auth_layer;

#[cfg(feature = "auth-mtls")]
pub mod tls;

#[cfg(feature = "stdio")]
pub use stdio::{ServeStdioExt, StdioTransport};

#[cfg(feature = "sse")]
pub use sse::{ServeSseExt, SseTransport};

#[cfg(feature = "sse")]
pub use streamable::{ServeStreamableExt, StreamableTransport};

#[cfg(feature = "websocket")]
pub use websocket::{ServeWebSocketExt, WebSocketTransport};

#[cfg(feature = "auth-mtls")]
pub use tls::{ServeSseTlsExt, TlsConfig};

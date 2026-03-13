#[cfg(feature = "stdio")]
pub(crate) mod codec;

#[cfg(feature = "stdio")]
pub mod stdio;

#[cfg(feature = "sse")]
pub mod sse;

#[cfg(all(feature = "sse", feature = "auth"))]
pub mod auth_layer;

#[cfg(feature = "auth-mtls")]
pub mod tls;

#[cfg(feature = "stdio")]
pub use stdio::{ServeStdioExt, StdioTransport};

#[cfg(feature = "sse")]
pub use sse::{ServeSseExt, SseTransport};

#[cfg(feature = "auth-mtls")]
pub use tls::{ServeSseTlsExt, TlsConfig};

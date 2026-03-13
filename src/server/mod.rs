pub mod builder;
pub mod core;
pub mod extract;
pub mod handler;
pub mod router;
pub mod session;

#[cfg(feature = "auth")]
pub mod auth_context;

pub use builder::{McpServerBuilder, ToolDef};
pub use core::McpServer;
pub use extract::{Extension, Json, State};
pub use handler::{BoxFuture, HandlerFn, IntoToolResult, ToolHandler, TypedMarker};
pub use router::Router;
pub use session::Session;

#[cfg(feature = "auth")]
pub use extract::Auth;

#[cfg(feature = "auth")]
pub use handler::AuthenticatedMarker;

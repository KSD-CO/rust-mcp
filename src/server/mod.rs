pub mod builder;
pub mod cancellation;
pub mod core;
pub mod elicitation;
pub mod extract;
pub mod handler;
pub mod notification;
pub mod progress;
pub mod roots;
pub mod router;
pub mod sampling;
pub mod session;
pub mod subscription;

#[cfg(feature = "auth")]
pub mod auth_context;

pub use builder::{McpServerBuilder, ToolDef};
pub use cancellation::{CancellationManager, RequestGuard};
pub use core::McpServer;
pub use elicitation::{
    ChannelElicitationClient, ElicitationClient, ElicitationClientExt, ElicitationError,
    ElicitationRequestBuilder, ElicitationRequestMessage,
};
pub use extract::{Extension, Json, State};
pub use handler::{
    BoxFuture, CompletionHandler, HandlerFn, IntoToolResult, ToolHandler, TypedMarker,
};
pub use notification::{
    NotificationReceiver, NotificationSender, SendError, SharedNotificationSender,
};
pub use progress::{ProgressTokenExt, ProgressTracker};
pub use roots::RootsManager;
pub use router::Router;
pub use sampling::{
    ChannelSamplingClient, NoOpSamplingClient, SamplingClient, SamplingRequestBuilder,
};
pub use session::Session;
pub use subscription::SubscriptionManager;

#[cfg(feature = "auth")]
pub use extract::Auth;

#[cfg(feature = "auth")]
pub use handler::AuthenticatedMarker;

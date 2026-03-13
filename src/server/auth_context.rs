//! Task-local auth identity context.
//!
//! Before dispatching a `tools/call`, `resources/read`, or `prompts/get`
//! request, `core.rs` calls [`scope`] to set the current identity for that
//! async task. Handler code retrieves it via [`current`] (or through the
//! [`Auth`] extractor, which calls [`current`] internally).
//!
//! Using a task-local means:
//! - `ToolHandlerFn` signatures stay **unchanged** — no breaking API change.
//! - Concurrently running handlers in different tasks each see their own identity.
//! - The overhead is a single `tokio::task_local!` lookup per extractor use.
//!
//! [`Auth`]: crate::server::extract::Auth

use std::future::Future;

use crate::auth::AuthenticatedIdentity;

tokio::task_local! {
    /// The identity of the authenticated caller for the current MCP request.
    /// `None` means the request was unauthenticated (or auth is not configured).
    static CURRENT_AUTH: Option<AuthenticatedIdentity>;
}

/// Run `fut` with `identity` set as the current auth context for this task.
///
/// Called by `core.rs` before dispatching each handler invocation.
pub fn scope<F: Future>(
    identity: Option<AuthenticatedIdentity>,
    fut: F,
) -> impl Future<Output = F::Output> {
    CURRENT_AUTH.scope(identity, fut)
}

/// Retrieve the current identity, or `None` if running outside an auth scope.
pub fn current() -> Option<AuthenticatedIdentity> {
    CURRENT_AUTH.try_with(|id| id.clone()).ok().flatten()
}

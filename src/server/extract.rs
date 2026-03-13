/// Extractors for tool/resource/prompt handlers — inspired by axum's extractor pattern.
use crate::error::{McpError, McpResult};
use serde::de::DeserializeOwned;

// ─── Json<T> extractor ────────────────────────────────────────────────────────

/// Deserialize the full arguments object as type `T`.
pub struct Json<T>(pub T);

impl<T: DeserializeOwned> Json<T> {
    pub fn from_value(v: serde_json::Value) -> McpResult<Self> {
        serde_json::from_value(v)
            .map(Json)
            .map_err(|e| McpError::InvalidParams(e.to_string()))
    }
}

impl<T> std::ops::Deref for Json<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// ─── State<S> extractor ───────────────────────────────────────────────────────

/// Share arbitrary state across handlers.
#[derive(Clone)]
pub struct State<S>(pub S);

impl<S> std::ops::Deref for State<S> {
    type Target = S;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// ─── Extension<T> extractor ───────────────────────────────────────────────────

/// Type-map extension data attached to a session.
#[derive(Clone)]
pub struct Extension<T>(pub T);

impl<T> std::ops::Deref for Extension<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// ─── Auth extractor ───────────────────────────────────────────────────────────

/// Extracts the [`AuthenticatedIdentity`] for the current request.
///
/// Available inside `#[tool]`, `#[resource]`, and `#[prompt]` handlers when
/// the `auth` feature is enabled and the server is configured with an auth
/// provider.
///
/// Returns [`McpError::Unauthorized`] if the request is unauthenticated.
///
/// # Example
///
/// ```rust,no_run
/// use mcp_kit::prelude::*;
/// use mcp_kit::Auth;
///
/// #[mcp_kit::tool(description = "A protected tool")]
/// async fn my_tool(input: String, auth: Auth) -> McpResult<CallToolResult> {
///     if !auth.has_scope("tools:execute") {
///         return Err(McpError::Unauthorized("missing scope".into()));
///     }
///     Ok(CallToolResult::text(format!("Hello, {}!", auth.subject)))
/// }
/// ```
///
/// [`AuthenticatedIdentity`]: crate::auth::AuthenticatedIdentity
/// [`McpError::Unauthorized`]: crate::error::McpError::Unauthorized
#[cfg(feature = "auth")]
pub struct Auth(pub crate::auth::AuthenticatedIdentity);

#[cfg(feature = "auth")]
impl Auth {
    /// Pull the current identity from the task-local auth context.
    /// Returns `Err(McpError::Unauthorized)` if no identity is present.
    pub fn from_context() -> McpResult<Self> {
        crate::server::auth_context::current()
            .map(Auth)
            .ok_or_else(|| McpError::Unauthorized("unauthenticated request".into()))
    }
}

#[cfg(feature = "auth")]
impl std::ops::Deref for Auth {
    type Target = crate::auth::AuthenticatedIdentity;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

use std::{future::Future, pin::Pin, sync::Arc};

use crate::{
    auth::{credentials::Credentials, identity::AuthenticatedIdentity},
    error::McpResult,
};

/// A pinned, boxed future returned by [`AuthProvider::authenticate`].
pub type AuthFuture<'a> =
    Pin<Box<dyn Future<Output = McpResult<AuthenticatedIdentity>> + Send + 'a>>;

/// Validates raw [`Credentials`] and produces an [`AuthenticatedIdentity`], or
/// returns [`McpError::Unauthorized`] on failure.
///
/// The trait is object-safe so providers can be stored as `Arc<dyn AuthProvider>`
/// (see [`DynAuthProvider`]).
///
/// # Implementing your own provider
///
/// ```rust,no_run
/// use mcp_kit::auth::{AuthProvider, AuthFuture, Credentials, AuthenticatedIdentity};
/// use mcp_kit::McpResult;
///
/// struct MyProvider;
///
/// impl AuthProvider for MyProvider {
///     fn authenticate<'a>(&'a self, creds: &'a Credentials) -> AuthFuture<'a> {
///         Box::pin(async move {
///             match creds {
///                 Credentials::Bearer { token } if token == "secret" => {
///                     Ok(AuthenticatedIdentity::new("user"))
///                 }
///                 _ => Err(mcp_kit::McpError::Unauthorized("invalid token".into())),
///             }
///         })
///     }
///
///     fn accepts(&self, creds: &Credentials) -> bool {
///         matches!(creds, Credentials::Bearer { .. })
///     }
/// }
/// ```
///
/// [`McpError::Unauthorized`]: crate::error::McpError::Unauthorized
pub trait AuthProvider: Send + Sync + 'static {
    /// Validate `credentials` and return the authenticated identity, or an error.
    fn authenticate<'a>(&'a self, credentials: &'a Credentials) -> AuthFuture<'a>;

    /// Returns `true` if this provider knows how to handle the given credential
    /// variant. Used by [`CompositeAuthProvider`] to select the right delegate
    /// without unnecessarily calling `authenticate`.
    ///
    /// [`CompositeAuthProvider`]: crate::auth::composite::CompositeAuthProvider
    fn accepts(&self, credentials: &Credentials) -> bool;
}

/// A type-erased, cheaply cloneable auth provider.
pub type DynAuthProvider = Arc<dyn AuthProvider>;

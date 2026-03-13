use std::{future::Future, pin::Pin, sync::Arc};

use crate::{
    auth::{
        credentials::Credentials,
        identity::AuthenticatedIdentity,
        provider::{AuthFuture, AuthProvider},
    },
    error::{McpError, McpResult},
};

/// A custom async validator for (username, password) pairs.
pub type BasicValidatorFn = Arc<
    dyn Fn(String, String) -> Pin<Box<dyn Future<Output = McpResult<AuthenticatedIdentity>> + Send>>
        + Send
        + Sync,
>;

/// Validates `Authorization: Basic <base64(username:password)>` credentials.
///
/// The base64 decoding is handled by the transport layer before the credentials
/// reach this provider; this provider receives the already-decoded username and
/// password via [`Credentials::Basic`].
///
/// # Examples
///
/// ```rust,no_run
/// use mcp_kit::auth::{BasicAuthProvider, AuthenticatedIdentity};
///
/// let provider = BasicAuthProvider::with_validator(|username, password| async move {
///     if username == "admin" && password == "secret" {
///         Ok(AuthenticatedIdentity::new(username).with_scopes(["admin"]))
///     } else {
///         Err(mcp_kit::McpError::Unauthorized("invalid credentials".into()))
///     }
/// });
/// ```
pub struct BasicAuthProvider {
    validator: BasicValidatorFn,
}

impl BasicAuthProvider {
    /// Create a provider with a custom async validator.
    pub fn with_validator<F, Fut>(f: F) -> Self
    where
        F: Fn(String, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = McpResult<AuthenticatedIdentity>> + Send + 'static,
    {
        Self {
            validator: Arc::new(move |u, p| Box::pin(f(u, p))),
        }
    }
}

impl AuthProvider for BasicAuthProvider {
    fn authenticate<'a>(&'a self, credentials: &'a Credentials) -> AuthFuture<'a> {
        Box::pin(async move {
            match credentials {
                Credentials::Basic { username, password } => {
                    (self.validator)(username.clone(), password.clone()).await
                }
                _ => Err(McpError::Unauthorized(
                    "expected basic auth credentials".into(),
                )),
            }
        })
    }

    fn accepts(&self, credentials: &Credentials) -> bool {
        matches!(credentials, Credentials::Basic { .. })
    }
}

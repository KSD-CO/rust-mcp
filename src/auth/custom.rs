use std::{future::Future, pin::Pin, sync::Arc};

use crate::{
    auth::{
        credentials::Credentials,
        identity::AuthenticatedIdentity,
        provider::{AuthFuture, AuthProvider},
    },
    error::{McpError, McpResult},
};

/// A custom async validator for arbitrary header values.
pub type CustomValidatorFn = Arc<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = McpResult<AuthenticatedIdentity>> + Send>>
        + Send
        + Sync,
>;

/// Validates a custom HTTP header credential (e.g. `X-Internal-Token`).
///
/// The transport layer extracts the value of the configured header and wraps it
/// in [`Credentials::CustomHeader`] before passing it to this provider.
///
/// # Examples
///
/// ```rust,no_run
/// use mcp_kit::auth::{CustomHeaderProvider, AuthenticatedIdentity};
///
/// let provider = CustomHeaderProvider::new("x-internal-token", |value| async move {
///     if value == "trusted" {
///         Ok(AuthenticatedIdentity::new("internal-service"))
///     } else {
///         Err(mcp_kit::McpError::Unauthorized("invalid token".into()))
///     }
/// });
/// ```
pub struct CustomHeaderProvider {
    header_name: String,
    validator: CustomValidatorFn,
}

impl CustomHeaderProvider {
    /// Create a provider that validates values from the named header.
    ///
    /// `header_name` is case-insensitive; it will be normalised to lowercase.
    pub fn new<F, Fut>(header_name: impl Into<String>, f: F) -> Self
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = McpResult<AuthenticatedIdentity>> + Send + 'static,
    {
        Self {
            header_name: header_name.into().to_lowercase(),
            validator: Arc::new(move |v| Box::pin(f(v))),
        }
    }

    /// The header name this provider expects (normalised to lowercase).
    pub fn header_name(&self) -> &str {
        &self.header_name
    }
}

impl AuthProvider for CustomHeaderProvider {
    fn authenticate<'a>(&'a self, credentials: &'a Credentials) -> AuthFuture<'a> {
        Box::pin(async move {
            match credentials {
                Credentials::CustomHeader { header_name, value }
                    if header_name == &self.header_name =>
                {
                    (self.validator)(value.clone()).await
                }
                Credentials::CustomHeader { header_name, .. } => Err(McpError::Unauthorized(
                    format!("unexpected header: {header_name}"),
                )),
                _ => Err(McpError::Unauthorized(format!(
                    "expected custom header: {}",
                    self.header_name
                ))),
            }
        })
    }

    fn accepts(&self, credentials: &Credentials) -> bool {
        match credentials {
            Credentials::CustomHeader { header_name, .. } => header_name == &self.header_name,
            _ => false,
        }
    }
}

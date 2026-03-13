use std::{collections::HashSet, future::Future, pin::Pin, sync::Arc};

use crate::{
    auth::{
        credentials::Credentials,
        identity::AuthenticatedIdentity,
        provider::{AuthFuture, AuthProvider},
    },
    error::{McpError, McpResult},
};

/// A custom async validator function for bearer tokens.
pub type BearerValidatorFn = Arc<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = McpResult<AuthenticatedIdentity>> + Send>>
        + Send
        + Sync,
>;

enum Inner {
    /// Static allow-list: any token in the set is accepted; identity subject = token.
    Static(HashSet<String>),
    /// Custom async validator.
    Custom(BearerValidatorFn),
}

/// Validates `Authorization: Bearer <token>` credentials.
///
/// # Examples
///
/// Static token list:
/// ```rust,no_run
/// use mcp_kit::auth::BearerTokenProvider;
///
/// let provider = BearerTokenProvider::new(["my-secret-token", "other-token"]);
/// ```
///
/// Custom async validator:
/// ```rust,no_run
/// use mcp_kit::auth::{BearerTokenProvider, AuthenticatedIdentity};
///
/// let provider = BearerTokenProvider::with_validator(|token| async move {
///     if token == "valid" {
///         Ok(AuthenticatedIdentity::new("alice").with_scopes(["tools:execute"]))
///     } else {
///         Err(mcp_kit::McpError::Unauthorized("bad token".into()))
///     }
/// });
/// ```
pub struct BearerTokenProvider {
    inner: Inner,
}

impl BearerTokenProvider {
    /// Create a provider that accepts any token in the given iterator.
    /// The identity subject is set to the token value itself.
    pub fn new(tokens: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            inner: Inner::Static(tokens.into_iter().map(Into::into).collect()),
        }
    }

    /// Create a provider with a custom async validator function.
    pub fn with_validator<F, Fut>(f: F) -> Self
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = McpResult<AuthenticatedIdentity>> + Send + 'static,
    {
        Self {
            inner: Inner::Custom(Arc::new(move |token| Box::pin(f(token)))),
        }
    }
}

impl AuthProvider for BearerTokenProvider {
    fn authenticate<'a>(&'a self, credentials: &'a Credentials) -> AuthFuture<'a> {
        Box::pin(async move {
            match credentials {
                Credentials::Bearer { token } => match &self.inner {
                    Inner::Static(set) => {
                        if set.contains(token.as_str()) {
                            Ok(AuthenticatedIdentity::new(token.clone()))
                        } else {
                            Err(McpError::Unauthorized("invalid bearer token".into()))
                        }
                    }
                    Inner::Custom(f) => f(token.clone()).await,
                },
                _ => Err(McpError::Unauthorized("expected bearer token".into())),
            }
        })
    }

    fn accepts(&self, credentials: &Credentials) -> bool {
        matches!(credentials, Credentials::Bearer { .. })
    }
}

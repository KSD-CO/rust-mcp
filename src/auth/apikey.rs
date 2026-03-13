use std::{collections::HashSet, future::Future, pin::Pin, sync::Arc};

use crate::{
    auth::{
        credentials::Credentials,
        identity::AuthenticatedIdentity,
        provider::{AuthFuture, AuthProvider},
    },
    error::{McpError, McpResult},
};

/// A custom async validator function for API keys.
pub type ApiKeyValidatorFn = Arc<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = McpResult<AuthenticatedIdentity>> + Send>>
        + Send
        + Sync,
>;

enum Inner {
    Static(HashSet<String>),
    Custom(ApiKeyValidatorFn),
}

/// Validates `X-Api-Key: <key>` header credentials (or `?api_key=` query param,
/// which the transport layer normalises into the same [`Credentials::ApiKey`] variant).
///
/// # Examples
///
/// Static key list:
/// ```rust,no_run
/// use mcp_kit::auth::ApiKeyProvider;
///
/// let provider = ApiKeyProvider::new(["key-abc-123", "key-def-456"]);
/// ```
///
/// Custom async validator:
/// ```rust,no_run
/// use mcp_kit::auth::{ApiKeyProvider, AuthenticatedIdentity};
///
/// let provider = ApiKeyProvider::with_validator(|key| async move {
///     // look up the key in a database, etc.
///     Ok(AuthenticatedIdentity::new("service-a").with_scopes(["read"]))
/// });
/// ```
pub struct ApiKeyProvider {
    inner: Inner,
}

impl ApiKeyProvider {
    /// Accept any API key in the given iterator.
    /// The identity subject is set to the key value itself.
    pub fn new(keys: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            inner: Inner::Static(keys.into_iter().map(Into::into).collect()),
        }
    }

    /// Use a custom async validator.
    pub fn with_validator<F, Fut>(f: F) -> Self
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = McpResult<AuthenticatedIdentity>> + Send + 'static,
    {
        Self {
            inner: Inner::Custom(Arc::new(move |key| Box::pin(f(key)))),
        }
    }
}

impl AuthProvider for ApiKeyProvider {
    fn authenticate<'a>(&'a self, credentials: &'a Credentials) -> AuthFuture<'a> {
        Box::pin(async move {
            match credentials {
                Credentials::ApiKey { key } => match &self.inner {
                    Inner::Static(set) => {
                        if set.contains(key.as_str()) {
                            Ok(AuthenticatedIdentity::new(key.clone()))
                        } else {
                            Err(McpError::Unauthorized("invalid API key".into()))
                        }
                    }
                    Inner::Custom(f) => f(key.clone()).await,
                },
                _ => Err(McpError::Unauthorized("expected API key".into())),
            }
        })
    }

    fn accepts(&self, credentials: &Credentials) -> bool {
        matches!(credentials, Credentials::ApiKey { .. })
    }
}

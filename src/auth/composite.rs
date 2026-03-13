use crate::{
    auth::{
        credentials::Credentials,
        provider::{AuthFuture, AuthProvider, DynAuthProvider},
    },
    error::McpError,
};

/// Tries a list of [`AuthProvider`]s in order, returning the first successful result.
///
/// Providers are tested with [`AuthProvider::accepts`] first; if no provider
/// accepts the credential type, `Unauthorized` is returned immediately.
/// If a matching provider returns `Unauthorized`, the next matching provider is tried.
///
/// # Examples
///
/// ```rust,no_run
/// use mcp_kit::auth::{BearerTokenProvider, ApiKeyProvider, CompositeAuthProvider, IntoDynProvider};
///
/// let provider = CompositeAuthProvider::new(vec![
///     BearerTokenProvider::new(["token-abc"]).into_dyn(),
///     ApiKeyProvider::new(["key-xyz"]).into_dyn(),
/// ]);
/// ```
pub struct CompositeAuthProvider {
    providers: Vec<DynAuthProvider>,
}

impl CompositeAuthProvider {
    /// Create a composite from an ordered list of type-erased providers.
    pub fn new(providers: Vec<DynAuthProvider>) -> Self {
        Self { providers }
    }
}

impl AuthProvider for CompositeAuthProvider {
    fn authenticate<'a>(&'a self, credentials: &'a Credentials) -> AuthFuture<'a> {
        Box::pin(async move {
            let mut last_err: Option<McpError> = None;

            for provider in &self.providers {
                if !provider.accepts(credentials) {
                    continue;
                }
                match provider.authenticate(credentials).await {
                    Ok(identity) => return Ok(identity),
                    Err(e) => last_err = Some(e),
                }
            }

            Err(last_err.unwrap_or_else(|| {
                McpError::Unauthorized(format!(
                    "no provider accepts '{}' credentials",
                    credentials.kind()
                ))
            }))
        })
    }

    fn accepts(&self, credentials: &Credentials) -> bool {
        self.providers.iter().any(|p| p.accepts(credentials))
    }
}

/// Extension trait that makes it ergonomic to convert a concrete provider into a
/// `DynAuthProvider` for use with [`CompositeAuthProvider`].
pub trait IntoDynProvider: AuthProvider + Sized {
    fn into_dyn(self) -> DynAuthProvider {
        std::sync::Arc::new(self)
    }
}

impl<T: AuthProvider> IntoDynProvider for T {}

use std::collections::HashMap;

/// The result of a successful authentication.
///
/// An `AuthenticatedIdentity` is stored in the [`Session`] after the transport
/// layer validates the incoming credentials. Handlers that declare an [`Auth`]
/// extractor receive a clone of this value.
///
/// [`Session`]: crate::server::session::Session
/// [`Auth`]: crate::server::extract::Auth
#[derive(Debug, Clone)]
pub struct AuthenticatedIdentity {
    /// Stable subject identifier — e.g. a username, client ID, or JWT `sub` claim.
    pub subject: String,

    /// Scopes / roles granted to this identity (e.g. `["tools:execute", "admin"]`).
    pub scopes: Vec<String>,

    /// Arbitrary key-value metadata attached during authentication.
    /// Examples: OAuth2 claims, certificate CN, tenant ID.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl AuthenticatedIdentity {
    /// Create a new identity with the given subject and no scopes or metadata.
    pub fn new(subject: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            scopes: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Builder: attach scopes to this identity.
    pub fn with_scopes(mut self, scopes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.scopes = scopes.into_iter().map(Into::into).collect();
        self
    }

    /// Builder: attach a single metadata key-value pair.
    pub fn with_meta(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Returns `true` if this identity has the given scope.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// Returns `true` if this identity has **all** of the given scopes.
    pub fn has_all_scopes(&self, scopes: &[&str]) -> bool {
        scopes.iter().all(|s| self.has_scope(s))
    }
}

//! API Key authentication.

use super::{get_header, get_query_param, AuthResult, Authenticator, Identity};
use std::collections::HashMap;
use worker::Request;

/// API Key authentication provider.
///
/// Supports API keys in:
/// - Header (default: `X-API-Key`)
/// - Query parameter (default: `api_key`)
pub struct ApiKeyAuth {
    /// Valid API keys mapped to identities.
    keys: HashMap<String, Identity>,
    /// Header name to check.
    header_name: String,
    /// Query parameter name to check.
    query_param: Option<String>,
}

impl ApiKeyAuth {
    /// Create a new API key authenticator.
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
            header_name: "X-API-Key".to_string(),
            query_param: Some("api_key".to_string()),
        }
    }

    /// Set the header name to check for API key.
    pub fn header_name(mut self, name: impl Into<String>) -> Self {
        self.header_name = name.into();
        self
    }

    /// Set the query parameter name to check for API key.
    pub fn query_param(mut self, name: impl Into<String>) -> Self {
        self.query_param = Some(name.into());
        self
    }

    /// Disable query parameter authentication.
    pub fn no_query_param(mut self) -> Self {
        self.query_param = None;
        self
    }

    /// Add a valid API key.
    pub fn add_key(mut self, key: impl Into<String>, identity: Identity) -> Self {
        self.keys.insert(key.into(), identity);
        self
    }

    /// Add a simple API key (identity = key name).
    pub fn add_simple_key(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        self.keys.insert(key.clone(), Identity::new(key));
        self
    }

    /// Extract API key from request.
    fn extract_key(&self, req: &Request) -> Option<String> {
        // Try header first
        if let Some(key) = get_header(req, &self.header_name) {
            return Some(key);
        }

        // Try query parameter
        if let Some(ref param) = self.query_param {
            if let Some(key) = get_query_param(req, param) {
                return Some(key);
            }
        }

        None
    }
}

impl Default for ApiKeyAuth {
    fn default() -> Self {
        Self::new()
    }
}

impl Authenticator for ApiKeyAuth {
    fn authenticate(&self, req: &Request) -> AuthResult {
        match self.extract_key(req) {
            Some(key) => {
                if let Some(identity) = self.keys.get(&key) {
                    AuthResult::Authenticated(identity.clone())
                } else {
                    AuthResult::Denied("Invalid API key".to_string())
                }
            }
            None => AuthResult::NoCredentials,
        }
    }
}

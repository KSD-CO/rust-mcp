//! Bearer token authentication.

use super::{get_header, AuthResult, Authenticator, Identity};
use std::collections::HashMap;
use worker::Request;

/// Bearer token authentication provider.
///
/// Validates tokens from the `Authorization: Bearer <token>` header.
pub struct BearerAuth {
    /// Valid tokens mapped to identities.
    tokens: HashMap<String, Identity>,
}

impl BearerAuth {
    /// Create a new bearer token authenticator.
    pub fn new() -> Self {
        Self {
            tokens: HashMap::new(),
        }
    }

    /// Add a valid token.
    pub fn add_token(mut self, token: impl Into<String>, identity: Identity) -> Self {
        self.tokens.insert(token.into(), identity);
        self
    }

    /// Add a simple token (identity = token prefix).
    pub fn add_simple_token(mut self, token: impl Into<String>) -> Self {
        let token = token.into();
        let id = if token.len() > 8 {
            format!("{}...", &token[..8])
        } else {
            token.clone()
        };
        self.tokens.insert(token, Identity::new(id));
        self
    }

    /// Extract bearer token from Authorization header.
    fn extract_token(&self, req: &Request) -> Option<String> {
        get_header(req, "Authorization").and_then(|auth| {
            let auth = auth.trim();
            if auth.to_lowercase().starts_with("bearer ") {
                Some(auth[7..].trim().to_string())
            } else {
                None
            }
        })
    }
}

impl Default for BearerAuth {
    fn default() -> Self {
        Self::new()
    }
}

impl Authenticator for BearerAuth {
    fn authenticate(&self, req: &Request) -> AuthResult {
        match self.extract_token(req) {
            Some(token) => {
                if let Some(identity) = self.tokens.get(&token) {
                    AuthResult::Authenticated(identity.clone())
                } else {
                    AuthResult::Denied("Invalid bearer token".to_string())
                }
            }
            None => AuthResult::NoCredentials,
        }
    }
}

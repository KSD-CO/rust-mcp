//! Basic authentication (username:password).

use super::{get_header, AuthResult, Authenticator, Identity};
use std::collections::HashMap;
use worker::Request;

/// Basic authentication provider.
///
/// Validates credentials from the `Authorization: Basic <base64>` header.
pub struct BasicAuth {
    /// Valid username:password pairs mapped to identities.
    credentials: HashMap<(String, String), Identity>,
}

impl BasicAuth {
    /// Create a new basic auth provider.
    pub fn new() -> Self {
        Self {
            credentials: HashMap::new(),
        }
    }

    /// Add valid credentials.
    pub fn add_user(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
        identity: Identity,
    ) -> Self {
        self.credentials
            .insert((username.into(), password.into()), identity);
        self
    }

    /// Add simple credentials (identity = username).
    pub fn add_simple_user(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        let username = username.into();
        let identity = Identity::new(&username).with_name(&username);
        self.credentials
            .insert((username, password.into()), identity);
        self
    }

    /// Extract and decode Basic auth credentials.
    fn extract_credentials(&self, req: &Request) -> Option<(String, String)> {
        get_header(req, "Authorization").and_then(|auth| {
            let auth = auth.trim();
            if !auth.to_lowercase().starts_with("basic ") {
                return None;
            }

            let encoded = auth[6..].trim();
            
            // Decode base64
            use base64::{engine::general_purpose::STANDARD, Engine};
            let decoded = STANDARD.decode(encoded).ok()?;
            let decoded_str = String::from_utf8(decoded).ok()?;

            // Split username:password
            let mut parts = decoded_str.splitn(2, ':');
            let username = parts.next()?.to_string();
            let password = parts.next()?.to_string();

            Some((username, password))
        })
    }
}

impl Default for BasicAuth {
    fn default() -> Self {
        Self::new()
    }
}

impl Authenticator for BasicAuth {
    fn authenticate(&self, req: &Request) -> AuthResult {
        match self.extract_credentials(req) {
            Some((username, password)) => {
                if let Some(identity) = self.credentials.get(&(username.clone(), password)) {
                    AuthResult::Authenticated(identity.clone())
                } else {
                    AuthResult::Denied(format!("Invalid credentials for user: {}", username))
                }
            }
            None => AuthResult::NoCredentials,
        }
    }
}

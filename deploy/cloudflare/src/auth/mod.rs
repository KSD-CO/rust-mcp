//! Authentication module for Cloudflare Workers MCP server.
//!
//! Supports multiple authentication methods:
//! - API Key (header or query parameter)
//! - Bearer Token
//! - Basic Authentication
//!
//! For production, store secrets in Cloudflare Workers Secrets or KV.

use worker::Request;

mod apikey;
mod basic;
mod bearer;

pub use apikey::ApiKeyAuth;
pub use basic::BasicAuth;
pub use bearer::BearerAuth;

// ─── AuthResult ───────────────────────────────────────────────────────────────

/// Result of an authentication attempt.
#[derive(Debug, Clone)]
pub enum AuthResult {
    /// Authentication successful with identity info.
    Authenticated(Identity),
    /// Authentication failed with reason.
    Denied(String),
    /// No credentials provided (for optional auth).
    NoCredentials,
}

impl AuthResult {
    pub fn is_authenticated(&self) -> bool {
        matches!(self, AuthResult::Authenticated(_))
    }

    pub fn identity(&self) -> Option<&Identity> {
        match self {
            AuthResult::Authenticated(id) => Some(id),
            _ => None,
        }
    }
}

// ─── Identity ─────────────────────────────────────────────────────────────────

/// Authenticated user identity.
#[derive(Debug, Clone)]
pub struct Identity {
    /// Unique identifier (user ID, API key name, etc.)
    pub id: String,
    /// Display name (optional)
    pub name: Option<String>,
    /// Roles/permissions (optional)
    pub roles: Vec<String>,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl Identity {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            roles: Vec::new(),
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.roles.push(role.into());
        self
    }

    pub fn with_roles(mut self, roles: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.roles.extend(roles.into_iter().map(Into::into));
        self
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}

// ─── Authenticator Trait ──────────────────────────────────────────────────────

/// Trait for authentication providers.
pub trait Authenticator: Send + Sync {
    /// Authenticate a request.
    fn authenticate(&self, req: &Request) -> AuthResult;
}

// ─── CompositeAuth ────────────────────────────────────────────────────────────

/// Combines multiple authenticators, trying each in order.
pub struct CompositeAuth {
    authenticators: Vec<Box<dyn Authenticator>>,
}

impl CompositeAuth {
    pub fn new() -> Self {
        Self {
            authenticators: Vec::new(),
        }
    }

    pub fn add<A: Authenticator + 'static>(mut self, auth: A) -> Self {
        self.authenticators.push(Box::new(auth));
        self
    }

    /// Authenticate using all providers, returning first success.
    pub fn authenticate(&self, req: &Request) -> AuthResult {
        for auth in &self.authenticators {
            match auth.authenticate(req) {
                AuthResult::Authenticated(id) => return AuthResult::Authenticated(id),
                AuthResult::Denied(reason) => return AuthResult::Denied(reason),
                AuthResult::NoCredentials => continue,
            }
        }
        AuthResult::NoCredentials
    }
}

impl Default for CompositeAuth {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Helper Functions ─────────────────────────────────────────────────────────

/// Extract a header value from a request.
pub fn get_header(req: &Request, name: &str) -> Option<String> {
    req.headers().get(name).ok().flatten()
}

/// Extract a query parameter from a request URL.
pub fn get_query_param(req: &Request, name: &str) -> Option<String> {
    req.url()
        .ok()
        .and_then(|url| {
            url.query_pairs()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.to_string())
        })
}

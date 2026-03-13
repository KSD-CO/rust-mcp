//! Authentication support for MCP servers.
//!
//! This module provides the core auth abstractions used across all transports.
//! Each auth scheme is gated behind a feature flag so you only compile what you use:
//!
//! | Feature | Provider | Credential type |
//! |---------|----------|-----------------|
//! | `auth-bearer` | [`BearerTokenProvider`] | `Authorization: Bearer <token>` |
//! | `auth-apikey` | [`ApiKeyProvider`] | `X-Api-Key: <key>` or `?api_key=` |
//! | `auth-basic`  | [`BasicAuthProvider`]  | `Authorization: Basic <b64>` |
//! | `auth-oauth2` | [`OAuth2Provider`] | Bearer token (introspection or JWT/JWKS) |
//! | `auth-mtls`   | [`MtlsProvider`] | Client certificate (mTLS) |
//! | *(always)*    | [`CustomHeaderProvider`] | Any custom header |
//! | *(always)*    | [`CompositeAuthProvider`] | Try multiple providers in order |
//!
//! # Quick start
//!
//! ```rust,no_run
//! use mcp_kit::auth::{BearerTokenProvider, CompositeAuthProvider};
//! use mcp_kit::auth::composite::IntoDynProvider;
//!
//! let provider = CompositeAuthProvider::new(vec![
//!     BearerTokenProvider::new(["my-secret"]).into_dyn(),
//! ]);
//! ```

pub mod composite;
pub mod credentials;
pub mod custom;
pub mod identity;
pub mod provider;

#[cfg(feature = "auth-bearer")]
pub mod bearer;

#[cfg(feature = "auth-apikey")]
pub mod apikey;

#[cfg(feature = "auth-basic")]
pub mod basic;

#[cfg(feature = "auth-oauth2")]
pub mod oauth2;

#[cfg(feature = "auth-mtls")]
pub mod mtls;

// ─── Public re-exports ────────────────────────────────────────────────────────

pub use composite::{CompositeAuthProvider, IntoDynProvider};
pub use credentials::Credentials;
pub use custom::CustomHeaderProvider;
pub use identity::AuthenticatedIdentity;
pub use provider::{AuthFuture, AuthProvider, DynAuthProvider};

#[cfg(feature = "auth-bearer")]
pub use bearer::BearerTokenProvider;

#[cfg(feature = "auth-apikey")]
pub use apikey::ApiKeyProvider;

#[cfg(feature = "auth-basic")]
pub use basic::BasicAuthProvider;

#[cfg(feature = "auth-oauth2")]
pub use oauth2::{OAuth2Config, OAuth2Provider};

#[cfg(feature = "auth-mtls")]
pub use mtls::MtlsProvider;

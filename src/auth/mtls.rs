//! Mutual TLS (mTLS) authentication provider.
//!
//! Validates client certificates presented during the TLS handshake.
//! The raw DER bytes of the peer certificate are wrapped in
//! [`Credentials::ClientCertificate`] and forwarded to the configured
//! validator closure.
//!
//! # Example
//! ```rust,no_run
//! use mcp_kit::auth::mtls::MtlsProvider;
//! use std::sync::Arc;
//!
//! // Accept any certificate whose subject contains "allowed-client"
//! let provider = Arc::new(MtlsProvider::new(|der: &[u8]| {
//!     // Parse with your preferred X.509 library and return subject/SANs.
//!     Ok(mcp_kit::auth::AuthenticatedIdentity::new("client"))
//! }));
//! ```

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::{
    auth::{AuthProvider, AuthenticatedIdentity, Credentials},
    error::{McpError, McpResult},
};

/// Async validator signature: takes DER-encoded certificate bytes, returns
/// an identity or an error.
pub type CertValidatorFn = Arc<
    dyn Fn(&[u8]) -> Pin<Box<dyn Future<Output = McpResult<AuthenticatedIdentity>> + Send>>
        + Send
        + Sync
        + 'static,
>;

/// mTLS authentication provider.
///
/// Accepts [`Credentials::ClientCertificate`] credentials produced by the
/// TLS transport after a successful handshake.
pub struct MtlsProvider {
    validator: CertValidatorFn,
}

impl MtlsProvider {
    /// Create a provider with a **synchronous** certificate validator.
    ///
    /// The closure receives DER-encoded certificate bytes and must return an
    /// `AuthenticatedIdentity` on success or a `McpError` on rejection.
    pub fn new<F>(validator: F) -> Self
    where
        F: Fn(&[u8]) -> McpResult<AuthenticatedIdentity> + Send + Sync + 'static,
    {
        let validator = Arc::new(validator);
        let async_validator: CertValidatorFn = Arc::new(move |der: &[u8]| {
            let v = validator.clone();
            let der = der.to_vec();
            Box::pin(async move { v(&der) })
        });
        Self {
            validator: async_validator,
        }
    }

    /// Create a provider with an **async** certificate validator.
    pub fn new_async<F, Fut>(validator: F) -> Self
    where
        F: Fn(Vec<u8>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = McpResult<AuthenticatedIdentity>> + Send + 'static,
    {
        let validator = Arc::new(validator);
        let async_validator: CertValidatorFn = Arc::new(move |der: &[u8]| {
            let v = validator.clone();
            let der = der.to_vec();
            Box::pin(async move { v(der).await })
        });
        Self {
            validator: async_validator,
        }
    }
}

impl AuthProvider for MtlsProvider {
    fn accepts(&self, credentials: &Credentials) -> bool {
        matches!(credentials, Credentials::ClientCertificate { .. })
    }

    fn authenticate<'a>(
        &'a self,
        credentials: &'a Credentials,
    ) -> crate::auth::provider::AuthFuture<'a> {
        Box::pin(async move {
            let der = match credentials {
                Credentials::ClientCertificate { der } => der.as_slice(),
                _ => {
                    return Err(McpError::Unauthorized(
                        "MtlsProvider requires ClientCertificate credentials".into(),
                    ))
                }
            };

            (self.validator)(der).await
        })
    }
}

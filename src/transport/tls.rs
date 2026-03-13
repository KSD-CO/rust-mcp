//! TLS (HTTPS + mTLS) transport for the SSE/HTTP server.
//!
//! Wraps [`SseTransport`] in a `tokio-rustls` TLS acceptor so that all
//! connections are encrypted.  When `client_auth` is enabled in
//! [`TlsConfig`], the transport also extracts the peer certificate and
//! injects it into each Axum request's extensions as a
//! [`PeerCertificate`], making it available to the auth middleware.
//!
//! # Example
//! ```rust,no_run
//! use mcp_kit::prelude::*;
//! use mcp_kit::auth::mtls::MtlsProvider;
//! use mcp_kit::transport::tls::{TlsConfig, ServeSseTlsExt};
//! use mcp_kit::auth::IntoDynProvider;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let server = McpServer::builder()
//!         .name("secure-server")
//!         .version("1.0")
//!         .auth(MtlsProvider::new(|_der| {
//!             Ok(mcp_kit::auth::AuthenticatedIdentity::new("client"))
//!         }).into_dyn())
//!         .build();
//!
//!     let tls = TlsConfig::builder()
//!         .cert_pem("server.crt")
//!         .key_pem("server.key")
//!         .client_auth_ca_pem("ca.crt")
//!         .build()?;
//!
//!     server.serve_tls(([0, 0, 0, 0], 8443), tls).await?;
//!     Ok(())
//! }
//! ```

use std::{net::SocketAddr, sync::Arc};

use hyper::body::Incoming;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder as AutoBuilder,
};
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    server::WebPkiClientVerifier,
    RootCertStore, ServerConfig,
};
use rustls_pemfile::{certs, pkcs8_private_keys, rsa_private_keys};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing::{error, info, warn};

use crate::{
    error::{McpError, McpResult},
    server::core::McpServer,
    transport::{
        auth_layer::PeerCertificate,
        sse::{build_router, SseState},
    },
};

#[cfg(feature = "auth")]
use crate::transport::auth_layer::AuthMiddlewareState;

use dashmap::DashMap;

// ─── TlsConfig ────────────────────────────────────────────────────────────────

/// Configuration for the TLS-wrapped SSE transport.
pub struct TlsConfig {
    /// DER-encoded server certificate chain (leaf first).
    pub cert_chain: Vec<CertificateDer<'static>>,
    /// DER-encoded private key.
    pub private_key: PrivateKeyDer<'static>,
    /// Optional CA certificates used to verify client certs (enables mTLS).
    pub client_ca_certs: Vec<CertificateDer<'static>>,
}

impl TlsConfig {
    pub fn builder() -> TlsConfigBuilder {
        TlsConfigBuilder::default()
    }

    /// Build a `rustls::ServerConfig` from this `TlsConfig`.
    pub(crate) fn into_rustls_config(self) -> McpResult<Arc<ServerConfig>> {
        let mut config = if self.client_ca_certs.is_empty() {
            // TLS only — no client auth.
            ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(self.cert_chain, self.private_key)
                .map_err(|e| McpError::Transport(format!("TLS config error: {e}")))?
        } else {
            // mTLS — require client certificate.
            let mut root_store = RootCertStore::empty();
            for ca in self.client_ca_certs {
                root_store
                    .add(ca)
                    .map_err(|e| McpError::Transport(format!("CA cert error: {e}")))?;
            }
            let verifier = WebPkiClientVerifier::builder(Arc::new(root_store))
                .build()
                .map_err(|e| McpError::Transport(format!("client verifier error: {e}")))?;

            ServerConfig::builder()
                .with_client_cert_verifier(verifier)
                .with_single_cert(self.cert_chain, self.private_key)
                .map_err(|e| McpError::Transport(format!("TLS config error: {e}")))?
        };

        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        Ok(Arc::new(config))
    }
}

// ─── TlsConfigBuilder ─────────────────────────────────────────────────────────

#[derive(Default)]
pub struct TlsConfigBuilder {
    cert_pem_path: Option<String>,
    key_pem_path: Option<String>,
    client_ca_pem_path: Option<String>,
}

impl TlsConfigBuilder {
    /// Path to the server's PEM certificate file.
    pub fn cert_pem(mut self, path: impl Into<String>) -> Self {
        self.cert_pem_path = Some(path.into());
        self
    }

    /// Path to the server's PEM private key file.
    pub fn key_pem(mut self, path: impl Into<String>) -> Self {
        self.key_pem_path = Some(path.into());
        self
    }

    /// Path to a PEM file containing one or more CA certificates used to
    /// verify client certificates (enables mTLS).
    pub fn client_auth_ca_pem(mut self, path: impl Into<String>) -> Self {
        self.client_ca_pem_path = Some(path.into());
        self
    }

    pub fn build(self) -> McpResult<TlsConfig> {
        let cert_path = self
            .cert_pem_path
            .ok_or_else(|| McpError::Transport("cert_pem path is required".into()))?;
        let key_path = self
            .key_pem_path
            .ok_or_else(|| McpError::Transport("key_pem path is required".into()))?;

        let cert_chain = load_certs(&cert_path)?;
        let private_key = load_key(&key_path)?;

        let client_ca_certs = if let Some(ca_path) = self.client_ca_pem_path {
            load_certs(&ca_path)?
        } else {
            vec![]
        };

        Ok(TlsConfig {
            cert_chain,
            private_key,
            client_ca_certs,
        })
    }
}

// ─── PEM loading helpers ───────────────────────────────────────────────────────

fn load_certs(path: &str) -> McpResult<Vec<CertificateDer<'static>>> {
    let pem =
        std::fs::read(path).map_err(|e| McpError::Transport(format!("read cert {path}: {e}")))?;
    certs(&mut pem.as_slice())
        .map(|r| r.map_err(|e| McpError::Transport(format!("parse cert: {e}"))))
        .collect()
}

fn load_key(path: &str) -> McpResult<PrivateKeyDer<'static>> {
    let pem =
        std::fs::read(path).map_err(|e| McpError::Transport(format!("read key {path}: {e}")))?;

    // Try PKCS#8 first, then RSA PKCS#1.
    if let Some(key) = pkcs8_private_keys(&mut pem.as_slice())
        .filter_map(|r| r.ok())
        .next()
    {
        return Ok(PrivateKeyDer::Pkcs8(key));
    }
    if let Some(key) = rsa_private_keys(&mut pem.as_slice())
        .filter_map(|r| r.ok())
        .next()
    {
        return Ok(PrivateKeyDer::Pkcs1(key));
    }
    Err(McpError::Transport(format!(
        "no private key found in {path}"
    )))
}

// ─── TLS serve loop ───────────────────────────────────────────────────────────

/// Serve the given Axum app over TLS, injecting `PeerCertificate` into each
/// accepted connection's request extensions when client auth is enabled.
pub(crate) async fn serve_tls_loop(
    app: axum::Router,
    addr: SocketAddr,
    tls_config: Arc<ServerConfig>,
) -> McpResult<()> {
    let acceptor = TlsAcceptor::from(tls_config);
    let listener = TcpListener::bind(addr).await.map_err(McpError::Io)?;

    info!(addr = %addr, "TLS transport listening");

    loop {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                error!("TCP accept error: {e}");
                continue;
            }
        };

        let acceptor = acceptor.clone();
        let app = app.clone();

        tokio::spawn(async move {
            let tls_stream = match acceptor.accept(stream).await {
                Ok(s) => s,
                Err(e) => {
                    warn!(peer = %peer_addr, "TLS handshake failed: {e}");
                    return;
                }
            };

            // Extract peer certificate DER bytes (present only in mTLS mode).
            let peer_cert: Option<Vec<u8>> = {
                let (_, server_conn) = tls_stream.get_ref();
                server_conn
                    .peer_certificates()
                    .and_then(|certs| certs.first())
                    .map(|cert| cert.as_ref().to_vec())
            };

            let io = TokioIo::new(tls_stream);

            let svc = hyper::service::service_fn(move |mut req: axum::http::Request<Incoming>| {
                // Inject the peer cert into request extensions so the auth
                // middleware can extract it via `PeerCertificate`.
                if let Some(ref der) = peer_cert {
                    req.extensions_mut().insert(PeerCertificate(der.clone()));
                }
                let app = app.clone();
                async move {
                    use tower_service::Service;
                    let mut app = app;
                    app.call(req).await
                }
            });

            if let Err(e) = AutoBuilder::new(TokioExecutor::new())
                .serve_connection(io, svc)
                .await
            {
                error!(peer = %peer_addr, "connection error: {e}");
            }
        });
    }
}

// ─── ServeSseTlsExt ───────────────────────────────────────────────────────────

/// Extension trait that adds `.serve_tls()` to `McpServer`.
pub trait ServeSseTlsExt {
    fn serve_tls(
        self,
        addr: impl Into<SocketAddr> + Send,
        tls: TlsConfig,
    ) -> impl std::future::Future<Output = McpResult<()>> + Send;
}

impl ServeSseTlsExt for McpServer {
    #[allow(clippy::manual_async_fn)]
    fn serve_tls(
        self,
        addr: impl Into<SocketAddr> + Send,
        tls: TlsConfig,
    ) -> impl std::future::Future<Output = McpResult<()>> + Send {
        async move {
            let addr = addr.into();
            let tls_config = tls.into_rustls_config()?;

            let state = SseState {
                server: Arc::new(self.clone()),
                sessions: Arc::new(DashMap::new()),
                #[cfg(feature = "auth")]
                auth: match (self.auth_provider, self.require_auth) {
                    (Some(provider), true) => Some(AuthMiddlewareState {
                        provider,
                        require_auth: true,
                    }),
                    (Some(provider), false) => Some(AuthMiddlewareState {
                        provider,
                        require_auth: false,
                    }),
                    (None, _) => None,
                },
            };

            let app = build_router(state);
            serve_tls_loop(app, addr, tls_config).await
        }
    }
}

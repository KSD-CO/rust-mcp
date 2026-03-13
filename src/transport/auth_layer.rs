//! Axum middleware that enforces authentication on SSE/HTTP routes.
//!
//! When an [`AuthProvider`] is configured on the server, every incoming request
//! passes through [`auth_middleware`] before reaching the route handlers.
//! On success the [`AuthenticatedIdentity`] is inserted into the request's
//! Axum [`Extensions`] so that `sse_handler` / `message_handler` can store it
//! on the [`Session`].
//!
//! [`AuthProvider`]: crate::auth::AuthProvider
//! [`AuthenticatedIdentity`]: crate::auth::AuthenticatedIdentity
//! [`Session`]: crate::server::session::Session

use std::sync::Arc;

use axum::{
    extract::{Request, State as AxumState},
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::auth::{AuthenticatedIdentity, Credentials, DynAuthProvider};

// ─── mTLS peer certificate extension ─────────────────────────────────────────

/// Newtype wrapper inserted into Axum request extensions by the TLS transport
/// when the client presents a certificate during the mTLS handshake.
///
/// The inner `Vec<u8>` contains the DER-encoded certificate bytes.
#[cfg(feature = "auth-mtls")]
#[derive(Clone)]
pub struct PeerCertificate(pub Vec<u8>);

// ─── Middleware state ──────────────────────────────────────────────────────────

/// Configuration carried by the Axum middleware layer.
#[derive(Clone)]
pub struct AuthMiddlewareState {
    /// The provider used to validate incoming credentials.
    pub provider: DynAuthProvider,
    /// When `true`, requests with no credentials are rejected with 401.
    /// When `false`, unauthenticated requests proceed (identity stays `None`).
    pub require_auth: bool,
}

// ─── Middleware function ───────────────────────────────────────────────────────

/// Axum middleware that authenticates incoming requests.
///
/// Pass this to `axum::middleware::from_fn_with_state` with an
/// [`AuthMiddlewareState`] as the state.
pub async fn auth_middleware(
    AxumState(auth): AxumState<AuthMiddlewareState>,
    mut request: Request,
    next: Next,
) -> Response {
    let credentials = extract_credentials(request.headers(), request.extensions());

    if credentials.is_none() {
        if auth.require_auth {
            return unauthorized_response(&credentials);
        }
        // No credentials and auth is optional — proceed without an identity.
        return next.run(request).await;
    }

    if !auth.provider.accepts(&credentials) {
        if auth.require_auth {
            return unauthorized_response(&credentials);
        }
        return next.run(request).await;
    }

    match auth.provider.authenticate(&credentials).await {
        Ok(identity) => {
            request
                .extensions_mut()
                .insert(Arc::new(identity) as Arc<AuthenticatedIdentity>);
            next.run(request).await
        }
        Err(_) => unauthorized_response(&credentials),
    }
}

// ─── Credential extraction ────────────────────────────────────────────────────

/// Extract [`Credentials`] from request headers and extensions.
///
/// Precedence:
/// 1. mTLS peer certificate (from TLS handshake, in request extensions)
/// 2. `Authorization: Bearer <token>`
/// 3. `Authorization: Basic <b64>`  → decoded to `(username, password)`
/// 4. `X-Api-Key: <key>`
/// 5. Falls back to [`Credentials::None`]
///
/// The query-param `?api_key=` fallback is handled separately in the SSE
/// handler, because Axum exposes query params after routing.
pub fn extract_credentials(
    headers: &HeaderMap,
    extensions: &axum::http::Extensions,
) -> Credentials {
    // mTLS peer certificate takes highest precedence.
    #[cfg(feature = "auth-mtls")]
    if let Some(cert) = extensions.get::<PeerCertificate>() {
        return Credentials::ClientCertificate {
            der: cert.0.clone(),
        };
    }

    if let Some(auth_value) = headers.get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_value.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Credentials::Bearer {
                    token: token.trim().to_owned(),
                };
            }
            if let Some(encoded) = auth_str.strip_prefix("Basic ") {
                if let Ok(decoded) = decode_basic(encoded.trim()) {
                    return decoded;
                }
            }
        }
    }

    if let Some(key_value) = headers.get("x-api-key") {
        if let Ok(key) = key_value.to_str() {
            return Credentials::ApiKey {
                key: key.trim().to_owned(),
            };
        }
    }

    Credentials::None
}

fn decode_basic(encoded: &str) -> Result<Credentials, ()> {
    use std::str;

    let bytes = BASE64_ENGINE.decode(encoded).map_err(|_| ())?;
    let decoded = str::from_utf8(&bytes).map_err(|_| ())?;
    let (username, password) = decoded.split_once(':').ok_or(())?;
    Ok(Credentials::Basic {
        username: username.to_owned(),
        password: password.to_owned(),
    })
}

// base64 engine (standard alphabet, with padding)
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use base64::Engine as _;

// ─── 401 response ─────────────────────────────────────────────────────────────

fn unauthorized_response(credentials: &Credentials) -> Response {
    let www_auth = match credentials {
        Credentials::Bearer { .. } | Credentials::None => r#"Bearer realm="mcp""#,
        Credentials::Basic { .. } => r#"Basic realm="mcp""#,
        Credentials::ApiKey { .. } => r#"ApiKey realm="mcp""#,
        _ => r#"Bearer realm="mcp""#,
    };

    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, www_auth)],
        "Unauthorized",
    )
        .into_response()
}

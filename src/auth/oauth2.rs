//! OAuth 2.0 authentication provider.
//!
//! Supports two validation modes:
//!
//! - **Token Introspection** (RFC 7662) — forwards the bearer token to a
//!   remote introspection endpoint and checks `"active": true`.
//! - **JWT / JWKS** (RFC 7519 + RFC 7517) — validates JWTs locally using
//!   public keys fetched from a JWKS endpoint.
//!
//! # Example — introspection
//! ```rust,no_run
//! use mcp_kit::auth::oauth2::{OAuth2Config, OAuth2Provider};
//! use std::sync::Arc;
//!
//! let provider = Arc::new(OAuth2Provider::new(OAuth2Config::Introspection {
//!     introspection_url: "https://auth.example.com/introspect".to_owned(),
//!     client_id: "my-client".to_owned(),
//!     client_secret: "my-secret".to_owned(),
//!     cache_ttl_secs: 60,
//! }));
//! ```
//!
//! # Example — JWT / JWKS
//! ```rust,no_run
//! use mcp_kit::auth::oauth2::{OAuth2Config, OAuth2Provider};
//! use std::sync::Arc;
//!
//! let provider = Arc::new(OAuth2Provider::new(OAuth2Config::Jwt {
//!     jwks_url: "https://auth.example.com/.well-known/jwks.json".to_owned(),
//!     required_audience: Some("my-api".to_owned()),
//!     required_issuer: Some("https://auth.example.com/".to_owned()),
//!     jwks_refresh_secs: 3600,
//! }));
//! ```

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::{
    auth::{AuthProvider, AuthenticatedIdentity, Credentials},
    error::{McpError, McpResult},
};

/// Selects the OAuth 2.0 validation strategy.
pub enum OAuth2Config {
    /// Token introspection (RFC 7662).
    Introspection {
        /// The introspection endpoint URL.
        introspection_url: String,
        /// client_id sent as HTTP Basic credentials to the introspection endpoint.
        client_id: String,
        /// client_secret sent as HTTP Basic credentials.
        client_secret: String,
        /// How long (in seconds) to cache a successful introspection result.
        cache_ttl_secs: u64,
    },
    /// Local JWT validation using a remote JWKS.
    Jwt {
        /// URL of the JSON Web Key Set.
        jwks_url: String,
        /// If set, the aud claim must contain this value.
        required_audience: Option<String>,
        /// If set, the iss claim must equal this value.
        required_issuer: Option<String>,
        /// How long (in seconds) to cache the JWKS before re-fetching.
        jwks_refresh_secs: u64,
    },
}

struct CachedIntrospection {
    identity: AuthenticatedIdentity,
    expires_at: Instant,
}

struct CachedJwks {
    keys: Vec<jsonwebtoken::jwk::Jwk>,
    fetched_at: Instant,
    ttl: Duration,
}

impl CachedJwks {
    fn is_stale(&self) -> bool {
        self.fetched_at.elapsed() >= self.ttl
    }
}

/// OAuth 2.0 auth provider — introspection or JWT/JWKS.
pub struct OAuth2Provider {
    config: OAuth2Config,
    http: Client,
    introspection_cache: Mutex<HashMap<String, CachedIntrospection>>,
    jwks_cache: Mutex<Option<CachedJwks>>,
}

impl OAuth2Provider {
    pub fn new(config: OAuth2Config) -> Self {
        Self {
            config,
            http: Client::new(),
            introspection_cache: Mutex::new(HashMap::new()),
            jwks_cache: Mutex::new(None),
        }
    }
}

impl AuthProvider for OAuth2Provider {
    fn accepts(&self, credentials: &Credentials) -> bool {
        matches!(credentials, Credentials::Bearer { .. })
    }

    fn authenticate<'a>(
        &'a self,
        credentials: &'a Credentials,
    ) -> crate::auth::provider::AuthFuture<'a> {
        Box::pin(async move {
            let token = match credentials {
                Credentials::Bearer { token } => token.as_str(),
                _ => {
                    return Err(McpError::Unauthorized(
                        "OAuth2 requires a Bearer token".into(),
                    ))
                }
            };

            match &self.config {
                OAuth2Config::Introspection {
                    introspection_url,
                    client_id,
                    client_secret,
                    cache_ttl_secs,
                } => {
                    introspect(
                        &self.http,
                        &self.introspection_cache,
                        token,
                        introspection_url,
                        client_id,
                        client_secret,
                        *cache_ttl_secs,
                    )
                    .await
                }
                OAuth2Config::Jwt {
                    jwks_url,
                    required_audience,
                    required_issuer,
                    jwks_refresh_secs,
                } => {
                    validate_jwt(
                        &self.http,
                        &self.jwks_cache,
                        token,
                        jwks_url,
                        required_audience.as_deref(),
                        required_issuer.as_deref(),
                        *jwks_refresh_secs,
                    )
                    .await
                }
            }
        })
    }
}

#[derive(Deserialize)]
struct IntrospectionResponse {
    active: bool,
    sub: Option<String>,
    scope: Option<String>,
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

async fn introspect(
    http: &Client,
    cache: &Mutex<HashMap<String, CachedIntrospection>>,
    token: &str,
    url: &str,
    client_id: &str,
    client_secret: &str,
    ttl_secs: u64,
) -> McpResult<AuthenticatedIdentity> {
    {
        let guard = cache.lock().await;
        if let Some(cached) = guard.get(token) {
            if cached.expires_at > Instant::now() {
                return Ok(cached.identity.clone());
            }
        }
    }

    let resp: IntrospectionResponse = http
        .post(url)
        .basic_auth(client_id, Some(client_secret))
        .form(&[("token", token)])
        .send()
        .await
        .map_err(|e| McpError::Unauthorized(format!("introspection request failed: {e}")))?
        .json()
        .await
        .map_err(|e| McpError::Unauthorized(format!("introspection response parse error: {e}")))?;

    if !resp.active {
        return Err(McpError::Unauthorized("token is not active".into()));
    }

    let subject = resp.sub.unwrap_or_else(|| "unknown".to_owned());
    let scopes: Vec<String> = resp
        .scope
        .unwrap_or_default()
        .split_whitespace()
        .map(|s| s.to_owned())
        .collect();

    let mut identity = AuthenticatedIdentity::new(subject).with_scopes(scopes);
    for (k, v) in resp.extra {
        identity = identity.with_meta(k, v);
    }

    cache.lock().await.insert(
        token.to_owned(),
        CachedIntrospection {
            identity: identity.clone(),
            expires_at: Instant::now() + Duration::from_secs(ttl_secs),
        },
    );

    Ok(identity)
}

#[derive(Deserialize)]
struct JwksResponse {
    keys: Vec<jsonwebtoken::jwk::Jwk>,
}

#[derive(Deserialize)]
struct JwtClaims {
    sub: Option<String>,
    #[serde(rename = "scope")]
    scope: Option<String>,
    #[serde(rename = "scp")]
    scp: Option<serde_json::Value>,
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

async fn fetch_jwks(http: &Client, url: &str) -> McpResult<Vec<jsonwebtoken::jwk::Jwk>> {
    let resp: JwksResponse = http
        .get(url)
        .send()
        .await
        .map_err(|e| McpError::Unauthorized(format!("JWKS fetch failed: {e}")))?
        .json()
        .await
        .map_err(|e| McpError::Unauthorized(format!("JWKS parse error: {e}")))?;
    Ok(resp.keys)
}

async fn get_or_refresh_jwks(
    http: &Client,
    cache: &Mutex<Option<CachedJwks>>,
    url: &str,
    refresh_secs: u64,
) -> McpResult<Vec<jsonwebtoken::jwk::Jwk>> {
    let mut guard = cache.lock().await;
    if let Some(ref cached) = *guard {
        if !cached.is_stale() {
            return Ok(cached.keys.clone());
        }
    }
    let keys = fetch_jwks(http, url).await?;
    *guard = Some(CachedJwks {
        keys: keys.clone(),
        fetched_at: Instant::now(),
        ttl: Duration::from_secs(refresh_secs),
    });
    Ok(keys)
}

async fn validate_jwt(
    http: &Client,
    jwks_cache: &Mutex<Option<CachedJwks>>,
    token: &str,
    jwks_url: &str,
    required_audience: Option<&str>,
    required_issuer: Option<&str>,
    jwks_refresh_secs: u64,
) -> McpResult<AuthenticatedIdentity> {
    let keys = get_or_refresh_jwks(http, jwks_cache, jwks_url, jwks_refresh_secs).await?;

    let header = jsonwebtoken::decode_header(token)
        .map_err(|e| McpError::Unauthorized(format!("JWT header decode error: {e}")))?;

    let matching_keys: Vec<_> = if let Some(kid) = &header.kid {
        keys.iter()
            .filter(|k| k.common.key_id.as_deref() == Some(kid.as_str()))
            .collect()
    } else {
        keys.iter().collect()
    };

    if matching_keys.is_empty() {
        return Err(McpError::Unauthorized(
            "no matching JWK found for token kid".into(),
        ));
    }

    let alg = header.alg;
    let mut last_err = String::new();

    for jwk in matching_keys {
        let decoding_key = match DecodingKey::from_jwk(jwk) {
            Ok(k) => k,
            Err(e) => {
                last_err = format!("DecodingKey error: {e}");
                continue;
            }
        };

        let mut validation = Validation::new(alg);
        if let Some(iss) = required_issuer {
            validation.set_issuer(&[iss]);
        }
        if let Some(aud) = required_audience {
            validation.set_audience(&[aud]);
        } else {
            validation.validate_aud = false;
        }

        match jsonwebtoken::decode::<JwtClaims>(token, &decoding_key, &validation) {
            Ok(data) => {
                let claims = data.claims;
                let subject = claims.sub.unwrap_or_else(|| "unknown".to_owned());

                let scopes: Vec<String> = if let Some(scope_str) = claims.scope {
                    scope_str.split_whitespace().map(|s| s.to_owned()).collect()
                } else if let Some(scp_val) = claims.scp {
                    match scp_val {
                        serde_json::Value::Array(arr) => arr
                            .into_iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                            .collect(),
                        serde_json::Value::String(s) => {
                            s.split_whitespace().map(|s| s.to_owned()).collect()
                        }
                        _ => vec![],
                    }
                } else {
                    vec![]
                };

                let mut identity = AuthenticatedIdentity::new(subject).with_scopes(scopes);
                for (k, v) in claims.extra {
                    identity = identity.with_meta(k, v);
                }

                return Ok(identity);
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
    }

    Err(McpError::Unauthorized(format!(
        "JWT validation failed: {last_err}"
    )))
}

const _: Algorithm = Algorithm::RS256;

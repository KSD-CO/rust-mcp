# Auth Implementation Plan — mcp-kit

## Status

- [x] Phase 1 — Core Auth Traits & Types
- [x] Phase 2 — Transport Middleware (SSE/HTTP)
- [x] Phase 3 — Extractors & Handler Integration
- [x] Phase 4 — Builder & Macro Support
- [x] Phase 5 — OAuth 2.0
- [x] Phase 6 — mTLS / Client Certificates

---

## New Dependencies (`Cargo.toml`)

```toml
[features]
auth        = ["server"]
auth-bearer = ["auth"]
auth-apikey = ["auth"]
auth-basic  = ["auth", "dep:base64"]
auth-oauth2 = ["auth", "dep:reqwest", "dep:jsonwebtoken"]
auth-mtls   = ["auth", "dep:rustls", "dep:tokio-rustls"]
auth-full   = ["auth-bearer", "auth-apikey", "auth-basic"]
full        = ["server", "stdio", "sse", "auth-full"]

base64        = { version = "0.22", optional = true }
reqwest       = { version = "0.12", features = ["json", "rustls-tls"], optional = true }
jsonwebtoken  = { version = "9", optional = true }
rustls        = { version = "0.23", optional = true }
tokio-rustls  = { version = "0.26", optional = true }
```

---

## Phase 1 — Core Auth Traits & Types ✅

No new external dependencies (stdlib + serde only).

### New files

| File | Contents |
|---|---|
| `src/auth/credentials.rs` | `Credentials` enum: `Bearer`, `ApiKey`, `Basic`, `ClientCertificate`, `CustomHeader`, `None` |
| `src/auth/identity.rs` | `AuthenticatedIdentity { subject, scopes, metadata }` |
| `src/auth/provider.rs` | `AuthProvider` trait (object-safe) + `DynAuthProvider = Arc<dyn AuthProvider>` |
| `src/auth/bearer.rs` | `BearerTokenProvider` — static token list or custom async fn |
| `src/auth/apikey.rs` | `ApiKeyProvider` — `X-Api-Key` header or `?api_key=` query param |
| `src/auth/basic.rs` | `BasicAuthProvider` — pluggable async validator for (username, password) |
| `src/auth/custom.rs` | `CustomHeaderProvider` — user-defined header name + async validator |
| `src/auth/composite.rs` | `CompositeAuthProvider` — tries providers in order; `IntoDynProvider` extension trait |
| `src/auth/mod.rs` | Feature-gated re-exports |

### Modified files

| File | Changes |
|---|---|
| `Cargo.toml` | Added features: `auth`, `auth-bearer`, `auth-apikey`, `auth-basic`, `auth-full`; optional deps: `base64`, `reqwest`, `jsonwebtoken`, `rustls`, `tokio-rustls` |
| `src/lib.rs` | Added `pub mod auth` (gated on `feature = "auth"`); re-exported all auth types at crate root and in prelude |

---

## Phase 2 — Transport Middleware (SSE/HTTP)

### New files

| File | Contents |
|---|---|
| `src/transport/auth_layer.rs` | Axum middleware (`from_fn_with_state`): extract `Credentials` from HTTP headers → call `AuthProvider::authenticate()` → insert `AuthenticatedIdentity` into Axum extensions, or return HTTP 401 |

### Modified files

| File | Changes |
|---|---|
| `src/transport/sse.rs` | `SseState` gains `auth: Option<AuthMiddlewareState>`; `SseTransport` gains `.with_auth()` / `.with_optional_auth()`; Axum router wraps `route_layer(auth_middleware)` when auth is configured |
| `src/server/session.rs` | Add `identity: Option<AuthenticatedIdentity>` field (gated on `feature = "auth"`) |

**Credential extraction precedence (in `auth_layer.rs`):**

| Auth type | Source |
|---|---|
| Bearer | `Authorization: Bearer <token>` header |
| API Key | `X-Api-Key` header first, then `?api_key=` query param |
| Basic | `Authorization: Basic <b64>` header, decoded to `(username, password)` |
| Custom | User-specified header name |
| mTLS | Peer certificate from `ConnectInfo<TlsStream>` extension |

---

## Phase 3 — Extractors & Handler Integration

### Modified files

| File | Changes |
|---|---|
| `src/server/extract.rs` | Add `Auth` extractor — returns `McpError::Unauthorized` if `session.identity` is `None` |
| `src/server/handler.rs` | Add `AuthenticatedMarker<T>` + `ToolHandler` impl for `Fn(T, Auth) -> Fut` |
| `src/server/router.rs` | Thread `&Session` into `call_tool`, `read_resource`, `get_prompt` (gated on `feature = "auth"`) |
| `src/server/core.rs` | Pass session into dispatch calls (gated) |

---

## Phase 4 — Builder & Macro Support

### Modified files

| File | Changes |
|---|---|
| `src/server/builder.rs` | Add `.auth(provider)` / `.optional_auth(provider)` builder methods; propagate through `McpServer` → `SseTransport` |
| `mcp-kit-macros/src/lib.rs` | Auto-detect `Auth` parameter in `#[tool]` functions → emit `AuthenticatedMarker` handler |
| `src/lib.rs` | Re-export `Auth` extractor |

---

## Phase 5 — OAuth 2.0

### New files

| File | Contents |
|---|---|
| `src/auth/oauth2.rs` | `OAuth2IntrospectionProvider`: two modes — introspection (RFC 7662, with token cache) and JWT validation (RFC 7519, with JWKS endpoint + key rotation) |

### Config shape

```rust
pub enum OAuth2Config {
    Introspection {
        introspection_url: String,
        client_id: String,
        client_secret: String,
        cache_ttl: Option<Duration>,
    },
    Jwt {
        jwks_url: String,
        required_audience: Option<String>,
        required_issuer: Option<String>,
        jwks_refresh_interval: Duration,
    },
}
```

---

## Phase 6 — mTLS / Client Certificates

### New files

| File | Contents |
|---|---|
| `src/auth/mtls.rs` | `MtlsProvider`: validates peer certificate subject DN against an allow list |

### Modified files

| File | Changes |
|---|---|
| `src/transport/sse.rs` | Wrap `TcpListener` with `tokio-rustls::TlsAcceptor`; pass peer certificate via Axum extensions |

---

## Implementation Order (dependency graph)

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Macros
                ↘              ↗
              Phase 5 (OAuth2, independent)
              Phase 6 (mTLS, independent after Phase 2)
```

---

## Target ergonomics (after all phases complete)

```rust
#[tool(description = "A protected tool")]
async fn secret_tool(input: String, auth: Auth) -> McpResult<CallToolResult> {
    if !auth.has_scope("tools:execute") {
        return Err(McpError::Unauthorized("missing scope: tools:execute".into()));
    }
    Ok(CallToolResult::text(format!("Hello, {}!", auth.subject)))
}

McpServer::builder()
    .name("my-server")
    .version("1.0")
    .auth(BearerTokenProvider::new(["my-secret-token"]))
    .tool_def(secret_tool_tool_def())
    .build()
    .serve_sse("0.0.0.0:3000".parse()?)
    .await?;
```

//! MCP Server on Cloudflare Workers — Production-Ready Template
//!
//! A comprehensive, well-structured MCP server demonstrating all capabilities:
//!
//! ## Features
//! - **Tools**: Calculator, text utilities, async operations
//! - **Resources**: Static configs, dynamic data with templates
//! - **Prompts**: Code review, summarization, translation
//! - **Completion**: Auto-complete for prompt arguments
//! - **Authentication**: API Key, Bearer Token, Basic Auth
//!
//! ## Architecture
//! ```text
//! src/
//! ├── lib.rs           # Entry point & request routing
//! ├── server.rs        # MCP server builder & core
//! ├── auth/            # Authentication providers
//! │   ├── mod.rs
//! │   ├── apikey.rs
//! │   ├── bearer.rs
//! │   └── basic.rs
//! ├── tools/           # Tool handlers
//! │   ├── mod.rs
//! │   ├── calculator.rs
//! │   └── text.rs
//! ├── resources/       # Resource handlers
//! │   ├── mod.rs
//! │   ├── config.rs
//! │   └── templates.rs
//! ├── prompts/         # Prompt handlers
//! │   └── mod.rs
//! └── completion.rs    # Completion handlers
//! ```
//!
//! ## Endpoints
//! - `POST /mcp` — JSON-RPC endpoint for all MCP requests
//! - `GET  /mcp` — Server metadata and discovery
//! - `GET  /health` — Health check
//!
//! ## Authentication
//! Set `AUTH_ENABLED=true` in wrangler.toml to enable authentication.
//! Supports: API Key (X-API-Key header), Bearer token, Basic auth.
//!
//! ## Deploy
//! ```bash
//! cd deploy/cloudflare
//! npx wrangler deploy
//! ```

mod auth;
mod completion;
mod prompts;
mod resources;
mod server;
mod tools;

use mcp_kit::protocol::{JsonRpcMessage, MCP_PROTOCOL_VERSION};
use worker::*;

use crate::auth::{ApiKeyAuth, AuthResult, BasicAuth, BearerAuth, CompositeAuth, Identity};
use crate::server::get_server;

// ─── Authentication Configuration ─────────────────────────────────────────────

/// Check if authentication is enabled via environment variable.
fn is_auth_enabled(env: &Env) -> bool {
    env.var("AUTH_ENABLED")
        .map(|v| v.to_string().to_lowercase() == "true")
        .unwrap_or(false)
}

/// Build the authentication provider.
/// 
/// In production, load secrets from Cloudflare Workers Secrets:
/// ```bash
/// wrangler secret put API_KEY
/// wrangler secret put BEARER_TOKEN
/// ```
fn build_authenticator(env: &Env) -> CompositeAuth {
    let mut auth = CompositeAuth::new();

    // API Key authentication
    let mut apikey = ApiKeyAuth::new();
    
    // Try to get API key from secret, fallback to demo key
    if let Ok(key) = env.secret("API_KEY") {
        apikey = apikey.add_key(
            key.to_string(),
            Identity::new("api-user").with_role("user"),
        );
    } else {
        // Demo key for development
        apikey = apikey
            .add_key("demo-key", Identity::new("demo").with_role("demo"))
            .add_key(
                "admin-key",
                Identity::new("admin")
                    .with_name("Administrator")
                    .with_roles(["admin", "user"]),
            );
    }
    auth = auth.add(apikey);

    // Bearer token authentication
    let mut bearer = BearerAuth::new();
    if let Ok(token) = env.secret("BEARER_TOKEN") {
        bearer = bearer.add_token(
            token.to_string(),
            Identity::new("bearer-user").with_role("user"),
        );
    } else {
        // Demo tokens for development
        bearer = bearer
            .add_token("demo-token", Identity::new("demo-bearer").with_role("demo"))
            .add_token(
                "secret-token",
                Identity::new("secret-user").with_role("admin"),
            );
    }
    auth = auth.add(bearer);

    // Basic auth (demo only - use secrets in production!)
    let basic = BasicAuth::new()
        .add_simple_user("demo", "demo123")
        .add_user(
            "admin",
            "admin123",
            Identity::new("admin")
                .with_name("Admin User")
                .with_role("admin"),
        );
    auth = auth.add(basic);

    auth
}

// ─── Cloudflare Workers Entry Point ───────────────────────────────────────────

#[event(fetch)]
pub async fn main(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let method = req.method();
    let path = req.path();

    // CORS preflight
    if method == Method::Options {
        return Ok(cors_response(Response::empty()?));
    }

    // Authentication check for /mcp endpoint
    let identity = if is_auth_enabled(&env) && path == "/mcp" {
        let authenticator = build_authenticator(&env);
        match authenticator.authenticate(&req) {
            AuthResult::Authenticated(id) => Some(id),
            AuthResult::Denied(reason) => {
                return Ok(cors_response(
                    Response::error(format!("Unauthorized: {}", reason), 401)?,
                ));
            }
            AuthResult::NoCredentials => {
                return Ok(cors_response(
                    Response::error("Authentication required", 401)?
                        .with_headers(www_authenticate_header()),
                ));
            }
        }
    } else {
        None
    };

    match (method, path.as_str()) {
        // POST /mcp — JSON-RPC endpoint
        (Method::Post, "/mcp") => handle_mcp_request(&mut req, identity.as_ref()).await,

        // GET /mcp — Server discovery
        (Method::Get, "/mcp") => handle_discovery(&env),

        // Health check
        (Method::Get, "/health") => Ok(cors_response(Response::ok("OK")?)),

        _ => Response::error("Not Found", 404),
    }
}

// ─── Request Handlers ─────────────────────────────────────────────────────────

async fn handle_mcp_request(req: &mut Request, _identity: Option<&Identity>) -> Result<Response> {
    let msg: JsonRpcMessage = req
        .json()
        .await
        .map_err(|e| Error::RustError(format!("Invalid JSON: {e}")))?;

    let server = get_server();
    
    // Note: identity can be used to implement role-based access control
    // For example, check if identity.has_role("admin") before certain operations
    
    let response = server.handle(msg).await;

    match response {
        Some(resp) => {
            let body = serde_json::to_string(&resp).map_err(|e| Error::RustError(e.to_string()))?;
            Ok(cors_response(
                Response::ok(body)?.with_headers(json_headers()),
            ))
        }
        None => Ok(cors_response(Response::empty()?.with_status(202))),
    }
}

fn handle_discovery(env: &Env) -> Result<Response> {
    let server = get_server();
    let body = serde_json::json!({
        "name": server.info().name,
        "version": server.info().version,
        "protocol_version": MCP_PROTOCOL_VERSION,
        "transport": "streamable-http",
        "endpoint": "/mcp",
        "capabilities": server.capabilities_summary(),
        "authentication": {
            "enabled": is_auth_enabled(env),
            "methods": ["api-key", "bearer", "basic"]
        }
    });
    Ok(cors_response(
        Response::from_json(&body)?.with_headers(json_headers()),
    ))
}

// ─── HTTP Helpers ─────────────────────────────────────────────────────────────

fn json_headers() -> Headers {
    let h = Headers::new();
    let _ = h.set("content-type", "application/json");
    h
}

fn www_authenticate_header() -> Headers {
    let h = Headers::new();
    let _ = h.set("www-authenticate", "Bearer, Basic realm=\"MCP Server\", ApiKey");
    h
}

fn cors_response(resp: Response) -> Response {
    let h = Headers::new();
    let _ = h.set("access-control-allow-origin", "*");
    let _ = h.set("access-control-allow-methods", "GET, POST, OPTIONS");
    let _ = h.set(
        "access-control-allow-headers",
        "content-type, authorization, x-api-key",
    );
    let _ = h.set("access-control-max-age", "86400");
    resp.with_headers(h)
}

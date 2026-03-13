//! Composite Authentication Example
//!
//! Demonstrates how to combine multiple authentication providers.
//! The server will accept any of: Bearer token, API key, or Basic auth.
//!
//! Run with:
//!   cargo run --example auth_composite --features auth-full
//!
//! Test with curl:
//!   # Bearer token
//!   curl -H "Authorization: Bearer service-token" http://localhost:3000/sse
//!
//!   # API key
//!   curl -H "X-Api-Key: api-key-123" http://localhost:3000/sse
//!
//!   # Basic auth
//!   curl -u "admin:password" http://localhost:3000/sse

use mcp_kit::auth::{
    ApiKeyProvider, AuthenticatedIdentity, BasicAuthProvider, BearerTokenProvider,
    CompositeAuthProvider, IntoDynProvider,
};
use mcp_kit::prelude::*;
use mcp_kit::Auth;

#[mcp_kit::tool(description = "Show current user info")]
async fn whoami(auth: Auth) -> McpResult<CallToolResult> {
    let scopes = auth.scopes.join(", ");
    Ok(CallToolResult::text(format!(
        "Subject: {}\nScopes: [{}]\nMetadata: {:?}",
        auth.subject, scopes, auth.metadata
    )))
}

#[mcp_kit::tool(description = "Protected operation")]
async fn protected_op(data: String, auth: Auth) -> McpResult<CallToolResult> {
    Ok(CallToolResult::text(format!(
        "Operation executed by {} with data: {}",
        auth.subject, data
    )))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("info,mcp_kit=debug")
        .init();

    // Provider 1: Bearer tokens (for service-to-service auth)
    let bearer = BearerTokenProvider::with_validator(|token| {
        Box::pin(async move {
            if token == "service-token" {
                Ok(AuthenticatedIdentity::new("service-account")
                    .with_scopes(["service"])
                    .with_meta("auth_method", serde_json::json!("bearer")))
            } else {
                Err(McpError::Unauthorized("invalid bearer token".into()))
            }
        })
    });

    // Provider 2: API keys (for external integrations)
    let apikey = ApiKeyProvider::with_validator(|key| {
        Box::pin(async move {
            if key == "api-key-123" {
                Ok(AuthenticatedIdentity::new("integration-client")
                    .with_scopes(["api"])
                    .with_meta("auth_method", serde_json::json!("apikey")))
            } else {
                Err(McpError::Unauthorized("invalid API key".into()))
            }
        })
    });

    // Provider 3: Basic auth (for human users)
    let basic = BasicAuthProvider::with_validator(|username, password| {
        Box::pin(async move {
            if username == "admin" && password == "password" {
                Ok(AuthenticatedIdentity::new(&username)
                    .with_scopes(["admin", "read", "write"])
                    .with_meta("auth_method", serde_json::json!("basic")))
            } else {
                Err(McpError::Unauthorized("invalid credentials".into()))
            }
        })
    });

    // Combine all providers - tries each in order until one succeeds
    let composite =
        CompositeAuthProvider::new(vec![bearer.into_dyn(), apikey.into_dyn(), basic.into_dyn()]);

    let server = McpServer::builder()
        .name("auth-composite-example")
        .version("1.0.0")
        .instructions("A server demonstrating composite authentication")
        .auth(composite.into_dyn())
        .tool_def(whoami_tool_def())
        .tool_def(protected_op_tool_def())
        .build();

    println!("Starting server with composite auth on http://localhost:3000");
    println!("Accepts: Bearer token OR API Key OR Basic auth");

    server.serve_sse(([0, 0, 0, 0], 3000)).await?;

    Ok(())
}

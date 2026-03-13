//! Bearer Token Authentication Example
//!
//! Demonstrates how to protect MCP tools with bearer token authentication.
//!
//! Run with:
//!   cargo run --example auth_bearer --features auth-full
//!
//! Test with curl:
//!   # Without auth (will fail)
//!   curl http://localhost:3000/sse
//!
//!   # With auth
//!   curl -H "Authorization: Bearer my-secret-token" http://localhost:3000/sse

use mcp_kit::auth::{BearerTokenProvider, IntoDynProvider};
use mcp_kit::prelude::*;
use mcp_kit::Auth;

/// A protected tool that requires authentication
#[mcp_kit::tool(description = "Say hello to the authenticated user")]
async fn greet(message: String, auth: Auth) -> McpResult<CallToolResult> {
    Ok(CallToolResult::text(format!(
        "Hello, {}! Your message: {}",
        auth.subject, message
    )))
}

/// A tool that checks for specific scopes
#[mcp_kit::tool(description = "Admin-only operation")]
async fn admin_action(auth: Auth) -> McpResult<CallToolResult> {
    if !auth.has_scope("admin") {
        return Err(McpError::Unauthorized("admin scope required".into()));
    }
    Ok(CallToolResult::text("Admin action executed successfully!"))
}

/// A public tool (no auth parameter = no auth required)
#[mcp_kit::tool(description = "Get server status (public)")]
async fn status() -> CallToolResult {
    CallToolResult::text("Server is running!")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("info,mcp_kit=debug")
        .init();

    // Create a bearer token provider with static tokens
    // In production, you'd use a custom validator function
    let auth_provider = BearerTokenProvider::new(["my-secret-token", "another-token"]);

    // Or with custom validation logic:
    // let auth_provider = BearerTokenProvider::with_validator(|token| {
    //     Box::pin(async move {
    //         if token == "dynamic-token" {
    //             Ok(AuthenticatedIdentity::new("user-123")
    //                 .with_scopes(["admin", "read", "write"]))
    //         } else {
    //             Err(McpError::Unauthorized("invalid token".into()))
    //         }
    //     })
    // });

    let server = McpServer::builder()
        .name("auth-bearer-example")
        .version("1.0.0")
        .instructions("A server demonstrating bearer token authentication")
        .auth(auth_provider.into_dyn())
        .tool_def(greet_tool_def())
        .tool_def(admin_action_tool_def())
        .tool_def(status_tool_def())
        .build();

    println!("Starting server with bearer token auth on http://localhost:3000");
    println!("Use header: Authorization: Bearer my-secret-token");

    server.serve_sse(([0, 0, 0, 0], 3000)).await?;

    Ok(())
}

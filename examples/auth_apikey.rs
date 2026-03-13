//! API Key Authentication Example
//!
//! Demonstrates how to protect MCP tools with API key authentication.
//!
//! Run with:
//!   cargo run --example auth_apikey --features auth-full
//!
//! Test with curl:
//!   # Via header
//!   curl -H "X-Api-Key: api-key-123" http://localhost:3000/sse
//!
//!   # Via query parameter
//!   curl "http://localhost:3000/sse?api_key=api-key-123"

use mcp_kit::auth::{ApiKeyProvider, IntoDynProvider};
use mcp_kit::prelude::*;
use mcp_kit::Auth;

/// A protected tool
#[mcp_kit::tool(description = "Fetch user data")]
async fn get_user(user_id: String, auth: Auth) -> McpResult<CallToolResult> {
    Ok(CallToolResult::text(format!(
        "User {} fetched by API client: {}",
        user_id, auth.subject
    )))
}

/// Another protected tool
#[mcp_kit::tool(description = "Create a new resource")]
async fn create_resource(name: String, auth: Auth) -> McpResult<CallToolResult> {
    Ok(CallToolResult::text(format!(
        "Resource '{}' created by: {}",
        name, auth.subject
    )))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("info,mcp_kit=debug")
        .init();

    // Create an API key provider with static keys
    let auth_provider = ApiKeyProvider::new(["api-key-123", "api-key-456"]);

    // Or with custom validation:
    // let auth_provider = ApiKeyProvider::with_validator(|key| {
    //     Box::pin(async move {
    //         // Look up key in database, validate, etc.
    //         if key.starts_with("valid-") {
    //             Ok(AuthenticatedIdentity::new(format!("client:{}", key)))
    //         } else {
    //             Err(McpError::Unauthorized("invalid API key".into()))
    //         }
    //     })
    // });

    let server = McpServer::builder()
        .name("auth-apikey-example")
        .version("1.0.0")
        .instructions("A server demonstrating API key authentication")
        .auth(auth_provider.into_dyn())
        .tool_def(get_user_tool_def())
        .tool_def(create_resource_tool_def())
        .build();

    println!("Starting server with API key auth on http://localhost:3000");
    println!("Use header: X-Api-Key: api-key-123");
    println!("Or query param: ?api_key=api-key-123");

    server.serve_sse(([0, 0, 0, 0], 3000)).await?;

    Ok(())
}

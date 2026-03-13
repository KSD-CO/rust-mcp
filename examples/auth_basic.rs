//! Basic Authentication Example
//!
//! Demonstrates how to protect MCP tools with HTTP Basic authentication.
//!
//! Run with:
//!   cargo run --example auth_basic --features auth-full
//!
//! Test with curl:
//!   # Using -u flag (curl encodes to Base64)
//!   curl -u "admin:secret123" http://localhost:3000/sse
//!
//!   # Or with raw header (base64 of "admin:secret123")
//!   curl -H "Authorization: Basic YWRtaW46c2VjcmV0MTIz" http://localhost:3000/sse

use mcp_kit::auth::{AuthenticatedIdentity, BasicAuthProvider, IntoDynProvider};
use mcp_kit::prelude::*;
use mcp_kit::Auth;

/// A protected tool
#[mcp_kit::tool(description = "Get account balance")]
async fn get_balance(auth: Auth) -> McpResult<CallToolResult> {
    Ok(CallToolResult::text(format!(
        "Balance for {}: $1,234.56",
        auth.subject
    )))
}

/// Another protected tool with scope check
#[mcp_kit::tool(description = "Transfer money")]
async fn transfer(amount: f64, to: String, auth: Auth) -> McpResult<CallToolResult> {
    if !auth.has_scope("transfer") {
        return Err(McpError::Unauthorized(
            "transfer permission required".into(),
        ));
    }
    Ok(CallToolResult::text(format!(
        "Transferred ${} to {} by {}",
        amount, to, auth.subject
    )))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("info,mcp_kit=debug")
        .init();

    // Create a basic auth provider with custom validation
    let auth_provider = BasicAuthProvider::with_validator(|username, password| {
        Box::pin(async move {
            // In production, you'd validate against a database
            match (username.as_str(), password.as_str()) {
                ("admin", "secret123") => Ok(AuthenticatedIdentity::new("admin")
                    .with_scopes(["read", "write", "transfer", "admin"])),
                ("user", "password") => {
                    Ok(AuthenticatedIdentity::new("user").with_scopes(["read"]))
                }
                _ => Err(McpError::Unauthorized("invalid credentials".into())),
            }
        })
    });

    let server = McpServer::builder()
        .name("auth-basic-example")
        .version("1.0.0")
        .instructions("A server demonstrating HTTP Basic authentication")
        .auth(auth_provider.into_dyn())
        .tool_def(get_balance_tool_def())
        .tool_def(transfer_tool_def())
        .build();

    println!("Starting server with Basic auth on http://localhost:3000");
    println!("Test with: curl -u admin:secret123 http://localhost:3000/sse");

    server.serve_sse(([0, 0, 0, 0], 3000)).await?;

    Ok(())
}

//! OAuth 2.0 JWT Authentication Example
//!
//! Demonstrates how to validate OAuth 2.0 Bearer tokens using:
//! - Token Introspection (RFC 7662)
//! - JWT validation with JWKS (RFC 7517)
//!
//! Run with:
//!   cargo run --example auth_oauth2 --features auth-oauth2
//!
//! Note: This example requires a real OAuth 2.0 provider to work.
//! Popular options include:
//! - Auth0
//! - Keycloak
//! - Azure AD
//! - Google Cloud Identity

use mcp_kit::auth::oauth2::{OAuth2Config, OAuth2Provider};
use mcp_kit::prelude::*;
use mcp_kit::Auth;

#[allow(dead_code)]
#[mcp_kit::tool(description = "Get user profile from OAuth claims")]
async fn get_profile(auth: Auth) -> McpResult<CallToolResult> {
    let scopes = auth.scopes.join(", ");

    // Access standard OAuth claims from metadata
    let email = auth
        .metadata
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let name = auth
        .metadata
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    Ok(CallToolResult::text(format!(
        "Subject: {}\nEmail: {}\nName: {}\nScopes: [{}]",
        auth.subject, email, name, scopes
    )))
}

#[allow(dead_code)]
#[mcp_kit::tool(description = "Admin operation requiring specific scope")]
async fn admin_op(auth: Auth) -> McpResult<CallToolResult> {
    if !auth.has_scope("admin") && !auth.has_scope("mcp:admin") {
        return Err(McpError::Unauthorized("admin scope required".into()));
    }
    Ok(CallToolResult::text("Admin operation completed!"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("info,mcp_kit=debug")
        .init();

    // Example 1: JWT validation with JWKS endpoint
    // Use this when you have JWTs from an OAuth provider
    let _jwt_provider = OAuth2Provider::new(OAuth2Config::Jwt {
        // Replace with your OAuth provider's JWKS URL
        jwks_url: "https://your-tenant.auth0.com/.well-known/jwks.json".to_owned(),
        // Validate the audience claim
        required_audience: Some("https://your-api.example.com".to_owned()),
        // Validate the issuer claim
        required_issuer: Some("https://your-tenant.auth0.com/".to_owned()),
        // How long to cache the JWKS (in seconds)
        jwks_refresh_secs: 3600,
    });

    // Example 2: Token Introspection (RFC 7662)
    // Use this when tokens are opaque (not JWTs)
    let _introspection_provider = OAuth2Provider::new(OAuth2Config::Introspection {
        // Your OAuth provider's introspection endpoint
        introspection_url: "https://your-tenant.auth0.com/oauth/introspect".to_owned(),
        // Client credentials for introspection
        client_id: "your-client-id".to_owned(),
        client_secret: "your-client-secret".to_owned(),
        // How long to cache introspection results (in seconds)
        cache_ttl_secs: 60,
    });

    // For this example, we'll use a placeholder since we don't have a real OAuth server
    println!("OAuth 2.0 Example");
    println!("=================");
    println!();
    println!("This example shows how to configure OAuth 2.0 authentication.");
    println!(
        "To use it in production, replace the placeholder URLs with your OAuth provider's URLs."
    );
    println!();
    println!("JWT/JWKS Configuration (for JWTs):");
    println!("  - jwks_url: Your provider's JWKS endpoint");
    println!("  - required_audience: The API identifier");
    println!("  - required_issuer: The OAuth provider's URL");
    println!();
    println!("Introspection Configuration (for opaque tokens):");
    println!("  - introspection_url: RFC 7662 introspection endpoint");
    println!("  - client_id/client_secret: Credentials for introspection");
    println!();
    println!("Example with Auth0:");
    println!("  jwks_url: https://YOUR-TENANT.auth0.com/.well-known/jwks.json");
    println!("  required_issuer: https://YOUR-TENANT.auth0.com/");
    println!();
    println!("Example with Keycloak:");
    println!(
        "  jwks_url: https://keycloak.example.com/realms/YOUR-REALM/protocol/openid-connect/certs"
    );
    println!("  introspection_url: https://keycloak.example.com/realms/YOUR-REALM/protocol/openid-connect/token/introspect");

    // Uncomment to run with a real provider:
    // let server = McpServer::builder()
    //     .name("auth-oauth2-example")
    //     .version("1.0.0")
    //     .auth(jwt_provider.into_dyn())
    //     .tool_def(get_profile_tool_def())
    //     .tool_def(admin_op_tool_def())
    //     .build();
    //
    // server.serve_sse(([0, 0, 0, 0], 3000)).await?;

    Ok(())
}

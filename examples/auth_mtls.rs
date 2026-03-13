//! mTLS (Mutual TLS) Authentication Example
//!
//! Demonstrates how to use client certificate authentication.
//!
//! Run with:
//!   cargo run --example auth_mtls --features auth-mtls
//!
//! This example requires TLS certificates:
//!   - server.crt / server.key: Server certificate
//!   - ca.crt: CA certificate for verifying client certs
//!
//! Generate test certificates:
//! ```bash
//! # Create CA
//! openssl genrsa -out ca.key 4096
//! openssl req -new -x509 -days 365 -key ca.key -out ca.crt -subj "/CN=Test CA"
//!
//! # Create server cert
//! openssl genrsa -out server.key 2048
//! openssl req -new -key server.key -out server.csr -subj "/CN=localhost"
//! openssl x509 -req -days 365 -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out server.crt
//!
//! # Create client cert
//! openssl genrsa -out client.key 2048
//! openssl req -new -key client.key -out client.csr -subj "/CN=test-client"
//! openssl x509 -req -days 365 -in client.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out client.crt
//! ```
//!
//! Test with curl:
//!   curl --cert client.crt --key client.key --cacert ca.crt https://localhost:8443/sse

use mcp_kit::auth::mtls::MtlsProvider;
use mcp_kit::auth::{AuthenticatedIdentity, IntoDynProvider};
use mcp_kit::prelude::*;
use mcp_kit::transport::tls::{ServeSseTlsExt, TlsConfig};
use mcp_kit::Auth;

#[mcp_kit::tool(description = "Get client certificate info")]
async fn cert_info(auth: Auth) -> McpResult<CallToolResult> {
    Ok(CallToolResult::text(format!(
        "Client CN: {}\nScopes: {:?}\nMetadata: {:?}",
        auth.subject, auth.scopes, auth.metadata
    )))
}

#[mcp_kit::tool(description = "Secure operation")]
async fn secure_op(data: String, auth: Auth) -> McpResult<CallToolResult> {
    Ok(CallToolResult::text(format!(
        "Secure operation by {} with data: {}",
        auth.subject, data
    )))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("info,mcp_kit=debug")
        .init();

    // Check if certificate files exist
    let cert_path = std::path::Path::new("server.crt");
    let key_path = std::path::Path::new("server.key");
    let ca_path = std::path::Path::new("ca.crt");

    if !cert_path.exists() || !key_path.exists() || !ca_path.exists() {
        println!("mTLS Example - Certificate Setup Required");
        println!("==========================================");
        println!();
        println!("This example requires TLS certificates. Generate them with:");
        println!();
        println!("# Create CA");
        println!("openssl genrsa -out ca.key 4096");
        println!("openssl req -new -x509 -days 365 -key ca.key -out ca.crt -subj \"/CN=Test CA\"");
        println!();
        println!("# Create server certificate");
        println!("openssl genrsa -out server.key 2048");
        println!("openssl req -new -key server.key -out server.csr -subj \"/CN=localhost\"");
        println!("openssl x509 -req -days 365 -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out server.crt");
        println!();
        println!("# Create client certificate");
        println!("openssl genrsa -out client.key 2048");
        println!("openssl req -new -key client.key -out client.csr -subj \"/CN=test-client\"");
        println!("openssl x509 -req -days 365 -in client.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out client.crt");
        println!();
        println!("Then run this example again.");
        return Ok(());
    }

    // Create mTLS provider that validates client certificates
    let mtls_provider = MtlsProvider::new(|cert_der: &[u8]| {
        // In production, parse the DER certificate and extract subject info
        // You can use libraries like x509-parser or rustls-pemfile

        // For this example, we accept any valid client certificate
        // and use a placeholder subject
        let _cert_len = cert_der.len();

        Ok(AuthenticatedIdentity::new("mtls-client")
            .with_scopes(["mtls", "secure"])
            .with_meta("cert_size", serde_json::json!(cert_der.len())))
    });

    // Build TLS configuration
    let tls_config = TlsConfig::builder()
        .cert_pem("server.crt")
        .key_pem("server.key")
        .client_auth_ca_pem("ca.crt") // Enable mTLS
        .build()?;

    let server = McpServer::builder()
        .name("auth-mtls-example")
        .version("1.0.0")
        .instructions("A server demonstrating mTLS authentication")
        .auth(mtls_provider.into_dyn())
        .tool_def(cert_info_tool_def())
        .tool_def(secure_op_tool_def())
        .build();

    println!("Starting server with mTLS on https://localhost:8443");
    println!("Test with: curl --cert client.crt --key client.key --cacert ca.crt https://localhost:8443/sse");

    server.serve_tls(([0, 0, 0, 0], 8443), tls_config).await?;

    Ok(())
}

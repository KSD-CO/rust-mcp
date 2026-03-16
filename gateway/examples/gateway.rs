//! MCP Gateway Example
//!
//! This example demonstrates how to create an MCP gateway server that proxies
//! tools, resources, and prompts from one or more upstream MCP servers.
//!
//! # Usage
//!
//! First, start one or more upstream MCP servers, then run:
//!
//! ```sh
//! UPSTREAM_URL=http://localhost:3001/sse cargo run --example gateway
//! ```
//!
//! The gateway will connect to the upstream server, discover its tools/resources/prompts,
//! and expose them (with a namespace prefix) on its own SSE endpoint at `:3000`.

use std::net::SocketAddr;

use mcp_kit::prelude::*;
use mcp_kit_gateway::{GatewayManager, UpstreamConfig, UpstreamTransport};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("gateway=debug,mcp_kit=info,mcp_kit_gateway=debug")
        .init();

    // Read upstream URL from environment (default to a common dev URL)
    let upstream_url =
        std::env::var("UPSTREAM_URL").unwrap_or_else(|_| "http://localhost:3001/sse".to_string());
    let upstream_name = std::env::var("UPSTREAM_NAME").unwrap_or_else(|_| "upstream".to_string());

    eprintln!("MCP Gateway starting...");
    eprintln!("  Upstream: {upstream_name} @ {upstream_url}");

    // Configure the gateway
    let mut gw = GatewayManager::new();
    gw.add_upstream(UpstreamConfig {
        name: upstream_name,
        transport: UpstreamTransport::Sse(upstream_url),
        prefix: Some("upstream".into()),
        client_name: None,
        client_version: None,
    });

    // Build the gateway server — connects to upstreams and discovers capabilities
    let server =
        gw
            .build_server(
                McpServer::builder()
                    .name("mcp-gateway")
                    .version("1.0.0")
                    .instructions("MCP Gateway — aggregates tools from upstream servers")
                    // You can also add local tools alongside gateway tools
                    .tool(
                        Tool::no_params("gateway/status", "Check gateway status"),
                        |_args: serde_json::Value| async move {
                            CallToolResult::text("Gateway is running")
                        },
                    ),
            )
            .await?;

    eprintln!("Gateway server listening on http://localhost:3000");
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    server.serve_sse(addr).await?;

    Ok(())
}

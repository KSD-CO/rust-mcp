//! Static configuration resources.

use mcp_kit::{
    error::McpResult,
    types::{
        messages::ReadResourceRequest,
        resource::{ReadResourceResult, Resource},
    },
};

// ─── App Configuration ────────────────────────────────────────────────────────

pub fn app_config_resource() -> Resource {
    Resource::new("config://app", "App Configuration")
        .with_description("Application configuration settings")
        .with_mime_type("application/json")
}

pub async fn app_config_handler(req: ReadResourceRequest) -> McpResult<ReadResourceResult> {
    let config = serde_json::json!({
        "name": "mcp-cloudflare-demo",
        "version": "1.0.0",
        "environment": "production",
        "features": {
            "tools": true,
            "resources": true,
            "prompts": true,
            "completion": true
        },
        "limits": {
            "max_tokens": 4096,
            "timeout_ms": 30000
        }
    });

    Ok(ReadResourceResult::text(
        req.uri,
        serde_json::to_string_pretty(&config).unwrap(),
    ))
}

// ─── Server Information ───────────────────────────────────────────────────────

pub fn server_info_resource() -> Resource {
    Resource::new("info://server", "Server Information")
        .with_description("Runtime server information")
        .with_mime_type("application/json")
}

pub async fn server_info_handler(req: ReadResourceRequest) -> McpResult<ReadResourceResult> {
    let info = serde_json::json!({
        "runtime": "Cloudflare Workers",
        "transport": "Streamable HTTP",
        "endpoint": "/mcp",
        "capabilities": ["tools", "resources", "prompts", "completion"],
        "timestamp": chrono_lite_now()
    });

    Ok(ReadResourceResult::text(
        req.uri,
        serde_json::to_string_pretty(&info).unwrap(),
    ))
}

// ─── README Documentation ─────────────────────────────────────────────────────

pub fn readme_resource() -> Resource {
    Resource::new("docs://readme", "README Documentation")
        .with_description("How to use this MCP server")
        .with_mime_type("text/markdown")
}

pub async fn readme_handler(req: ReadResourceRequest) -> McpResult<ReadResourceResult> {
    Ok(ReadResourceResult::text(
        req.uri,
        include_str!("../instructions.md"),
    ))
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Simple timestamp (WASM doesn't have std::time easily).
fn chrono_lite_now() -> String {
    "2024-01-01T00:00:00Z".to_string()
}

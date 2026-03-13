//! Resource templates — dynamic resources with URI variables.

use mcp_kit::{
    error::McpResult,
    types::{
        messages::ReadResourceRequest,
        resource::{ReadResourceResult, ResourceTemplate},
    },
};

use crate::server::extract_uri_var;

// ─── User Profile Template ────────────────────────────────────────────────────

pub fn user_template() -> ResourceTemplate {
    ResourceTemplate::new("user://{id}", "User Profile")
        .with_description("Get user profile by ID")
        .with_mime_type("application/json")
}

pub async fn user_handler(req: ReadResourceRequest) -> McpResult<ReadResourceResult> {
    let id = extract_uri_var(&req.uri, "user://{id}").unwrap_or_else(|| "unknown".to_string());

    // Simulated user data (in production, fetch from KV/D1/external API)
    let user = serde_json::json!({
        "id": id,
        "name": format!("User {}", id),
        "email": format!("user{}@example.com", id),
        "role": if id == "1" { "admin" } else { "user" },
        "created_at": "2024-01-01T00:00:00Z",
        "metadata": {
            "last_login": "2024-01-15T10:30:00Z",
            "login_count": 42
        }
    });

    Ok(ReadResourceResult::text(
        req.uri,
        serde_json::to_string_pretty(&user).unwrap(),
    ))
}

// ─── Document Template ────────────────────────────────────────────────────────

pub fn document_template() -> ResourceTemplate {
    ResourceTemplate::new("doc://{id}", "Document")
        .with_description("Get document by ID")
        .with_mime_type("application/json")
}

pub async fn document_handler(req: ReadResourceRequest) -> McpResult<ReadResourceResult> {
    let id = extract_uri_var(&req.uri, "doc://{id}").unwrap_or_else(|| "unknown".to_string());

    let doc = serde_json::json!({
        "id": id,
        "title": format!("Document {}", id),
        "content": format!("This is the content of document {}.\n\nIt contains important information.", id),
        "author": "System",
        "tags": ["documentation", "example"],
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-15T12:00:00Z"
    });

    Ok(ReadResourceResult::text(
        req.uri,
        serde_json::to_string_pretty(&doc).unwrap(),
    ))
}

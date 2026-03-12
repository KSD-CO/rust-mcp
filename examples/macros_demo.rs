//! Demo of all three MCP macros: #[tool], #[resource], and #[prompt]
//!
//! Run with:
//!   cargo run --example macros_demo

use rust_mcp::prelude::*;
use rust_mcp::{prompt, resource, GetPromptRequest, ReadResourceRequest};

// ─── Tools with #[tool] macro ────────────────────────────────────────────────

/// Add two numbers together
#[tool(description = "Add two numbers and return the sum")]
async fn add(a: f64, b: f64) -> String {
    format!("{}", a + b)
}

/// Multiply two numbers
#[tool(description = "Multiply two numbers")]
async fn multiply(x: f64, y: f64) -> String {
    format!("{}", x * y)
}

/// Greet someone by name
#[tool(name = "greet", description = "Generate a greeting message")]
async fn greet_user(name: String) -> CallToolResult {
    CallToolResult::text(format!("Hello, {}! Welcome to MCP!", name))
}

// ─── Resources with #[resource] macro ────────────────────────────────────────

/// Static resource - returns app configuration
#[resource(
    uri = "config://app",
    name = "App Configuration",
    description = "Application configuration and metadata",
    mime_type = "application/json"
)]
async fn app_config(_req: ReadResourceRequest) -> McpResult<ReadResourceResult> {
    let config = serde_json::json!({
        "name": "macros_demo",
        "version": "1.0.0",
        "features": ["tools", "resources", "prompts"],
        "macros": ["#[tool]", "#[resource]", "#[prompt]"]
    });
    Ok(ReadResourceResult::text("config://app", config.to_string()))
}

/// Template resource - reads files from the filesystem
#[resource(
    uri = "file://{path}",
    name = "File System",
    description = "Read files from the local filesystem"
)]
async fn read_file(req: ReadResourceRequest) -> McpResult<ReadResourceResult> {
    let path = req.uri.trim_start_matches("file://");

    match tokio::fs::read_to_string(path).await {
        Ok(content) => Ok(ReadResourceResult::text(req.uri.clone(), content)),
        Err(e) => Err(McpError::ResourceNotFound(format!(
            "Could not read {}: {}",
            path, e
        ))),
    }
}

/// Static resource - returns server status
#[resource(
    uri = "status://server",
    name = "Server Status",
    mime_type = "application/json"
)]
async fn server_status(_req: ReadResourceRequest) -> McpResult<ReadResourceResult> {
    let status = serde_json::json!({
        "status": "running",
        "uptime": "unknown",
        "tools_count": 3,
        "resources_count": 3,
        "prompts_count": 2
    });
    Ok(ReadResourceResult::text(
        "status://server",
        status.to_string(),
    ))
}

// ─── Prompts with #[prompt] macro ────────────────────────────────────────────

/// Simple greeting prompt
#[prompt(
    name = "hello",
    description = "A friendly greeting to start a conversation"
)]
async fn hello_prompt(_req: GetPromptRequest) -> McpResult<GetPromptResult> {
    Ok(GetPromptResult::new(vec![PromptMessage::user_text(
        "Hello! I'm ready to help you with your tasks. What would you like to do today?",
    )]))
}

/// Code review prompt with parameters
#[prompt(
    name = "code-review",
    description = "Generate a code review for a given code snippet",
    arguments = ["code:required", "language:optional", "focus:optional"]
)]
async fn code_review_prompt(req: GetPromptRequest) -> McpResult<GetPromptResult> {
    let code = req.arguments.get("code").cloned().unwrap_or_default();
    let language = req
        .arguments
        .get("language")
        .cloned()
        .unwrap_or_else(|| "unknown".into());
    let focus = req
        .arguments
        .get("focus")
        .cloned()
        .unwrap_or_else(|| "general quality".into());

    let prompt_text = format!(
        "Please review the following {language} code with a focus on {focus}:\n\n```{language}\n{code}\n```\n\nProvide feedback on:\n- Code quality and best practices\n- Potential bugs or issues\n- Suggestions for improvement\n- Security considerations"
    );

    Ok(GetPromptResult::new(vec![PromptMessage::user_text(
        prompt_text,
    )]))
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Set up logging (write to stderr for stdio transport)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("macros_demo=info,mcp=debug")
        .init();

    tracing::info!("Starting macros_demo MCP server");

    // Build the server using all three macro-generated definitions
    McpServer::builder()
        .name("macros-demo")
        .version("1.0.0")
        .instructions(
            "Demo server showcasing #[tool], #[resource], and #[prompt] macros. \
             Try calling the tools, reading resources, or using prompts!",
        )
        // Register tools using _tool_def() functions
        .tool_def(add_tool_def())
        .tool_def(multiply_tool_def())
        .tool_def(greet_user_tool_def())
        // Register resources using _resource_def() functions
        .resource_def(app_config_resource_def())
        .resource_def(read_file_resource_def())
        .resource_def(server_status_resource_def())
        // Register prompts using _prompt_def() functions
        .prompt_def(hello_prompt_prompt_def())
        .prompt_def(code_review_prompt_prompt_def())
        .build()
        .serve_stdio()
        .await?;

    Ok(())
}

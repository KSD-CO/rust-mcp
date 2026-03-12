//! "Everything" MCP server — demonstrates tools, resources, and prompts.
//!
//! Run with:
//!   cargo run --bin everything
//!
//! Or over SSE:
//!   cargo run --bin everything -- --sse

use mcp::prelude::*;
use mcp::{GetPromptRequest, ReadResourceRequest};
use schemars::JsonSchema;
use serde::Deserialize;

// ─── Shared state ─────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
struct AppState {
    counter: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

// ─── Tool parameter types ─────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
struct EchoInput {
    /// The message to echo back
    message: String,
}

#[derive(Deserialize, JsonSchema)]
struct ReverseInput {
    /// The string to reverse
    text: String,
}

#[derive(Deserialize, JsonSchema)]
struct RepeatInput {
    /// The string to repeat
    text: String,
    /// How many times to repeat it (1–100)
    times: u32,
}

#[derive(Deserialize, JsonSchema)]
struct WaitInput {
    /// Number of milliseconds to sleep (max 5000)
    ms: u64,
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let use_sse = std::env::args().any(|a| a == "--sse");

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("everything=debug,mcp=info")
        .init();

    let echo_schema    = serde_json::to_value(schemars::schema_for!(EchoInput))?;
    let reverse_schema = serde_json::to_value(schemars::schema_for!(ReverseInput))?;
    let repeat_schema  = serde_json::to_value(schemars::schema_for!(RepeatInput))?;
    let wait_schema    = serde_json::to_value(schemars::schema_for!(WaitInput))?;

    let server = McpServer::builder()
        .name("everything")
        .version("1.0.0")
        .instructions(
            "A demonstration server showing all MCP capabilities: tools, resources, and prompts.",
        )
        // ── Tools ─────────────────────────────────────────────────────────────
        .tool(
            Tool::new("echo", "Echo the input message back unchanged", echo_schema),
            |params: EchoInput| async move { CallToolResult::text(params.message) },
        )
        .tool(
            Tool::new("reverse", "Reverse a string", reverse_schema),
            |params: ReverseInput| async move {
                CallToolResult::text(params.text.chars().rev().collect::<String>())
            },
        )
        .tool(
            Tool::new("repeat", "Repeat a string N times", repeat_schema),
            |params: RepeatInput| async move {
                let times = params.times.min(100);
                CallToolResult::text(params.text.repeat(times as usize))
            },
        )
        .tool(
            Tool::new("wait", "Sleep for a given number of milliseconds", wait_schema),
            |params: WaitInput| async move {
                let ms = params.ms.min(5000);
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                CallToolResult::text(format!("Waited {ms}ms"))
            },
        )
        .tool(
            Tool::new(
                "current-time",
                "Return the current UTC date/time",
                serde_json::json!({ "type": "object", "properties": {} }),
            ),
            |_args: serde_json::Value| async move {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                CallToolResult::text(format!("Unix timestamp: {now}"))
            },
        )
        // ── Resources ─────────────────────────────────────────────────────────
        .resource(
            Resource::new("info://server", "Server Info")
                .with_description("Information about this server")
                .with_mime_type("application/json"),
            |_req: ReadResourceRequest| async move {
                Ok(ReadResourceResult::text(
                    "info://server",
                    serde_json::json!({
                        "name": "everything",
                        "version": "1.0.0",
                        "capabilities": ["tools", "resources", "prompts"]
                    })
                    .to_string(),
                ))
            },
        )
        .resource(
            Resource::new("text://sample", "Sample Text")
                .with_description("A sample plain-text resource")
                .with_mime_type("text/plain"),
            |_req: ReadResourceRequest| async move {
                Ok(ReadResourceResult::text(
                    "text://sample",
                    "This is a sample text resource served by the MCP server.",
                ))
            },
        )
        // URI-template resource: fetch://docs/{topic}
        .resource_template(
            ResourceTemplate::new("fetch://docs/{topic}", "Documentation Page"),
            |req: ReadResourceRequest| async move {
                let topic = req
                    .uri
                    .trim_start_matches("fetch://docs/")
                    .to_owned();
                Ok(ReadResourceResult::text(
                    req.uri.clone(),
                    format!("# {topic}\n\nThis is the documentation page for `{topic}`.\n"),
                ))
            },
        )
        // ── Prompts ───────────────────────────────────────────────────────────
        .prompt(
            Prompt::new("summarize")
                .with_description("Summarize text provided by the user")
                .with_arguments(vec![
                    PromptArgument::required("text")
                        .with_description("The text to summarize"),
                    PromptArgument::optional("style")
                        .with_description("Output style: 'bullet' or 'paragraph'"),
                ]),
            |req: GetPromptRequest| async move {
                let text = req.arguments.get("text").cloned().unwrap_or_default();
                let style = req.arguments.get("style").cloned().unwrap_or_else(|| "paragraph".into());
                Ok(GetPromptResult::new(vec![
                    PromptMessage::user_text(format!(
                        "Please summarize the following text in {style} style:\n\n{text}"
                    )),
                ]))
            },
        )
        .prompt(
            Prompt::new("code-review")
                .with_description("Generate a code review for a given code snippet")
                .with_arguments(vec![
                    PromptArgument::required("code").with_description("The code to review"),
                    PromptArgument::optional("language").with_description("Programming language"),
                ]),
            |req: GetPromptRequest| async move {
                let code = req.arguments.get("code").cloned().unwrap_or_default();
                let lang = req.arguments.get("language").cloned().unwrap_or_else(|| "unknown".into());
                Ok(GetPromptResult::new(vec![
                    PromptMessage::user_text(format!(
                        "Please review the following {lang} code and provide constructive feedback:\n\n```{lang}\n{code}\n```"
                    )),
                ]))
            },
        )
        .build();

    if use_sse {
        let addr: std::net::SocketAddr = "127.0.0.1:3000".parse()?;
        eprintln!("Starting SSE server on http://{addr}");
        server.serve_sse(addr).await?;
    } else {
        server.serve_stdio().await?;
    }

    Ok(())
}

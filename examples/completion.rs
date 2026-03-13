//! Example: Completion handlers for auto-completing prompt arguments
//!
//! This example demonstrates how to provide argument completion for prompts and resources.
//!
//! Run: cargo run --example completion

use mcp_kit::prelude::*;
use mcp_kit::types::messages::{CompleteRequest, CompletionReference};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("completion=debug,mcp=info")
        .init();

    McpServer::builder()
        .name("completion-demo")
        .version("1.0.0")
        .instructions("A server demonstrating argument completion")
        // Prompt with associated completion handler
        .prompt_with_completion(
            Prompt::new("code-review")
                .with_description("Review code in a specific language")
                .with_arguments(vec![
                    PromptArgument::required("language").with_description("Programming language"),
                    PromptArgument::optional("style").with_description("Review style (brief/detailed)"),
                ]),
            // Prompt handler
            |req: mcp_kit::types::messages::GetPromptRequest| async move {
                let language = req.arguments.get("language").cloned().unwrap_or_default();
                let style = req.arguments.get("style").cloned().unwrap_or("brief".into());
                Ok(GetPromptResult::new(vec![PromptMessage::user_text(format!(
                    "Please review the following {} code in {} style:",
                    language, style
                ))])
                .with_description(format!("Code review prompt for {} ({})", language, style)))
            },
            // Completion handler for this prompt
            |req: CompleteRequest| async move {
                let values = match req.argument.name.as_str() {
                    "language" => {
                        // Filter languages based on current input
                        let languages = ["rust", "python", "javascript", "typescript", "go", "java"];
                        languages
                            .iter()
                            .filter(|l| l.starts_with(&req.argument.value.to_lowercase()))
                            .map(|s| s.to_string())
                            .collect()
                    }
                    "style" => vec!["brief".into(), "detailed".into(), "security".into()],
                    _ => vec![],
                };
                Ok(CompleteResult::new(values))
            },
        )
        // Global completion handler for resources
        .completion(|req: CompleteRequest| async move {
            match &req.reference {
                CompletionReference::Resource { uri } => {
                    // Auto-complete file paths
                    if uri.starts_with("file://") {
                        let values: Vec<String> = vec![
                            "file:///src/main.rs".into(),
                            "file:///src/lib.rs".into(),
                            "file:///Cargo.toml".into(),
                        ];
                        return Ok(CompleteResult::new(values));
                    }
                }
                CompletionReference::Prompt { name: _ } => {
                    // Fallback for any prompt not handled by prompt-specific handler
                }
            }
            Ok(CompleteResult::empty())
        })
        // Resource with completion
        .resource(
            Resource::new("file://{path}", "Source file").with_description("Read source files"),
            |req: mcp_kit::types::messages::ReadResourceRequest| async move {
                Ok(ReadResourceResult::text(
                    req.uri.clone(),
                    format!("Content of {}", req.uri),
                ))
            },
        )
        .build()
        .serve_stdio()
        .await?;

    Ok(())
}

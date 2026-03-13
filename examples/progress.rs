//! Progress tracking example
//!
//! This example demonstrates how to use ProgressTracker for long-running operations.
//!
//! Run with:
//!   cargo run --example progress
//!   cargo run --example progress -- --sse

use mcp_kit::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Debug, Deserialize, JsonSchema)]
struct LongTaskInput {
    /// Duration in seconds (1-10)
    duration: u64,
    /// Number of steps (1-20)
    steps: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "progress=debug,mcp_kit=info".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let use_sse = args.iter().any(|a| a == "--sse");

    let server = McpServer::builder()
        .name("progress-demo")
        .version("1.0.0")
        .instructions(
            "Demonstrates simulated long-running operations.\n\
             The long_task tool simulates work with configurable duration and steps.\n\
             In a real implementation, progress would be reported to the client.",
        )
        .tool(
            Tool::new(
                "long_task",
                "Run a long task (simulates progress)",
                serde_json::to_value(schemars::schema_for!(LongTaskInput))?,
            ),
            |params: LongTaskInput| async move {
                let duration = params.duration.clamp(1, 10);
                let steps = params.steps.clamp(1, 20);
                let step_duration = Duration::from_secs(duration) / steps;

                let mut progress_log = Vec::new();

                for i in 0..steps {
                    tokio::time::sleep(step_duration).await;

                    let progress_pct = ((i + 1) as f64 / steps as f64) * 100.0;
                    progress_log.push(format!(
                        "Step {}/{}: {:.0}% complete",
                        i + 1,
                        steps,
                        progress_pct
                    ));
                }

                CallToolResult::text(format!(
                    "Long task completed!\n\nProgress log:\n{}",
                    progress_log.join("\n")
                ))
            },
        )
        .tool(
            Tool::new(
                "quick_task",
                "Quick task that returns immediately",
                serde_json::json!({"type": "object"}),
            ),
            |_: serde_json::Value| async move { CallToolResult::text("Quick task done!") },
        )
        .build();

    if use_sse {
        eprintln!("Starting SSE server on http://0.0.0.0:3000");
        let addr: SocketAddr = "0.0.0.0:3000".parse()?;
        server.serve_sse(addr).await?;
    } else {
        eprintln!("Starting stdio server...");
        server.serve_stdio().await?;
    }

    Ok(())
}

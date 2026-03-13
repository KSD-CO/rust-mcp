//! Streamable HTTP Transport Example
//!
//! This example demonstrates the Streamable HTTP transport (MCP 2025-03-26 spec).
//! It uses a single endpoint that can return either JSON or SSE streaming responses.
//!
//! Run: `cargo run --example streamable_http`
//! Test with curl:
//!
//! ```bash
//! # Initialize session
//! curl -X POST http://localhost:3000/mcp \
//!   -H "Content-Type: application/json" \
//!   -d '{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"curl","version":"1.0"}}}'
//!
//! # List tools (use session ID from response)
//! curl -X POST http://localhost:3000/mcp \
//!   -H "Content-Type: application/json" \
//!   -H "Mcp-Session-Id: <session-id>" \
//!   -d '{"jsonrpc":"2.0","method":"tools/list","id":2}'
//!
//! # Call a tool
//! curl -X POST http://localhost:3000/mcp \
//!   -H "Content-Type: application/json" \
//!   -H "Mcp-Session-Id: <session-id>" \
//!   -d '{"jsonrpc":"2.0","method":"tools/call","id":3,"params":{"name":"greet","arguments":{"name":"World"}}}'
//!
//! # Open SSE stream for notifications (in another terminal)
//! curl -N -X GET http://localhost:3000/mcp \
//!   -H "Mcp-Session-Id: <session-id>"
//!
//! # Terminate session
//! curl -X DELETE http://localhost:3000/mcp \
//!   -H "Mcp-Session-Id: <session-id>"
//! ```

use mcp_kit::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct GreetInput {
    /// Name to greet
    name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CalculateInput {
    /// First operand
    a: f64,
    /// Second operand
    b: f64,
    /// Operation: add, subtract, multiply, divide
    operation: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("streamable_http=debug,mcp_kit=info")
        .init();

    let greet_schema = serde_json::to_value(schemars::schema_for!(GreetInput))?;
    let calc_schema = serde_json::to_value(schemars::schema_for!(CalculateInput))?;

    let server = McpServer::builder()
        .name("streamable-http-example")
        .version("1.0.0")
        .instructions("Example MCP server using Streamable HTTP transport")
        // Simple greeting tool
        .tool(
            Tool::new("greet", "Greet someone by name", greet_schema),
            |input: GreetInput| async move {
                CallToolResult::text(format!("Hello, {}! 👋", input.name))
            },
        )
        // Calculator tool
        .tool(
            Tool::new("calculate", "Perform basic math operations", calc_schema),
            |input: CalculateInput| async move {
                let result = match input.operation.as_str() {
                    "add" => input.a + input.b,
                    "subtract" => input.a - input.b,
                    "multiply" => input.a * input.b,
                    "divide" => {
                        if input.b == 0.0 {
                            return CallToolResult::error("Division by zero");
                        }
                        input.a / input.b
                    }
                    _ => return CallToolResult::error("Unknown operation"),
                };
                CallToolResult::text(format!(
                    "{} {} {} = {}",
                    input.a, input.operation, input.b, result
                ))
            },
        )
        .build();

    println!("🚀 Streamable HTTP server starting on http://localhost:3000/mcp");
    println!();
    println!("Try these commands:");
    println!();
    println!("1. Initialize:");
    println!(
        r#"   curl -X POST http://localhost:3000/mcp -H "Content-Type: application/json" -d '{{"jsonrpc":"2.0","method":"initialize","id":1,"params":{{"protocolVersion":"2025-03-26","capabilities":{{}},"clientInfo":{{"name":"curl","version":"1.0"}}}}}}'"#
    );
    println!();
    println!("2. List tools (use session ID from response):");
    println!(
        r#"   curl -X POST http://localhost:3000/mcp -H "Content-Type: application/json" -H "Mcp-Session-Id: <session-id>" -d '{{"jsonrpc":"2.0","method":"tools/list","id":2}}'"#
    );
    println!();
    println!("3. Call greet tool:");
    println!(
        r#"   curl -X POST http://localhost:3000/mcp -H "Content-Type: application/json" -H "Mcp-Session-Id: <session-id>" -d '{{"jsonrpc":"2.0","method":"tools/call","id":3,"params":{{"name":"greet","arguments":{{"name":"World"}}}}}}'"#
    );

    server.serve_streamable(([0, 0, 0, 0], 3000)).await?;

    Ok(())
}

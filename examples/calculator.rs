//! Calculator MCP server — demonstrates both the typed closure API and the raw JSON API.
//!
//! Run with:
//!   cargo run --example calculator

use mcp::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

// ─── Typed parameter structs ─────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
struct BinaryInput {
    /// First operand
    a: f64,
    /// Second operand
    b: f64,
}

#[derive(Deserialize, JsonSchema)]
struct PowerInput {
    /// Base number
    base: f64,
    /// Exponent
    exponent: f64,
}

#[derive(Deserialize, JsonSchema)]
struct SqrtInput {
    /// The number to take the square root of (must be ≥ 0)
    n: f64,
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("calculator=info,mcp=debug")
        .init();

    let binary_schema = serde_json::to_value(schemars::schema_for!(BinaryInput))?;
    let power_schema  = serde_json::to_value(schemars::schema_for!(PowerInput))?;
    let sqrt_schema   = serde_json::to_value(schemars::schema_for!(SqrtInput))?;

    McpServer::builder()
        .name("calculator")
        .version("1.0.0")
        .instructions(
            "A simple calculator server. Supports add, subtract, multiply, divide, power, sqrt.",
        )
        .tool(
            Tool::new("add", "Add two numbers together", binary_schema.clone()),
            |params: BinaryInput| async move {
                CallToolResult::text(format!("{}", params.a + params.b))
            },
        )
        .tool(
            Tool::new("subtract", "Subtract b from a", binary_schema.clone()),
            |params: BinaryInput| async move {
                CallToolResult::text(format!("{}", params.a - params.b))
            },
        )
        .tool(
            Tool::new("multiply", "Multiply two numbers", binary_schema.clone()),
            |params: BinaryInput| async move {
                CallToolResult::text(format!("{}", params.a * params.b))
            },
        )
        .tool(
            Tool::new("divide", "Divide a by b", binary_schema),
            |params: BinaryInput| async move {
                if params.b == 0.0 {
                    return CallToolResult::error("Division by zero");
                }
                CallToolResult::text(format!("{}", params.a / params.b))
            },
        )
        .tool(
            Tool::new("power", "Raise base to the given exponent", power_schema),
            |params: PowerInput| async move {
                CallToolResult::text(format!("{}", params.base.powf(params.exponent)))
            },
        )
        .tool(
            Tool::new("sqrt", "Compute the square root of a number", sqrt_schema),
            |params: SqrtInput| async move {
                if params.n < 0.0 {
                    return CallToolResult::error(
                        "Cannot take square root of a negative number",
                    );
                }
                CallToolResult::text(format!("{}", params.n.sqrt()))
            },
        )
        .tool(
            Tool::new(
                "factorial",
                "Compute n! (integer, n ≤ 20)",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "n": {
                            "type": "integer",
                            "description": "Non-negative integer (≤ 20)",
                            "minimum": 0,
                            "maximum": 20
                        }
                    },
                    "required": ["n"]
                }),
            ),
            |args: serde_json::Value| async move {
                let n = args["n"].as_u64().unwrap_or(0).min(20);
                let result: u64 = (1..=n).product();
                CallToolResult::text(format!("{result}"))
            },
        )
        .build()
        .serve_stdio()
        .await?;

    Ok(())
}

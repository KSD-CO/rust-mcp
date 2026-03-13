//! Calculator tools: add, subtract, multiply, divide, sqrt.

use mcp_kit::{
    error::McpResult,
    types::tool::{CallToolResult, Tool},
};
use serde::Deserialize;
use serde_json::Value;

// ─── Input Types ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct BinaryInput {
    pub a: f64,
    pub b: f64,
}

#[derive(Deserialize)]
pub struct UnaryInput {
    pub n: f64,
}

// ─── Schemas ──────────────────────────────────────────────────────────────────

fn binary_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "a": { "type": "number", "description": "First operand" },
            "b": { "type": "number", "description": "Second operand" }
        },
        "required": ["a", "b"]
    })
}

fn unary_schema(desc: &str) -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "n": { "type": "number", "description": desc }
        },
        "required": ["n"]
    })
}

// ─── Add ──────────────────────────────────────────────────────────────────────

pub fn add_tool() -> Tool {
    Tool::new("add", "Add two numbers", binary_schema())
}

pub async fn add_handler(input: BinaryInput) -> McpResult<CallToolResult> {
    Ok(CallToolResult::text(format!("{}", input.a + input.b)))
}

// ─── Subtract ─────────────────────────────────────────────────────────────────

pub fn subtract_tool() -> Tool {
    Tool::new("subtract", "Subtract b from a", binary_schema())
}

pub async fn subtract_handler(input: BinaryInput) -> McpResult<CallToolResult> {
    Ok(CallToolResult::text(format!("{}", input.a - input.b)))
}

// ─── Multiply ─────────────────────────────────────────────────────────────────

pub fn multiply_tool() -> Tool {
    Tool::new("multiply", "Multiply two numbers", binary_schema())
}

pub async fn multiply_handler(input: BinaryInput) -> McpResult<CallToolResult> {
    Ok(CallToolResult::text(format!("{}", input.a * input.b)))
}

// ─── Divide ───────────────────────────────────────────────────────────────────

pub fn divide_tool() -> Tool {
    Tool::new("divide", "Divide a by b", binary_schema())
}

pub async fn divide_handler(input: BinaryInput) -> McpResult<CallToolResult> {
    if input.b == 0.0 {
        return Ok(CallToolResult::error("Division by zero"));
    }
    Ok(CallToolResult::text(format!("{}", input.a / input.b)))
}

// ─── Sqrt ─────────────────────────────────────────────────────────────────────

pub fn sqrt_tool() -> Tool {
    Tool::new("sqrt", "Square root of n", unary_schema("Non-negative number"))
}

pub async fn sqrt_handler(input: UnaryInput) -> McpResult<CallToolResult> {
    if input.n < 0.0 {
        return Ok(CallToolResult::error("Cannot compute sqrt of negative number"));
    }
    Ok(CallToolResult::text(format!("{}", input.n.sqrt())))
}

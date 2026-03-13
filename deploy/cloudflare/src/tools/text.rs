//! Text utility tools: uppercase, lowercase, reverse, word_count, echo.

use mcp_kit::{
    error::McpResult,
    types::{
        content::Content,
        tool::{CallToolResult, Tool},
    },
};
use serde::Deserialize;
use serde_json::Value;

// ─── Input Types ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TextInput {
    pub text: String,
}

#[derive(Deserialize)]
pub struct EchoInput {
    pub message: String,
}

// ─── Schemas ──────────────────────────────────────────────────────────────────

fn text_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": { "type": "string", "description": "Input text" }
        },
        "required": ["text"]
    })
}

fn echo_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "message": { "type": "string", "description": "Message to echo" }
        },
        "required": ["message"]
    })
}

// ─── Uppercase ────────────────────────────────────────────────────────────────

pub fn uppercase_tool() -> Tool {
    Tool::new("uppercase", "Convert text to uppercase", text_schema())
}

pub async fn uppercase_handler(input: TextInput) -> McpResult<CallToolResult> {
    Ok(CallToolResult::text(input.text.to_uppercase()))
}

// ─── Lowercase ────────────────────────────────────────────────────────────────

pub fn lowercase_tool() -> Tool {
    Tool::new("lowercase", "Convert text to lowercase", text_schema())
}

pub async fn lowercase_handler(input: TextInput) -> McpResult<CallToolResult> {
    Ok(CallToolResult::text(input.text.to_lowercase()))
}

// ─── Reverse ──────────────────────────────────────────────────────────────────

pub fn reverse_tool() -> Tool {
    Tool::new("reverse", "Reverse the characters in text", text_schema())
}

pub async fn reverse_handler(input: TextInput) -> McpResult<CallToolResult> {
    Ok(CallToolResult::text(
        input.text.chars().rev().collect::<String>(),
    ))
}

// ─── Word Count ───────────────────────────────────────────────────────────────

pub fn word_count_tool() -> Tool {
    Tool::new("word_count", "Count words in text", text_schema())
}

pub async fn word_count_handler(input: TextInput) -> McpResult<CallToolResult> {
    let count = input.text.split_whitespace().count();
    Ok(CallToolResult::text(format!("{count}")))
}

// ─── Echo ─────────────────────────────────────────────────────────────────────

pub fn echo_tool() -> Tool {
    Tool::new("echo", "Echo back the input with metadata", echo_schema())
}

pub async fn echo_handler(input: EchoInput) -> McpResult<CallToolResult> {
    Ok(CallToolResult::success(vec![
        Content::text(format!("You said: {}", input.message)),
        Content::text(format!("Length: {} chars", input.message.len())),
    ]))
}

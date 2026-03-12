use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::content::Content;

/// A tool that the server exposes to clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema object describing the tool's input parameters
    pub input_schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
}

impl Tool {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: Some(description.into()),
            input_schema,
            annotations: None,
        }
    }

    /// Create a tool with an empty object schema (no parameters)
    pub fn no_params(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(
            name,
            description,
            serde_json::json!({ "type": "object", "properties": {} }),
        )
    }
}

/// Behavioural annotations for tools
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// If true, the tool does not mutate external state (safe to cache/retry)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
    /// If true, the tool may cause destructive side-effects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
    /// If true, calling the tool multiple times with the same args has the same effect as calling once
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,
    /// If true, the tool interacts with the external world (network, filesystem, …)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,
}

/// The result returned from a tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    pub content: Vec<Content>,
    /// If true, the tool call errored (in-band error, not a protocol error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl CallToolResult {
    /// Successful result with a single text content
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![Content::text(text)],
            is_error: None,
        }
    }

    /// Successful result with multiple content items
    pub fn success(content: Vec<Content>) -> Self {
        Self {
            content,
            is_error: None,
        }
    }

    /// Error result (in-band)
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![Content::text(message)],
            is_error: Some(true),
        }
    }

    pub fn with_content(mut self, content: impl Into<Content>) -> Self {
        self.content.push(content.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.is_error.unwrap_or(false)
    }
}

impl From<String> for CallToolResult {
    fn from(s: String) -> Self {
        Self::text(s)
    }
}

impl From<&str> for CallToolResult {
    fn from(s: &str) -> Self {
        Self::text(s)
    }
}

//! Elicitation types for server-initiated user input requests.
//!
//! Elicitation allows MCP servers to request additional information from users
//! through the client. This is useful for:
//! - Confirming dangerous operations
//! - Requesting missing parameters
//! - Getting user preferences
//! - Multi-step workflows requiring user decisions
//!
//! # Example
//!
//! ```rust,ignore
//! use mcp_kit::types::elicitation::*;
//!
//! // Request user confirmation
//! let request = ElicitRequest {
//!     message: "Delete all files in /tmp?".to_string(),
//!     requested_schema: ElicitSchema::boolean(),
//! };
//!
//! // Or request structured input
//! let request = ElicitRequest {
//!     message: "Configure backup settings".to_string(),
//!     requested_schema: ElicitSchema::object(serde_json::json!({
//!         "type": "object",
//!         "properties": {
//!             "path": { "type": "string", "description": "Backup path" },
//!             "compress": { "type": "boolean", "default": true }
//!         },
//!         "required": ["path"]
//!     })),
//! };
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Request to elicit information from the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitRequest {
    /// Human-readable message explaining what information is needed.
    pub message: String,

    /// Schema describing the expected response format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_schema: Option<ElicitSchema>,
}

impl ElicitRequest {
    /// Create a new elicitation request with just a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            requested_schema: None,
        }
    }

    /// Create a request with a specific schema.
    pub fn with_schema(message: impl Into<String>, schema: ElicitSchema) -> Self {
        Self {
            message: message.into(),
            requested_schema: Some(schema),
        }
    }

    /// Create a yes/no confirmation request.
    pub fn confirm(message: impl Into<String>) -> Self {
        Self::with_schema(message, ElicitSchema::boolean())
    }

    /// Create a text input request.
    pub fn text(message: impl Into<String>) -> Self {
        Self::with_schema(message, ElicitSchema::string())
    }

    /// Create a choice selection request.
    pub fn choice(message: impl Into<String>, options: Vec<String>) -> Self {
        Self::with_schema(message, ElicitSchema::enum_values(options))
    }
}

/// Schema for elicitation response validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElicitSchema {
    /// JSON Schema definition.
    #[serde(flatten)]
    pub schema: Value,
}

impl ElicitSchema {
    /// Create a schema from a JSON value.
    pub fn new(schema: Value) -> Self {
        Self { schema }
    }

    /// Create a boolean (yes/no) schema.
    pub fn boolean() -> Self {
        Self::new(serde_json::json!({
            "type": "boolean",
            "description": "Confirmation response (true/false)"
        }))
    }

    /// Create a string input schema.
    pub fn string() -> Self {
        Self::new(serde_json::json!({
            "type": "string"
        }))
    }

    /// Create a string with description.
    pub fn string_with_desc(description: impl Into<String>) -> Self {
        Self::new(serde_json::json!({
            "type": "string",
            "description": description.into()
        }))
    }

    /// Create an enum schema for selecting from options.
    pub fn enum_values(values: Vec<String>) -> Self {
        Self::new(serde_json::json!({
            "type": "string",
            "enum": values
        }))
    }

    /// Create a number input schema.
    pub fn number() -> Self {
        Self::new(serde_json::json!({
            "type": "number"
        }))
    }

    /// Create a number with min/max constraints.
    pub fn number_range(min: f64, max: f64) -> Self {
        Self::new(serde_json::json!({
            "type": "number",
            "minimum": min,
            "maximum": max
        }))
    }

    /// Create an integer input schema.
    pub fn integer() -> Self {
        Self::new(serde_json::json!({
            "type": "integer"
        }))
    }

    /// Create an object schema from a JSON Schema definition.
    pub fn object(schema: Value) -> Self {
        Self::new(schema)
    }
}

/// Result of an elicitation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitResult {
    /// The action taken by the user.
    pub action: ElicitAction,

    /// The content provided by the user (if accepted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
}

impl ElicitResult {
    /// Create an accepted result with content.
    pub fn accepted(content: Value) -> Self {
        Self {
            action: ElicitAction::Accepted,
            content: Some(content),
        }
    }

    /// Create a declined result.
    pub fn declined() -> Self {
        Self {
            action: ElicitAction::Declined,
            content: None,
        }
    }

    /// Create a cancelled result.
    pub fn cancelled() -> Self {
        Self {
            action: ElicitAction::Cancelled,
            content: None,
        }
    }

    /// Check if the user accepted.
    pub fn is_accepted(&self) -> bool {
        matches!(self.action, ElicitAction::Accepted)
    }

    /// Check if the user declined.
    pub fn is_declined(&self) -> bool {
        matches!(self.action, ElicitAction::Declined)
    }

    /// Check if the request was cancelled.
    pub fn is_cancelled(&self) -> bool {
        matches!(self.action, ElicitAction::Cancelled)
    }

    /// Get the content as a specific type.
    pub fn content_as<T: serde::de::DeserializeOwned>(&self) -> Option<T> {
        self.content
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Get content as bool (for confirmation requests).
    pub fn as_bool(&self) -> Option<bool> {
        self.content_as()
    }

    /// Get content as string.
    pub fn as_string(&self) -> Option<String> {
        self.content_as()
    }
}

/// The action taken by the user in response to an elicitation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElicitAction {
    /// User provided the requested information.
    Accepted,
    /// User explicitly declined to provide information.
    Declined,
    /// Request was cancelled (e.g., timeout, user closed dialog).
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirm_request() {
        let req = ElicitRequest::confirm("Delete all files?");
        assert_eq!(req.message, "Delete all files?");
        assert!(req.requested_schema.is_some());
    }

    #[test]
    fn test_choice_request() {
        let req = ElicitRequest::choice(
            "Select format",
            vec!["json".into(), "yaml".into(), "toml".into()],
        );
        assert_eq!(req.message, "Select format");
    }

    #[test]
    fn test_result_accepted() {
        let result = ElicitResult::accepted(serde_json::json!(true));
        assert!(result.is_accepted());
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn test_result_declined() {
        let result = ElicitResult::declined();
        assert!(result.is_declined());
        assert!(result.content.is_none());
    }
}

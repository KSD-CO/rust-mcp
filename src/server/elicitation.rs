//! Elicitation client for requesting user input from clients.
//!
//! Elicitation allows MCP servers to request input from users through the client.
//! This is useful for interactive workflows where the server needs additional
//! information that wasn't provided in the initial request.
//!
//! # Example
//!
//! ```rust,ignore
//! use mcp_kit::server::ElicitationClientExt;
//!
//! // In a tool handler with access to an elicitation client
//! let client: &impl ElicitationClientExt = get_client();
//!
//! // Simple confirmation
//! if client.confirm("Delete this file?").await? {
//!     // User confirmed
//! }
//!
//! // Text input
//! if let Some(name) = client.prompt_text("Enter project name").await? {
//!     println!("Project: {}", name);
//! }
//!
//! // Choice selection
//! let options = vec!["small".into(), "medium".into(), "large".into()];
//! if let Some(size) = client.choose("Select size", options).await? {
//!     println!("Size: {}", size);
//! }
//! ```

use crate::types::elicitation::{ElicitAction, ElicitRequest, ElicitResult, ElicitSchema};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

// ─── Elicitation Client Trait ────────────────────────────────────────────────

/// Trait for clients that can request user input through elicitation.
pub trait ElicitationClient: Send + Sync {
    /// Send an elicitation request to the client.
    fn elicit(
        &self,
        request: ElicitRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ElicitResult, ElicitationError>> + Send + '_>>;
}

/// Extension trait for convenient elicitation methods.
pub trait ElicitationClientExt: ElicitationClient {
    /// Request a simple yes/no confirmation from the user.
    fn confirm(
        &self,
        message: &str,
    ) -> Pin<Box<dyn Future<Output = Result<bool, ElicitationError>> + Send + '_>> {
        let request = ElicitRequest::confirm(message);
        Box::pin(async move {
            let result = self.elicit(request).await?;
            Ok(matches!(result.action, ElicitAction::Accepted))
        })
    }

    /// Request text input from the user.
    fn prompt_text(
        &self,
        message: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>, ElicitationError>> + Send + '_>> {
        let request = ElicitRequest::text(message);
        Box::pin(async move {
            let result = self.elicit(request).await?;
            match result.action {
                ElicitAction::Accepted => Ok(result.as_string()),
                _ => Ok(None),
            }
        })
    }

    /// Request the user to choose from a list of options.
    fn choose(
        &self,
        message: &str,
        options: Vec<String>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>, ElicitationError>> + Send + '_>> {
        let request = ElicitRequest::choice(message, options);
        Box::pin(async move {
            let result = self.elicit(request).await?;
            match result.action {
                ElicitAction::Accepted => Ok(result.as_string()),
                _ => Ok(None),
            }
        })
    }

    /// Request a number from the user.
    fn prompt_number(
        &self,
        message: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<f64>, ElicitationError>> + Send + '_>> {
        let request = ElicitRequest::with_schema(message, ElicitSchema::number());
        Box::pin(async move {
            let result = self.elicit(request).await?;
            match result.action {
                ElicitAction::Accepted => Ok(result.content_as::<f64>()),
                _ => Ok(None),
            }
        })
    }
}

// Blanket implementation for all ElicitationClient
impl<T: ElicitationClient + ?Sized> ElicitationClientExt for T {}

// ─── Elicitation Error ───────────────────────────────────────────────────────

/// Errors that can occur during elicitation.
#[derive(Debug, thiserror::Error)]
pub enum ElicitationError {
    /// The client doesn't support elicitation.
    #[error("Elicitation not supported by client")]
    NotSupported,

    /// The elicitation request was cancelled.
    #[error("Elicitation cancelled")]
    Cancelled,

    /// The client connection was lost.
    #[error("Client connection lost")]
    ConnectionLost,

    /// A timeout occurred waiting for response.
    #[error("Elicitation timeout")]
    Timeout,

    /// An error occurred during elicitation.
    #[error("Elicitation error: {0}")]
    Other(String),
}

// ─── Channel-based Elicitation Client ────────────────────────────────────────

/// An elicitation request with a response channel.
pub struct ElicitationRequestMessage {
    /// The elicitation request.
    pub request: ElicitRequest,
    /// Channel to send the response.
    pub response_tx: oneshot::Sender<Result<ElicitResult, ElicitationError>>,
}

/// Channel-based elicitation client.
///
/// This client sends elicitation requests through an mpsc channel,
/// which can be processed by the transport layer.
#[derive(Clone)]
pub struct ChannelElicitationClient {
    tx: mpsc::Sender<ElicitationRequestMessage>,
}

impl ChannelElicitationClient {
    /// Create a new channel-based elicitation client.
    pub fn new(tx: mpsc::Sender<ElicitationRequestMessage>) -> Self {
        Self { tx }
    }

    /// Create a new channel pair for elicitation.
    pub fn channel(buffer: usize) -> (Self, mpsc::Receiver<ElicitationRequestMessage>) {
        let (tx, rx) = mpsc::channel(buffer);
        (Self::new(tx), rx)
    }
}

impl ElicitationClient for ChannelElicitationClient {
    fn elicit(
        &self,
        request: ElicitRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ElicitResult, ElicitationError>> + Send + '_>> {
        Box::pin(async move {
            let (response_tx, response_rx) = oneshot::channel();

            self.tx
                .send(ElicitationRequestMessage {
                    request,
                    response_tx,
                })
                .await
                .map_err(|_| ElicitationError::ConnectionLost)?;

            response_rx
                .await
                .map_err(|_| ElicitationError::ConnectionLost)?
        })
    }
}

// ─── Arc wrapper implementation ──────────────────────────────────────────────

impl<T: ElicitationClient + ?Sized> ElicitationClient for Arc<T> {
    fn elicit(
        &self,
        request: ElicitRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ElicitResult, ElicitationError>> + Send + '_>> {
        (**self).elicit(request)
    }
}

// ─── Elicitation Request Builder ─────────────────────────────────────────────

/// Builder for creating complex elicitation requests with multiple fields.
#[derive(Debug, Default)]
pub struct ElicitationRequestBuilder {
    message: String,
    properties: serde_json::Map<String, serde_json::Value>,
    required: Vec<String>,
}

impl ElicitationRequestBuilder {
    /// Create a new builder with the given message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            properties: serde_json::Map::new(),
            required: Vec::new(),
        }
    }

    /// Add a boolean field to the schema.
    pub fn boolean(mut self, name: impl Into<String>, title: impl Into<String>) -> Self {
        let name = name.into();
        self.properties.insert(
            name.clone(),
            serde_json::json!({
                "type": "boolean",
                "title": title.into()
            }),
        );
        self
    }

    /// Add a required boolean field.
    pub fn boolean_required(mut self, name: impl Into<String>, title: impl Into<String>) -> Self {
        let name = name.into();
        self.required.push(name.clone());
        self.boolean(name, title)
    }

    /// Add a text field to the schema.
    pub fn text(mut self, name: impl Into<String>, title: impl Into<String>) -> Self {
        let name = name.into();
        self.properties.insert(
            name.clone(),
            serde_json::json!({
                "type": "string",
                "title": title.into()
            }),
        );
        self
    }

    /// Add a required text field.
    pub fn text_required(mut self, name: impl Into<String>, title: impl Into<String>) -> Self {
        let name = name.into();
        self.required.push(name.clone());
        self.text(name, title)
    }

    /// Add a number field to the schema.
    pub fn number(mut self, name: impl Into<String>, title: impl Into<String>) -> Self {
        let name = name.into();
        self.properties.insert(
            name.clone(),
            serde_json::json!({
                "type": "number",
                "title": title.into()
            }),
        );
        self
    }

    /// Add a required number field.
    pub fn number_required(mut self, name: impl Into<String>, title: impl Into<String>) -> Self {
        let name = name.into();
        self.required.push(name.clone());
        self.number(name, title)
    }

    /// Add an enum field to the schema.
    pub fn select(
        mut self,
        name: impl Into<String>,
        title: impl Into<String>,
        options: &[&str],
    ) -> Self {
        let name = name.into();
        self.properties.insert(
            name.clone(),
            serde_json::json!({
                "type": "string",
                "title": title.into(),
                "enum": options
            }),
        );
        self
    }

    /// Add a required enum field.
    pub fn select_required(
        mut self,
        name: impl Into<String>,
        title: impl Into<String>,
        options: &[&str],
    ) -> Self {
        let name = name.into();
        self.required.push(name.clone());
        self.select(name, title, options)
    }

    /// Build the elicitation request.
    pub fn build(self) -> ElicitRequest {
        let schema = serde_json::json!({
            "type": "object",
            "properties": self.properties,
            "required": self.required
        });

        ElicitRequest::with_schema(self.message, ElicitSchema::object(schema))
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_channel_elicitation_client() {
        let (client, mut rx) = ChannelElicitationClient::channel(10);

        // Spawn a handler that accepts elicitation requests
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let _ = msg
                    .response_tx
                    .send(Ok(ElicitResult::accepted(serde_json::json!(
                        "test response"
                    ))));
            }
        });

        // Test prompt_text
        let result = client.prompt_text("Enter something").await.unwrap();
        assert_eq!(result, Some("test response".to_string()));
    }

    #[tokio::test]
    async fn test_confirm() {
        let (client, mut rx) = ChannelElicitationClient::channel(10);

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let _ = msg
                    .response_tx
                    .send(Ok(ElicitResult::accepted(serde_json::json!(true))));
            }
        });

        let result = client.confirm("Are you sure?").await.unwrap();
        assert!(result);
    }

    #[test]
    fn test_elicitation_request_builder() {
        let request = ElicitationRequestBuilder::new("Configure your project")
            .text_required("name", "Project Name")
            .boolean("private", "Private Repository")
            .number("port", "Port Number")
            .select("language", "Language", &["rust", "python", "javascript"])
            .build();

        assert_eq!(request.message, "Configure your project");
        assert!(request.requested_schema.is_some());

        let schema = request.requested_schema.unwrap();
        let props = schema.schema.get("properties").unwrap();
        assert!(props.get("name").is_some());
        assert!(props.get("private").is_some());
        assert!(props.get("port").is_some());
        assert!(props.get("language").is_some());
    }
}

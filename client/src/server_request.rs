//! Server request handling for MCP clients.
//!
//! When the server sends requests to the client (e.g., sampling, elicitation),
//! the client needs to handle them and send responses back.

use mcp_kit::protocol::{JsonRpcRequest, JsonRpcResponse};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Result type for server request handlers.
pub type ServerRequestResult = Result<serde_json::Value, ServerRequestError>;

/// Error type for server request handlers.
#[derive(Debug, Clone)]
pub struct ServerRequestError {
    pub code: i32,
    pub message: String,
}

impl ServerRequestError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Method not supported by this client.
    pub fn method_not_found(method: &str) -> Self {
        Self::new(-32601, format!("Method not found: {}", method))
    }

    /// Invalid parameters.
    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self::new(-32602, msg)
    }

    /// Internal error.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(-32603, msg)
    }
}

/// A boxed async function that handles server requests.
pub type ServerRequestHandlerFn = Arc<
    dyn Fn(JsonRpcRequest) -> Pin<Box<dyn Future<Output = ServerRequestResult> + Send>>
        + Send
        + Sync,
>;

/// Handler for server-initiated requests.
///
/// Implement this trait to handle requests from the server, such as:
/// - `sampling/createMessage` - LLM sampling requests
/// - `elicitation/create` - User input requests
/// - `roots/list` - List client roots
#[derive(Clone, Default)]
pub struct ServerRequestHandler {
    handlers: Arc<std::collections::HashMap<String, ServerRequestHandlerFn>>,
}

impl ServerRequestHandler {
    /// Create a new empty handler.
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(std::collections::HashMap::new()),
        }
    }

    /// Create a builder for configuring handlers.
    pub fn builder() -> ServerRequestHandlerBuilder {
        ServerRequestHandlerBuilder::new()
    }

    /// Handle a server request.
    ///
    /// Returns `None` if no handler is registered for the method.
    pub async fn handle(&self, request: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let handler = self.handlers.get(&request.method)?;

        let result = handler(request.clone()).await;

        Some(match result {
            Ok(value) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: value,
            },
            Err(e) => {
                // Create error response
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: serde_json::json!({
                        "error": {
                            "code": e.code,
                            "message": e.message
                        }
                    }),
                }
            }
        })
    }

    /// Check if a handler is registered for a method.
    pub fn has_handler(&self, method: &str) -> bool {
        self.handlers.contains_key(method)
    }
}

/// Builder for `ServerRequestHandler`.
pub struct ServerRequestHandlerBuilder {
    handlers: std::collections::HashMap<String, ServerRequestHandlerFn>,
}

impl ServerRequestHandlerBuilder {
    pub fn new() -> Self {
        Self {
            handlers: std::collections::HashMap::new(),
        }
    }

    /// Register a handler for a specific method.
    pub fn handler<F, Fut>(mut self, method: impl Into<String>, handler: F) -> Self
    where
        F: Fn(JsonRpcRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ServerRequestResult> + Send + 'static,
    {
        let handler = Arc::new(move |req: JsonRpcRequest| {
            let fut = handler(req);
            Box::pin(fut) as Pin<Box<dyn Future<Output = ServerRequestResult> + Send>>
        });
        self.handlers.insert(method.into(), handler);
        self
    }

    /// Register a sampling handler.
    ///
    /// Called when the server requests LLM sampling via `sampling/createMessage`.
    pub fn sampling<F, Fut>(self, handler: F) -> Self
    where
        F: Fn(JsonRpcRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ServerRequestResult> + Send + 'static,
    {
        self.handler("sampling/createMessage", handler)
    }

    /// Register an elicitation handler.
    ///
    /// Called when the server requests user input via `elicitation/create`.
    pub fn elicitation<F, Fut>(self, handler: F) -> Self
    where
        F: Fn(JsonRpcRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ServerRequestResult> + Send + 'static,
    {
        self.handler("elicitation/create", handler)
    }

    /// Register a roots list handler.
    ///
    /// Called when the server requests the client's root URIs.
    pub fn roots_list<F, Fut>(self, handler: F) -> Self
    where
        F: Fn(JsonRpcRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ServerRequestResult> + Send + 'static,
    {
        self.handler("roots/list", handler)
    }

    /// Build the handler.
    pub fn build(self) -> ServerRequestHandler {
        ServerRequestHandler {
            handlers: Arc::new(self.handlers),
        }
    }
}

impl Default for ServerRequestHandlerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

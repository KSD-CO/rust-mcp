//! MCP Client implementation.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use mcp_kit::protocol::{
    JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, RequestId, MCP_PROTOCOL_VERSION,
};
use mcp_kit::types::messages::{
    CallToolRequest, CompleteRequest, CompleteResult, GetPromptRequest, GetPromptResult,
    InitializeRequest, InitializeResult, ListPromptsResult, ListResourcesResult, ListToolsResult,
    ReadResourceRequest, ReadResourceResult,
};
use mcp_kit::types::{ClientCapabilities, ClientInfo, ServerCapabilities};
use mcp_kit::{CallToolResult, Prompt, Resource, Tool};
use serde_json::Value;
use tracing::{debug, info};

use crate::error::{ClientError, ClientResult};
use crate::transport::BoxTransport;

#[cfg(feature = "stdio")]
use crate::transport::stdio::StdioTransport;

#[cfg(feature = "sse")]
use crate::transport::sse::SseTransport;

#[cfg(feature = "websocket")]
use crate::transport::websocket::WebSocketTransport;

#[cfg(feature = "streamable-http")]
use crate::transport::streamable::StreamableHttpTransport;

/// MCP Client for connecting to MCP servers.
pub struct McpClient {
    transport: BoxTransport,
    request_id: AtomicU64,
    initialized: AtomicBool,
    server_info: Arc<tokio::sync::RwLock<Option<ServerInfo>>>,
    server_capabilities: Arc<tokio::sync::RwLock<Option<ServerCapabilities>>>,
}

/// Server information returned after initialization.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    /// Server name
    pub name: String,
    /// Server version
    pub version: String,
}

impl McpClient {
    /// Create a new client with the given transport.
    fn new(transport: BoxTransport) -> Self {
        Self {
            transport,
            request_id: AtomicU64::new(1),
            initialized: AtomicBool::new(false),
            server_info: Arc::new(tokio::sync::RwLock::new(None)),
            server_capabilities: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Create a client builder.
    pub fn builder() -> McpClientBuilder {
        McpClientBuilder::new()
    }

    /// Connect to an MCP server via stdio (subprocess).
    #[cfg(feature = "stdio")]
    pub async fn stdio(program: impl AsRef<Path>) -> ClientResult<Self> {
        let transport = StdioTransport::spawn(program, &[], &[]).await?;
        Ok(Self::new(Box::new(transport)))
    }

    /// Connect to an MCP server via stdio with arguments.
    #[cfg(feature = "stdio")]
    pub async fn stdio_with_args(
        program: impl AsRef<Path>,
        args: &[&str],
        env: &[(&str, &str)],
    ) -> ClientResult<Self> {
        let transport = StdioTransport::spawn(program, args, env).await?;
        Ok(Self::new(Box::new(transport)))
    }

    /// Connect to an MCP server via SSE.
    #[cfg(feature = "sse")]
    pub async fn sse(url: &str) -> ClientResult<Self> {
        let transport = SseTransport::connect(url).await?;
        Ok(Self::new(Box::new(transport)))
    }

    /// Connect to an MCP server via WebSocket.
    #[cfg(feature = "websocket")]
    pub async fn websocket(url: &str) -> ClientResult<Self> {
        let transport = WebSocketTransport::connect(url).await?;
        Ok(Self::new(Box::new(transport)))
    }

    /// Connect to an MCP server via Streamable HTTP (MCP 2025-03-26 spec).
    #[cfg(feature = "streamable-http")]
    pub async fn streamable_http(url: &str) -> ClientResult<Self> {
        let transport = StreamableHttpTransport::connect(url).await?;
        Ok(Self::new(Box::new(transport)))
    }

    /// Connect to an MCP server via Streamable HTTP with custom endpoint.
    #[cfg(feature = "streamable-http")]
    pub async fn streamable_http_with_endpoint(url: &str, endpoint: &str) -> ClientResult<Self> {
        let transport = StreamableHttpTransport::connect_with_endpoint(url, endpoint).await?;
        Ok(Self::new(Box::new(transport)))
    }

    /// Get the next request ID.
    fn next_request_id(&self) -> RequestId {
        RequestId::Number(self.request_id.fetch_add(1, Ordering::SeqCst) as i64)
    }

    /// Send a JSON-RPC request and wait for a response.
    async fn request<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> ClientResult<T> {
        let request = JsonRpcRequest {
            jsonrpc: mcp_kit::protocol::JSONRPC_VERSION.to_string(),
            id: self.next_request_id(),
            method: method.to_string(),
            params,
        };

        debug!("Sending request: {} (id={:?})", method, request.id);

        let response = self
            .transport
            .request(JsonRpcMessage::Request(request))
            .await?;

        match response {
            JsonRpcMessage::Response(resp) => Ok(serde_json::from_value(resp.result)?),
            JsonRpcMessage::Error(err) => Err(ClientError::ServerError {
                code: err.error.code as i32,
                message: err.error.message,
            }),
            _ => Err(ClientError::InvalidResponse("Expected response".into())),
        }
    }

    /// Send a notification to the server.
    async fn notify(&self, method: &str, params: Option<Value>) -> ClientResult<()> {
        let notification = JsonRpcNotification {
            jsonrpc: mcp_kit::protocol::JSONRPC_VERSION.to_string(),
            method: method.to_string(),
            params,
        };

        self.transport.notify(notification).await
    }

    // ─── MCP Operations ──────────────────────────────────────────────────────

    /// Initialize the connection with the server.
    ///
    /// This must be called before any other operations.
    pub async fn initialize(
        &self,
        client_name: &str,
        client_version: &str,
    ) -> ClientResult<ServerInfo> {
        let params = InitializeRequest {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: ClientInfo {
                name: client_name.to_string(),
                version: client_version.to_string(),
            },
        };

        let result: InitializeResult = self
            .request("initialize", Some(serde_json::to_value(&params)?))
            .await?;

        let server_info = ServerInfo {
            name: result.server_info.name.clone(),
            version: result.server_info.version.clone(),
        };

        // Store server info and capabilities
        {
            let mut info = self.server_info.write().await;
            *info = Some(server_info.clone());
        }
        {
            let mut caps = self.server_capabilities.write().await;
            *caps = Some(result.capabilities);
        }

        // Send initialized notification
        self.notify("notifications/initialized", None).await?;

        self.initialized.store(true, Ordering::SeqCst);

        info!(
            "Connected to MCP server: {} v{}",
            server_info.name, server_info.version
        );

        Ok(server_info)
    }

    /// Check if the client is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    /// Ensure the client is initialized.
    fn ensure_initialized(&self) -> ClientResult<()> {
        if !self.is_initialized() {
            return Err(ClientError::NotInitialized);
        }
        Ok(())
    }

    /// List available tools.
    pub async fn list_tools(&self) -> ClientResult<Vec<Tool>> {
        self.ensure_initialized()?;
        let result: ListToolsResult = self.request("tools/list", None).await?;
        Ok(result.tools)
    }

    /// Call a tool with the given arguments.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> ClientResult<CallToolResult> {
        self.ensure_initialized()?;
        let params = CallToolRequest {
            name: name.to_string(),
            arguments,
        };
        self.request("tools/call", Some(serde_json::to_value(&params)?))
            .await
    }

    /// List available resources.
    pub async fn list_resources(&self) -> ClientResult<Vec<Resource>> {
        self.ensure_initialized()?;
        let result: ListResourcesResult = self.request("resources/list", None).await?;
        Ok(result.resources)
    }

    /// Read a resource by URI.
    pub async fn read_resource(&self, uri: &str) -> ClientResult<ReadResourceResult> {
        self.ensure_initialized()?;
        let params = ReadResourceRequest {
            uri: uri.to_string(),
        };
        self.request("resources/read", Some(serde_json::to_value(&params)?))
            .await
    }

    /// List available prompts.
    pub async fn list_prompts(&self) -> ClientResult<Vec<Prompt>> {
        self.ensure_initialized()?;
        let result: ListPromptsResult = self.request("prompts/list", None).await?;
        Ok(result.prompts)
    }

    /// Get a prompt by name.
    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<std::collections::HashMap<String, String>>,
    ) -> ClientResult<GetPromptResult> {
        self.ensure_initialized()?;
        let params = GetPromptRequest {
            name: name.to_string(),
            arguments: arguments.unwrap_or_default(),
        };
        self.request("prompts/get", Some(serde_json::to_value(&params)?))
            .await
    }

    /// Request completion suggestions.
    pub async fn complete(&self, request: CompleteRequest) -> ClientResult<CompleteResult> {
        self.ensure_initialized()?;
        self.request("completion/complete", Some(serde_json::to_value(&request)?))
            .await
    }

    /// Subscribe to a resource for updates.
    pub async fn subscribe(&self, uri: &str) -> ClientResult<()> {
        self.ensure_initialized()?;
        let params = serde_json::json!({ "uri": uri });
        self.request::<Value>("resources/subscribe", Some(params))
            .await?;
        Ok(())
    }

    /// Unsubscribe from a resource.
    pub async fn unsubscribe(&self, uri: &str) -> ClientResult<()> {
        self.ensure_initialized()?;
        let params = serde_json::json!({ "uri": uri });
        self.request::<Value>("resources/unsubscribe", Some(params))
            .await?;
        Ok(())
    }

    /// Receive a notification from the server.
    ///
    /// This blocks until a notification is received.
    pub async fn recv_notification(&self) -> ClientResult<JsonRpcNotification> {
        self.transport.recv_notification().await
    }

    /// Check if the transport is connected.
    pub fn is_connected(&self) -> bool {
        self.transport.is_connected()
    }

    /// Close the connection.
    pub async fn close(&self) -> ClientResult<()> {
        self.transport.close().await
    }

    /// Get the server capabilities.
    pub async fn server_capabilities(&self) -> Option<ServerCapabilities> {
        self.server_capabilities.read().await.clone()
    }

    /// Get the server info.
    pub async fn server_info(&self) -> Option<ServerInfo> {
        self.server_info.read().await.clone()
    }
}

/// Builder for creating MCP clients.
pub struct McpClientBuilder {
    #[allow(dead_code)]
    client_name: String,
    #[allow(dead_code)]
    client_version: String,
}

impl McpClientBuilder {
    /// Create a new client builder.
    pub fn new() -> Self {
        Self {
            client_name: "mcp-client".to_string(),
            client_version: "1.0.0".to_string(),
        }
    }

    /// Set the client name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.client_name = name.into();
        self
    }

    /// Set the client version.
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.client_version = version.into();
        self
    }

    /// Connect via stdio subprocess.
    #[cfg(feature = "stdio")]
    pub async fn stdio(self, program: impl AsRef<Path>) -> ClientResult<McpClient> {
        McpClient::stdio(program).await
    }

    /// Connect via stdio subprocess with arguments.
    #[cfg(feature = "stdio")]
    pub async fn stdio_with_args(
        self,
        program: impl AsRef<Path>,
        args: &[&str],
        env: &[(&str, &str)],
    ) -> ClientResult<McpClient> {
        McpClient::stdio_with_args(program, args, env).await
    }

    /// Connect via SSE.
    #[cfg(feature = "sse")]
    pub async fn sse(self, url: &str) -> ClientResult<McpClient> {
        McpClient::sse(url).await
    }

    /// Connect via WebSocket.
    #[cfg(feature = "websocket")]
    pub async fn websocket(self, url: &str) -> ClientResult<McpClient> {
        McpClient::websocket(url).await
    }
}

impl Default for McpClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

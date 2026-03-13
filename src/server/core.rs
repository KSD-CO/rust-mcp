use std::sync::Arc;

use crate::{
    error::{McpError, McpResult},
    protocol::{
        JsonRpcError, JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
        MCP_PROTOCOL_VERSION,
    },
    types::{
        messages::{
            CallToolRequest, CompleteRequest, GetPromptRequest, InitializeRequest,
            InitializeResult, ListPromptsRequest, ListResourcesRequest, ListToolsRequest,
            ReadResourceRequest, SetLevelRequest, SubscribeRequest, UnsubscribeRequest,
        },
        LoggingCapability, PromptsCapability, ResourcesCapability, ServerCapabilities, ServerInfo,
        ToolsCapability,
    },
};
use serde_json::Value;
use tracing::{debug, error, info, warn};

use crate::server::{router::Router, session::Session};

#[cfg(feature = "auth")]
use crate::auth::DynAuthProvider;

/// The core MCP server — holds configuration and the routing table.
///
/// Create one with `McpServer::builder()` then call `.serve_stdio()` or
/// another transport extension method to start accepting connections.
#[derive(Clone)]
pub struct McpServer {
    pub(crate) info: ServerInfo,
    pub(crate) instructions: Option<String>,
    pub(crate) router: Arc<Router>,
    #[cfg(feature = "auth")]
    pub(crate) auth_provider: Option<DynAuthProvider>,
    #[cfg(feature = "auth")]
    pub(crate) require_auth: bool,
}

impl McpServer {
    pub fn builder() -> crate::server::builder::McpServerBuilder {
        crate::server::builder::McpServerBuilder::new()
    }

    /// Handle a single incoming JSON-RPC message, returning the response (if any).
    pub async fn handle_message(
        &self,
        msg: JsonRpcMessage,
        session: &mut Session,
    ) -> Option<JsonRpcMessage> {
        match msg {
            JsonRpcMessage::Request(req) => {
                let id = req.id.clone();
                match self.dispatch_request(req, session).await {
                    Ok(result) => Some(JsonRpcMessage::Response(JsonRpcResponse {
                        jsonrpc: "2.0".to_owned(),
                        id,
                        result,
                    })),
                    Err(e) => {
                        error!(error = %e, "Request failed");
                        Some(JsonRpcMessage::Error(JsonRpcError::new(id, e)))
                    }
                }
            }
            JsonRpcMessage::Notification(notif) => {
                self.handle_notification(notif, session).await;
                None
            }
            _ => None,
        }
    }

    async fn dispatch_request(
        &self,
        req: JsonRpcRequest,
        session: &mut Session,
    ) -> McpResult<Value> {
        let params = req.params.unwrap_or(Value::Null);
        debug!(method = %req.method, "Dispatching request");

        match req.method.as_str() {
            "initialize" => {
                let init: InitializeRequest = serde_json::from_value(params)
                    .map_err(|e| McpError::InvalidParams(e.to_string()))?;
                self.handle_initialize(init, session).await
            }
            "ping" => Ok(serde_json::json!({})),
            "tools/list" => {
                self.require_initialized(session)?;
                let req: ListToolsRequest = serde_json::from_value(params).unwrap_or_default();
                Ok(serde_json::to_value(
                    self.router.list_tools(req.cursor.as_deref()),
                )?)
            }
            "tools/call" => {
                self.require_initialized(session)?;
                let req: CallToolRequest = serde_json::from_value(params)
                    .map_err(|e| McpError::InvalidParams(e.to_string()))?;
                #[cfg(feature = "auth")]
                let result = {
                    let identity = session.identity.clone();
                    crate::server::auth_context::scope(identity, self.router.call_tool(req)).await?
                };
                #[cfg(not(feature = "auth"))]
                let result = self.router.call_tool(req).await?;
                Ok(serde_json::to_value(result)?)
            }
            "resources/list" => {
                self.require_initialized(session)?;
                let req: ListResourcesRequest = serde_json::from_value(params).unwrap_or_default();
                Ok(serde_json::to_value(
                    self.router.list_resources(req.cursor.as_deref()),
                )?)
            }
            "resources/read" => {
                self.require_initialized(session)?;
                let req: ReadResourceRequest = serde_json::from_value(params)
                    .map_err(|e| McpError::InvalidParams(e.to_string()))?;
                #[cfg(feature = "auth")]
                let result = {
                    let identity = session.identity.clone();
                    crate::server::auth_context::scope(identity, self.router.read_resource(req))
                        .await?
                };
                #[cfg(not(feature = "auth"))]
                let result = self.router.read_resource(req).await?;
                Ok(serde_json::to_value(result)?)
            }
            "resources/subscribe" => {
                self.require_initialized(session)?;
                let _req: SubscribeRequest = serde_json::from_value(params)
                    .map_err(|e| McpError::InvalidParams(e.to_string()))?;
                Ok(serde_json::json!({}))
            }
            "resources/unsubscribe" => {
                self.require_initialized(session)?;
                let _req: UnsubscribeRequest = serde_json::from_value(params)
                    .map_err(|e| McpError::InvalidParams(e.to_string()))?;
                Ok(serde_json::json!({}))
            }
            "prompts/list" => {
                self.require_initialized(session)?;
                let req: ListPromptsRequest = serde_json::from_value(params).unwrap_or_default();
                Ok(serde_json::to_value(
                    self.router.list_prompts(req.cursor.as_deref()),
                )?)
            }
            "prompts/get" => {
                self.require_initialized(session)?;
                let req: GetPromptRequest = serde_json::from_value(params)
                    .map_err(|e| McpError::InvalidParams(e.to_string()))?;
                #[cfg(feature = "auth")]
                let result = {
                    let identity = session.identity.clone();
                    crate::server::auth_context::scope(identity, self.router.get_prompt(req))
                        .await?
                };
                #[cfg(not(feature = "auth"))]
                let result = self.router.get_prompt(req).await?;
                Ok(serde_json::to_value(result)?)
            }
            "logging/setLevel" => {
                self.require_initialized(session)?;
                let _req: SetLevelRequest = serde_json::from_value(params)
                    .map_err(|e| McpError::InvalidParams(e.to_string()))?;
                Ok(serde_json::json!({}))
            }
            "completion/complete" => {
                self.require_initialized(session)?;
                let _req: CompleteRequest = serde_json::from_value(params)
                    .map_err(|e| McpError::InvalidParams(e.to_string()))?;
                Ok(serde_json::json!({ "completion": { "values": [], "hasMore": false } }))
            }
            method => Err(McpError::MethodNotFound(method.to_owned())),
        }
    }

    async fn handle_initialize(
        &self,
        req: InitializeRequest,
        session: &mut Session,
    ) -> McpResult<Value> {
        info!(
            client = %req.client_info.name,
            version = %req.client_info.version,
            "Client initializing"
        );

        session.client_info = Some(req.client_info);
        session.protocol_version = Some(req.protocol_version);
        session.initialized = true;

        let capabilities = ServerCapabilities {
            tools: if self.router.has_tools() {
                Some(ToolsCapability {
                    list_changed: Some(true),
                })
            } else {
                None
            },
            resources: if self.router.has_resources() {
                Some(ResourcesCapability {
                    subscribe: Some(false),
                    list_changed: Some(true),
                })
            } else {
                None
            },
            prompts: if self.router.has_prompts() {
                Some(PromptsCapability {
                    list_changed: Some(true),
                })
            } else {
                None
            },
            logging: Some(LoggingCapability {}),
            experimental: None,
        };

        let result = InitializeResult {
            protocol_version: MCP_PROTOCOL_VERSION.to_owned(),
            capabilities,
            server_info: self.info.clone(),
            instructions: self.instructions.clone(),
        };

        Ok(serde_json::to_value(result)?)
    }

    async fn handle_notification(&self, notif: JsonRpcNotification, session: &mut Session) {
        match notif.method.as_str() {
            "notifications/initialized" => {
                info!(session = %session.id, "Client sent initialized notification");
            }
            "notifications/cancelled" => {
                debug!("Client cancelled a request");
            }
            method => {
                warn!(method, "Received unknown notification");
            }
        }
    }

    fn require_initialized(&self, session: &Session) -> McpResult<()> {
        if !session.initialized {
            Err(McpError::InvalidRequest(
                "Server not initialized. Send 'initialize' first.".to_owned(),
            ))
        } else {
            Ok(())
        }
    }

    pub fn info(&self) -> &ServerInfo {
        &self.info
    }
}

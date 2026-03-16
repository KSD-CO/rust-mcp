//! # mcp-kit-gateway — MCP Gateway for proxying upstream servers
//!
//! This crate connects to one or more upstream MCP servers using
//! [`mcp_kit_client::McpClient`] and exposes their tools, resources, and prompts
//! through a local [`mcp_kit::McpServer`] router. Each upstream's capabilities
//! are namespaced with a configurable prefix to avoid collisions.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use mcp_kit::prelude::*;
//! use mcp_kit_gateway::{GatewayManager, UpstreamConfig, UpstreamTransport};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let mut gw = GatewayManager::new();
//!     gw.add_upstream(UpstreamConfig {
//!         name: "weather".into(),
//!         transport: UpstreamTransport::Sse("http://localhost:3001/sse".into()),
//!         prefix: Some("weather".into()),
//!         client_name: None,
//!         client_version: None,
//!     });
//!
//!     let server = gw.build_server(
//!         McpServer::builder()
//!             .name("gateway-server")
//!             .version("1.0.0")
//!     ).await?;
//!
//!     Ok(())
//! }
//! ```

use std::sync::Arc;

use mcp_kit_client::McpClient;
use tracing::{debug, error, info, warn};

use mcp_kit::error::{McpError, McpResult};
use mcp_kit::server::builder::{PromptDef, ResourceDef, ToolDef};
use mcp_kit::server::handler::{BoxFuture, PromptHandlerFn, ResourceHandlerFn, ToolHandlerFn};
use mcp_kit::types::messages::{CallToolRequest, GetPromptRequest, ReadResourceRequest};
use mcp_kit::types::prompt::{GetPromptResult, Prompt};
use mcp_kit::types::resource::{ReadResourceResult, Resource};
use mcp_kit::types::tool::Tool;

// Re-export key types from mcp-kit for convenience
pub use mcp_kit;
pub use mcp_kit_client;

// ─── Configuration ─────────────────────────────────────────────────────────────

/// Transport type for connecting to an upstream MCP server.
#[derive(Debug, Clone)]
pub enum UpstreamTransport {
    /// Connect via SSE (HTTP Server-Sent Events).
    #[cfg(feature = "sse")]
    Sse(String),

    /// Connect via WebSocket.
    #[cfg(feature = "websocket")]
    WebSocket(String),

    /// Connect via Streamable HTTP (MCP 2025-03-26).
    #[cfg(feature = "streamable-http")]
    StreamableHttp(String),

    /// Connect via stdio subprocess.
    #[cfg(feature = "stdio")]
    Stdio {
        /// Path to the program to spawn.
        program: String,
        /// Arguments to pass to the program.
        args: Vec<String>,
        /// Environment variables to set.
        env: Vec<(String, String)>,
    },
}

/// Configuration for a single upstream MCP server.
#[derive(Debug, Clone)]
pub struct UpstreamConfig {
    /// Unique name for this upstream (used as default prefix).
    pub name: String,
    /// How to connect to this upstream.
    pub transport: UpstreamTransport,
    /// Optional prefix for namespacing (defaults to `name`).
    /// Set to empty string to disable prefixing.
    pub prefix: Option<String>,
    /// Client name reported during initialization (defaults to "mcp-kit-gateway").
    pub client_name: Option<String>,
    /// Client version reported during initialization (defaults to crate version).
    pub client_version: Option<String>,
}

// ─── GatewayBackend ───────────────────────────────────────────────────────────

/// A connected upstream MCP server.
struct GatewayBackend {
    /// The upstream's name.
    name: String,
    /// Prefix for namespacing tools/prompts.
    prefix: String,
    /// The connected client (shared via Arc since McpClient is not Clone).
    client: Arc<McpClient>,
}

impl GatewayBackend {
    /// Connect to an upstream server and initialize the session.
    async fn connect(config: &UpstreamConfig) -> McpResult<Self> {
        let client = Self::create_client(&config.transport).await?;

        let client_name = config.client_name.as_deref().unwrap_or("mcp-kit-gateway");
        let client_version = config
            .client_version
            .as_deref()
            .unwrap_or(env!("CARGO_PKG_VERSION"));

        client
            .initialize(client_name, client_version)
            .await
            .map_err(|e| {
                McpError::InternalError(format!(
                    "Failed to initialize upstream '{}': {}",
                    config.name, e
                ))
            })?;

        let prefix = config.prefix.clone().unwrap_or_else(|| config.name.clone());

        info!(
            upstream = %config.name,
            prefix = %prefix,
            "Connected to upstream MCP server"
        );

        Ok(Self {
            name: config.name.clone(),
            prefix,
            client: Arc::new(client),
        })
    }

    /// Create an McpClient for the given transport.
    async fn create_client(transport: &UpstreamTransport) -> McpResult<McpClient> {
        match transport {
            #[cfg(feature = "sse")]
            UpstreamTransport::Sse(url) => McpClient::sse(url)
                .await
                .map_err(|e| McpError::InternalError(format!("SSE connection failed: {e}"))),

            #[cfg(feature = "websocket")]
            UpstreamTransport::WebSocket(url) => McpClient::websocket(url)
                .await
                .map_err(|e| McpError::InternalError(format!("WebSocket connection failed: {e}"))),

            #[cfg(feature = "streamable-http")]
            UpstreamTransport::StreamableHttp(url) => {
                McpClient::streamable_http(url).await.map_err(|e| {
                    McpError::InternalError(format!("Streamable HTTP connection failed: {e}"))
                })
            }

            #[cfg(feature = "stdio")]
            UpstreamTransport::Stdio { program, args, env } => {
                let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                let env_refs: Vec<(&str, &str)> =
                    env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                McpClient::stdio_with_args(program, &args_refs, &env_refs)
                    .await
                    .map_err(|e| McpError::InternalError(format!("Stdio connection failed: {e}")))
            }
        }
    }

    /// Compute the prefixed name for a tool/prompt.
    fn prefixed_name(&self, name: &str) -> String {
        if self.prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{}/{}", self.prefix, name)
        }
    }

    /// Discover tools from the upstream and create proxy handlers.
    async fn discover_tools(&self) -> McpResult<Vec<ToolDef>> {
        let tools = self.client.list_tools().await.map_err(|e| {
            McpError::InternalError(format!("Failed to list tools from '{}': {}", self.name, e))
        })?;

        debug!(
            upstream = %self.name,
            count = tools.len(),
            "Discovered upstream tools"
        );

        let mut defs = Vec::with_capacity(tools.len());
        for tool in tools {
            let prefixed = self.prefixed_name(&tool.name);
            let original_name = tool.name.clone();
            let client = Arc::clone(&self.client);

            // Build a new Tool with the prefixed name but same schema/description
            let proxied_tool = Tool {
                name: prefixed,
                description: tool.description.clone(),
                input_schema: tool.input_schema.clone(),
                annotations: tool.annotations.clone(),
            };

            // Create a handler that forwards calls to the upstream
            let handler: ToolHandlerFn = Arc::new(
                move |req: CallToolRequest| -> BoxFuture<'static, McpResult<_>> {
                    let client = Arc::clone(&client);
                    let name = original_name.clone();
                    Box::pin(async move {
                        client.call_tool(&name, req.arguments).await.map_err(|e| {
                            McpError::InternalError(format!("Upstream tool call failed: {e}"))
                        })
                    })
                },
            );

            defs.push(ToolDef::new(proxied_tool, handler));
        }

        Ok(defs)
    }

    /// Discover resources from the upstream and create proxy handlers.
    async fn discover_resources(&self) -> McpResult<Vec<ResourceDef>> {
        let resources = self.client.list_resources().await.map_err(|e| {
            McpError::InternalError(format!(
                "Failed to list resources from '{}': {}",
                self.name, e
            ))
        })?;

        debug!(
            upstream = %self.name,
            count = resources.len(),
            "Discovered upstream resources"
        );

        let mut defs = Vec::with_capacity(resources.len());
        for resource in resources {
            let original_uri = resource.uri.clone();
            let client = Arc::clone(&self.client);

            // Prefix the resource name but keep the original URI for routing
            let proxied_resource = Resource {
                uri: resource.uri.clone(),
                name: if self.prefix.is_empty() {
                    resource.name.clone()
                } else {
                    format!("{}/{}", self.prefix, resource.name)
                },
                description: resource.description.clone(),
                mime_type: resource.mime_type.clone(),
                size: resource.size,
                annotations: resource.annotations.clone(),
            };

            // Create a handler that forwards reads to the upstream
            let handler: ResourceHandlerFn = Arc::new(
                move |_req: ReadResourceRequest| -> BoxFuture<'static, McpResult<ReadResourceResult>> {
                    let client = Arc::clone(&client);
                    let uri = original_uri.clone();
                    Box::pin(async move {
                        client
                            .read_resource(&uri)
                            .await
                            .map_err(|e| McpError::InternalError(format!("Upstream resource read failed: {e}")))
                    })
                },
            );

            defs.push(ResourceDef::new_static(proxied_resource, handler));
        }

        Ok(defs)
    }

    /// Discover prompts from the upstream and create proxy handlers.
    async fn discover_prompts(&self) -> McpResult<Vec<PromptDef>> {
        let prompts = self.client.list_prompts().await.map_err(|e| {
            McpError::InternalError(format!(
                "Failed to list prompts from '{}': {}",
                self.name, e
            ))
        })?;

        debug!(
            upstream = %self.name,
            count = prompts.len(),
            "Discovered upstream prompts"
        );

        let mut defs = Vec::with_capacity(prompts.len());
        for prompt in prompts {
            let prefixed = self.prefixed_name(&prompt.name);
            let original_name = prompt.name.clone();
            let client = Arc::clone(&self.client);

            // Build a new Prompt with the prefixed name
            let proxied_prompt = Prompt {
                name: prefixed,
                description: prompt.description.clone(),
                arguments: prompt.arguments.clone(),
            };

            // Create a handler that forwards get_prompt to the upstream
            let handler: PromptHandlerFn = Arc::new(
                move |req: GetPromptRequest| -> BoxFuture<'static, McpResult<GetPromptResult>> {
                    let client = Arc::clone(&client);
                    let name = original_name.clone();
                    Box::pin(async move {
                        client
                            .get_prompt(&name, Some(req.arguments))
                            .await
                            .map_err(|e| {
                                McpError::InternalError(format!("Upstream prompt get failed: {e}"))
                            })
                    })
                },
            );

            defs.push(PromptDef::new(proxied_prompt, handler));
        }

        Ok(defs)
    }

    /// Close the upstream connection.
    async fn close(&self) {
        if let Err(e) = self.client.close().await {
            warn!(
                upstream = %self.name,
                error = %e,
                "Error closing upstream connection"
            );
        }
    }
}

// ─── GatewayManager ──────────────────────────────────────────────────────────

/// Manages connections to multiple upstream MCP servers.
///
/// Add upstream configurations, then use [`build_server`](Self::build_server) to
/// connect to upstreams, discover their capabilities, and merge them into an
/// [`McpServerBuilder`] to produce a fully-configured [`McpServer`].
///
/// # Example
///
/// ```rust,no_run
/// use mcp_kit::prelude::*;
/// use mcp_kit_gateway::{GatewayManager, UpstreamConfig, UpstreamTransport};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let mut gw = GatewayManager::new();
///     gw.add_upstream(UpstreamConfig {
///         name: "weather".into(),
///         transport: UpstreamTransport::Sse("http://localhost:3001/sse".into()),
///         prefix: Some("weather".into()),
///         client_name: None,
///         client_version: None,
///     });
///
///     let server = gw.build_server(
///         McpServer::builder()
///             .name("gateway")
///             .version("1.0.0")
///     ).await?;
///
///     Ok(())
/// }
/// ```
pub struct GatewayManager {
    configs: Vec<UpstreamConfig>,
    backends: Vec<GatewayBackend>,
}

impl GatewayManager {
    /// Create a new, empty gateway manager.
    pub fn new() -> Self {
        Self {
            configs: Vec::new(),
            backends: Vec::new(),
        }
    }

    /// Add an upstream server configuration.
    pub fn add_upstream(&mut self, config: UpstreamConfig) {
        self.configs.push(config);
    }

    /// Connect to all configured upstreams, discover their capabilities, and
    /// register the proxied tools/resources/prompts into the given builder.
    ///
    /// Returns a fully-built [`McpServer`] ready to serve.
    ///
    /// Upstreams that fail to connect are logged as warnings and skipped — the
    /// server will still start with the remaining upstreams.
    pub async fn build_server(
        &mut self,
        mut builder: mcp_kit::McpServerBuilder,
    ) -> McpResult<mcp_kit::McpServer> {
        let (tools, resources, prompts) = self.connect_and_discover().await?;

        for tool_def in tools {
            builder = builder.tool_def(tool_def);
        }

        for resource_def in resources {
            builder = builder.resource_def(resource_def);
        }

        for prompt_def in prompts {
            builder = builder.prompt_def(prompt_def);
        }

        Ok(builder.build())
    }

    /// Connect to all configured upstreams and discover their capabilities.
    ///
    /// Returns aggregated tool, resource, and prompt definitions ready to register
    /// into the server router. This is a lower-level API — prefer
    /// [`build_server`](Self::build_server) for most use cases.
    pub async fn connect_and_discover(
        &mut self,
    ) -> McpResult<(Vec<ToolDef>, Vec<ResourceDef>, Vec<PromptDef>)> {
        let mut all_tools = Vec::new();
        let mut all_resources = Vec::new();
        let mut all_prompts = Vec::new();

        let configs = std::mem::take(&mut self.configs);

        for config in &configs {
            match GatewayBackend::connect(config).await {
                Ok(backend) => {
                    // Discover tools
                    match backend.discover_tools().await {
                        Ok(tools) => {
                            info!(
                                upstream = %backend.name,
                                tools = tools.len(),
                                "Registered upstream tools"
                            );
                            all_tools.extend(tools);
                        }
                        Err(e) => {
                            warn!(
                                upstream = %backend.name,
                                error = %e,
                                "Failed to discover tools"
                            );
                        }
                    }

                    // Discover resources
                    match backend.discover_resources().await {
                        Ok(resources) => {
                            if !resources.is_empty() {
                                info!(
                                    upstream = %backend.name,
                                    resources = resources.len(),
                                    "Registered upstream resources"
                                );
                            }
                            all_resources.extend(resources);
                        }
                        Err(e) => {
                            warn!(
                                upstream = %backend.name,
                                error = %e,
                                "Failed to discover resources"
                            );
                        }
                    }

                    // Discover prompts
                    match backend.discover_prompts().await {
                        Ok(prompts) => {
                            if !prompts.is_empty() {
                                info!(
                                    upstream = %backend.name,
                                    prompts = prompts.len(),
                                    "Registered upstream prompts"
                                );
                            }
                            all_prompts.extend(prompts);
                        }
                        Err(e) => {
                            warn!(
                                upstream = %backend.name,
                                error = %e,
                                "Failed to discover prompts"
                            );
                        }
                    }

                    self.backends.push(backend);
                }
                Err(e) => {
                    error!(
                        upstream = %config.name,
                        error = %e,
                        "Failed to connect to upstream — skipping"
                    );
                }
            }
        }

        self.configs = configs;

        info!(
            upstreams = self.backends.len(),
            total_tools = all_tools.len(),
            total_resources = all_resources.len(),
            total_prompts = all_prompts.len(),
            "Gateway discovery complete"
        );

        Ok((all_tools, all_resources, all_prompts))
    }

    /// Close all upstream connections.
    pub async fn close_all(&self) {
        for backend in &self.backends {
            backend.close().await;
        }
    }

    /// Number of connected upstreams.
    pub fn connected_count(&self) -> usize {
        self.backends.len()
    }

    /// Number of configured (but not yet connected) upstreams.
    pub fn configured_count(&self) -> usize {
        self.configs.len()
    }
}

impl Default for GatewayManager {
    fn default() -> Self {
        Self::new()
    }
}

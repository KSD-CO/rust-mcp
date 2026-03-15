//! Plugin system for dynamically loading and managing MCP tools, resources, and prompts.
//!
//! The plugin system allows you to:
//! - Load tools/resources/prompts from external libraries
//! - Hot reload plugins during development
//! - Create a plugin ecosystem with shareable components
//! - Sandbox untrusted plugins with WASM
//!
//! # Example
//!
//! ```rust,no_run
//! use mcp_kit::prelude::*;
//! use mcp_kit::plugin::{McpPlugin, PluginManager};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let mut plugin_manager = PluginManager::new();
//!     
//!     // Load plugins
//!     plugin_manager.load_from_path("./plugins/weather.so")?;
//!     
//!     let server = McpServer::builder()
//!         .name("plugin-server")
//!         .version("1.0.0")
//!         .with_plugin_manager(plugin_manager)
//!         .build()
//!         .serve_stdio()
//!         .await?;
//!     
//!     Ok(())
//! }
//! ```

use crate::error::{McpError, McpResult};
use crate::types::prompt::Prompt;
use crate::types::resource::Resource;
use crate::types::tool::Tool;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "plugin-native")]
mod native;
#[cfg(feature = "plugin-wasm")]
mod wasm;

pub mod registry;

// ─── Plugin Trait ────────────────────────────────────────────────────────────

/// Core trait that all MCP plugins must implement.
///
/// A plugin can provide tools, resources, and/or prompts that will be
/// registered with the MCP server.
pub trait McpPlugin: Send + Sync {
    /// Plugin identifier (unique name)
    fn name(&self) -> &str;

    /// Plugin version (semver)
    fn version(&self) -> &str;

    /// Optional plugin description
    fn description(&self) -> Option<&str> {
        None
    }

    /// Plugin author/maintainer
    fn author(&self) -> Option<&str> {
        None
    }

    /// Minimum mcp-kit version required
    fn min_mcp_version(&self) -> Option<&str> {
        None
    }

    /// Register tools provided by this plugin
    fn register_tools(&self) -> Vec<ToolDefinition> {
        vec![]
    }

    /// Register resources provided by this plugin
    fn register_resources(&self) -> Vec<ResourceDefinition> {
        vec![]
    }

    /// Register prompts provided by this plugin
    fn register_prompts(&self) -> Vec<PromptDefinition> {
        vec![]
    }

    /// Called when plugin is loaded
    fn on_load(&mut self, _config: &PluginConfig) -> McpResult<()> {
        Ok(())
    }

    /// Called when plugin is unloaded
    fn on_unload(&mut self) -> McpResult<()> {
        Ok(())
    }

    /// Called to check if plugin can be safely unloaded
    fn can_unload(&self) -> bool {
        true
    }
}

// ─── Plugin Definitions ──────────────────────────────────────────────────────

// Type aliases for complex handler types
type ToolHandlerFn = Arc<
    dyn Fn(
            crate::types::messages::CallToolRequest,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = McpResult<crate::types::tool::CallToolResult>>
                    + Send
                    + 'static,
            >,
        > + Send
        + Sync
        + 'static,
>;

type ResourceHandlerFn = Arc<
    dyn Fn(
            crate::types::messages::ReadResourceRequest,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = McpResult<crate::types::resource::ReadResourceResult>,
                    > + Send
                    + 'static,
            >,
        > + Send
        + Sync
        + 'static,
>;

type PromptHandlerFn = Arc<
    dyn Fn(
            crate::types::messages::GetPromptRequest,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = McpResult<crate::types::prompt::GetPromptResult>>
                    + Send
                    + 'static,
            >,
        > + Send
        + Sync
        + 'static,
>;

/// Tool definition from a plugin
pub struct ToolDefinition {
    pub tool: Tool,
    pub handler: ToolHandlerFn,
}

impl ToolDefinition {
    /// Create a tool definition with a typed handler
    pub fn new<F, Fut, T>(tool: Tool, handler: F) -> Self
    where
        F: Fn(T) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = crate::types::tool::CallToolResult> + Send + 'static,
        T: serde::de::DeserializeOwned + Send + 'static,
    {
        let handler = Arc::new(move |req: crate::types::messages::CallToolRequest| {
            let f = handler.clone();
            let args = req.arguments.clone();
            Box::pin(async move {
                let params: T = serde_json::from_value(args)
                    .map_err(|e| McpError::InvalidParams(e.to_string()))?;
                Ok(f(params).await)
            })
                as std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                                Output = McpResult<crate::types::tool::CallToolResult>,
                            > + Send
                            + 'static,
                    >,
                >
        });

        Self { tool, handler }
    }

    /// Create a tool definition with a raw handler that takes CallToolRequest
    pub fn from_handler(tool: Tool, handler: ToolHandlerFn) -> Self {
        Self { tool, handler }
    }
}

/// Resource definition from a plugin
pub struct ResourceDefinition {
    pub resource: Resource,
    pub handler: ResourceHandlerFn,
}

impl ResourceDefinition {
    pub fn new<F, Fut>(resource: Resource, handler: F) -> Self
    where
        F: Fn(crate::types::messages::ReadResourceRequest) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = McpResult<crate::types::resource::ReadResourceResult>>
            + Send
            + 'static,
    {
        let handler = Arc::new(move |req| {
            let f = handler.clone();
            Box::pin(f(req))
                as std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                                Output = McpResult<crate::types::resource::ReadResourceResult>,
                            > + Send
                            + 'static,
                    >,
                >
        });

        Self { resource, handler }
    }
}

/// Prompt definition from a plugin
pub struct PromptDefinition {
    pub prompt: Prompt,
    pub handler: PromptHandlerFn,
}

impl PromptDefinition {
    pub fn new<F, Fut>(prompt: Prompt, handler: F) -> Self
    where
        F: Fn(crate::types::messages::GetPromptRequest) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = McpResult<crate::types::prompt::GetPromptResult>>
            + Send
            + 'static,
    {
        let handler = Arc::new(move |req| {
            let f = handler.clone();
            Box::pin(f(req))
                as std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                                Output = McpResult<crate::types::prompt::GetPromptResult>,
                            > + Send
                            + 'static,
                    >,
                >
        });

        Self { prompt, handler }
    }
}

// ─── Plugin Config ───────────────────────────────────────────────────────────

/// Configuration for a plugin instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin-specific configuration (JSON)
    #[serde(default)]
    pub config: serde_json::Value,

    /// Whether plugin is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Plugin priority (higher = loaded first)
    #[serde(default)]
    pub priority: i32,

    /// Plugin permissions
    #[serde(default)]
    pub permissions: PluginPermissions,
}

fn default_true() -> bool {
    true
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            config: serde_json::Value::Null,
            enabled: true,
            priority: 0,
            permissions: PluginPermissions::default(),
        }
    }
}

/// Plugin permissions/capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginPermissions {
    /// Can access network
    #[serde(default)]
    pub network: bool,

    /// Can access filesystem
    #[serde(default)]
    pub filesystem: bool,

    /// Can access environment variables
    #[serde(default)]
    pub env: bool,

    /// Can spawn processes
    #[serde(default)]
    pub process: bool,

    /// Custom permission flags
    #[serde(default)]
    pub custom: HashMap<String, bool>,
}

// ─── Plugin Metadata ─────────────────────────────────────────────────────────

/// Metadata about a loaded plugin
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub min_mcp_version: Option<String>,
    pub tool_count: usize,
    pub resource_count: usize,
    pub prompt_count: usize,
}

// ─── Plugin Manager ──────────────────────────────────────────────────────────

/// Manages plugin lifecycle and registration
pub struct PluginManager {
    plugins: HashMap<String, Box<dyn McpPlugin>>,
    configs: HashMap<String, PluginConfig>,
    #[cfg(feature = "plugin-hot-reload")]
    watcher: Option<notify::RecommendedWatcher>,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            configs: HashMap::new(),
            #[cfg(feature = "plugin-hot-reload")]
            watcher: None,
        }
    }

    /// Load a plugin from a dynamic library file (.so, .dylib, .dll)
    #[cfg(feature = "plugin-native")]
    pub fn load_from_path(&mut self, path: &str) -> McpResult<()> {
        self.load_from_path_with_config(path, PluginConfig::default())
    }

    /// Load a plugin with custom configuration
    #[cfg(feature = "plugin-native")]
    pub fn load_from_path_with_config(
        &mut self,
        path: &str,
        config: PluginConfig,
    ) -> McpResult<()> {
        if !config.enabled {
            tracing::debug!("Plugin at {} is disabled, skipping", path);
            return Ok(());
        }

        let mut plugin = native::load_plugin(path)?;
        plugin.on_load(&config)?;

        let name = plugin.name().to_string();
        tracing::info!("Loaded plugin: {} v{}", name, plugin.version());

        self.plugins.insert(name.clone(), plugin);
        self.configs.insert(name, config);

        Ok(())
    }

    /// Load a WASM plugin
    #[cfg(feature = "plugin-wasm")]
    pub fn load_wasm(&mut self, wasm_bytes: &[u8]) -> McpResult<()> {
        self.load_wasm_with_config(wasm_bytes, PluginConfig::default())
    }

    /// Load a WASM plugin with custom configuration
    #[cfg(feature = "plugin-wasm")]
    pub fn load_wasm_with_config(
        &mut self,
        wasm_bytes: &[u8],
        config: PluginConfig,
    ) -> McpResult<()> {
        if !config.enabled {
            return Ok(());
        }

        let mut plugin = wasm::load_plugin(wasm_bytes)?;
        plugin.on_load(&config)?;

        let name = plugin.name().to_string();
        tracing::info!("Loaded WASM plugin: {} v{}", name, plugin.version());

        self.plugins.insert(name.clone(), plugin);
        self.configs.insert(name, config);

        Ok(())
    }

    /// Register a plugin directly (for in-process plugins)
    pub fn register_plugin<P: McpPlugin + 'static>(
        &mut self,
        mut plugin: P,
        config: PluginConfig,
    ) -> McpResult<()> {
        if !config.enabled {
            return Ok(());
        }

        plugin.on_load(&config)?;

        let name = plugin.name().to_string();
        tracing::info!("Registered plugin: {} v{}", name, plugin.version());

        self.plugins.insert(name.clone(), Box::new(plugin));
        self.configs.insert(name, config);

        Ok(())
    }

    /// Unload a plugin by name
    pub fn unload(&mut self, name: &str) -> McpResult<()> {
        if let Some(mut plugin) = self.plugins.remove(name) {
            if !plugin.can_unload() {
                // Put it back if can't unload
                self.plugins.insert(name.to_string(), plugin);
                return Err(McpError::InvalidRequest(format!(
                    "Plugin {} cannot be unloaded",
                    name
                )));
            }

            plugin.on_unload()?;
            self.configs.remove(name);
            tracing::info!("Unloaded plugin: {}", name);
        }

        Ok(())
    }

    /// Get plugin metadata
    pub fn get_metadata(&self, name: &str) -> Option<PluginMetadata> {
        self.plugins.get(name).map(|plugin| {
            let tools = plugin.register_tools();
            let resources = plugin.register_resources();
            let prompts = plugin.register_prompts();

            PluginMetadata {
                name: plugin.name().to_string(),
                version: plugin.version().to_string(),
                description: plugin.description().map(String::from),
                author: plugin.author().map(String::from),
                min_mcp_version: plugin.min_mcp_version().map(String::from),
                tool_count: tools.len(),
                resource_count: resources.len(),
                prompt_count: prompts.len(),
            }
        })
    }

    /// List all loaded plugins
    pub fn list_plugins(&self) -> Vec<PluginMetadata> {
        self.plugins
            .keys()
            .filter_map(|name| self.get_metadata(name))
            .collect()
    }

    /// Get all tool definitions from all plugins
    pub(crate) fn collect_tools(&self) -> Vec<ToolDefinition> {
        let mut tools = Vec::new();

        // Sort plugins by priority
        let mut plugins: Vec<_> = self.plugins.iter().collect();
        plugins.sort_by_key(|(name, _)| {
            self.configs
                .get(*name)
                .map(|c| -c.priority) // Negative for descending order
                .unwrap_or(0)
        });

        for (_, plugin) in plugins {
            tools.extend(plugin.register_tools());
        }

        tools
    }

    /// Get all resource definitions from all plugins
    pub(crate) fn collect_resources(&self) -> Vec<ResourceDefinition> {
        let mut resources = Vec::new();

        let mut plugins: Vec<_> = self.plugins.iter().collect();
        plugins.sort_by_key(|(name, _)| self.configs.get(*name).map(|c| -c.priority).unwrap_or(0));

        for (_, plugin) in plugins {
            resources.extend(plugin.register_resources());
        }

        resources
    }

    /// Get all prompt definitions from all plugins
    pub(crate) fn collect_prompts(&self) -> Vec<PromptDefinition> {
        let mut prompts = Vec::new();

        let mut plugins: Vec<_> = self.plugins.iter().collect();
        plugins.sort_by_key(|(name, _)| self.configs.get(*name).map(|c| -c.priority).unwrap_or(0));

        for (_, plugin) in plugins {
            prompts.extend(plugin.register_prompts());
        }

        prompts
    }

    /// Enable hot reload (watch for plugin file changes)
    #[cfg(feature = "plugin-hot-reload")]
    pub fn enable_hot_reload(&mut self) -> McpResult<()> {
        use notify::{RecommendedWatcher, Watcher};

        let (tx, _rx) = std::sync::mpsc::channel();

        let watcher = RecommendedWatcher::new(tx, notify::Config::default())
            .map_err(|e| McpError::internal(format!("Failed to create watcher: {}", e)))?;

        // TODO: Watch plugin directories

        self.watcher = Some(watcher);

        tracing::info!("Hot reload enabled");
        Ok(())
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

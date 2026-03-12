use std::sync::Arc;

use crate::{
    types::{
        prompt::Prompt,
        resource::{Resource, ResourceTemplate},
        tool::Tool,
        ServerInfo,
    },
};

use crate::server::{
    handler::{PromptHandler, ResourceHandler, ToolHandler, ToolHandlerFn},
    router::Router,
    server::McpServer,
};

/// Builder for `McpServer` — the main entry point for configuring your server.
pub struct McpServerBuilder {
    name: String,
    version: String,
    instructions: Option<String>,
    router: Router,
}

impl McpServerBuilder {
    pub fn new() -> Self {
        Self {
            name: "mcp-server".to_owned(),
            version: "0.1.0".to_owned(),
            instructions: None,
            router: Router::new(),
        }
    }

    /// Set the server name (shown to clients during handshake)
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the server version
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Human-readable instructions for how to use this server
    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }

    // ─── Tool registration ────────────────────────────────────────────────────

    /// Register a tool with an explicit `Tool` descriptor and a handler function.
    pub fn tool<M>(mut self, tool: Tool, handler: impl ToolHandler<M>) -> Self {
        self.router.add_tool(tool, handler.into_handler_fn());
        self
    }

    /// Register a tool using a pre-built `ToolDef` (from the `#[tool]` macro).
    pub fn tool_def(mut self, def: ToolDef) -> Self {
        self.router.add_tool(def.tool, def.handler);
        self
    }

    /// Convenience: register a no-parameter tool.
    pub fn tool_fn<M>(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        handler: impl ToolHandler<M>,
    ) -> Self {
        self.router.add_tool(Tool::no_params(name, description), handler.into_handler_fn());
        self
    }

    // ─── Resource registration ────────────────────────────────────────────────

    /// Register a static resource (exact URI match).
    pub fn resource<M>(mut self, resource: Resource, handler: impl ResourceHandler<M>) -> Self {
        self.router.add_resource(resource, handler.into_handler_fn());
        self
    }

    /// Register a URI-template resource (e.g. `"file://{path}"`).
    pub fn resource_template<M>(
        mut self,
        template: ResourceTemplate,
        handler: impl ResourceHandler<M>,
    ) -> Self {
        self.router.add_resource_template(template, handler.into_handler_fn());
        self
    }

    // ─── Prompt registration ──────────────────────────────────────────────────

    /// Register a prompt template.
    pub fn prompt<M>(mut self, prompt: Prompt, handler: impl PromptHandler<M>) -> Self {
        self.router.add_prompt(prompt, handler.into_handler_fn());
        self
    }

    // ─── Build ────────────────────────────────────────────────────────────────

    pub fn build(self) -> McpServer {
        McpServer {
            info: ServerInfo::new(self.name, self.version),
            instructions: self.instructions,
            router: Arc::new(self.router),
        }
    }
}

impl Default for McpServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ─── ToolDef ─────────────────────────────────────────────────────────────────

/// A fully-described tool produced by the `#[tool]` proc macro.
pub struct ToolDef {
    pub tool: Tool,
    pub handler: ToolHandlerFn,
}

impl ToolDef {
    pub fn new(tool: Tool, handler: ToolHandlerFn) -> Self {
        Self { tool, handler }
    }
}

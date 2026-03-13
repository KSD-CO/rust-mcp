use std::sync::Arc;

use crate::types::{
    prompt::Prompt,
    resource::{Resource, ResourceTemplate},
    tool::Tool,
    ServerInfo,
};

use crate::server::{
    core::McpServer,
    handler::{
        CompletionHandler, PromptHandler, PromptHandlerFn, ResourceHandler, ResourceHandlerFn,
        ToolHandler, ToolHandlerFn,
    },
    router::Router,
};

#[cfg(feature = "auth")]
use crate::auth::DynAuthProvider;

/// Builder for `McpServer` — the main entry point for configuring your server.
pub struct McpServerBuilder {
    name: String,
    version: String,
    instructions: Option<String>,
    router: Router,
    #[cfg(feature = "auth")]
    auth_provider: Option<DynAuthProvider>,
    #[cfg(feature = "auth")]
    require_auth: bool,
}

impl McpServerBuilder {
    pub fn new() -> Self {
        Self {
            name: "mcp-server".to_owned(),
            version: "0.1.0".to_owned(),
            instructions: None,
            router: Router::new(),
            #[cfg(feature = "auth")]
            auth_provider: None,
            #[cfg(feature = "auth")]
            require_auth: true,
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

    // ─── Auth configuration ───────────────────────────────────────────────────

    /// Require authentication on all requests using the given provider.
    ///
    /// Requests with no or invalid credentials receive HTTP 401 on SSE/HTTP
    /// transports. Stdio transport is unaffected (it relies on process-level
    /// access control).
    ///
    /// # Example
    /// ```rust,no_run
    /// use mcp_kit::prelude::*;
    /// use mcp_kit::auth::BearerTokenProvider;
    /// use std::sync::Arc;
    ///
    /// McpServer::builder()
    ///     .name("my-server")
    ///     .version("1.0")
    ///     .auth(Arc::new(BearerTokenProvider::new(["secret"])))
    ///     .build();
    /// ```
    #[cfg(feature = "auth")]
    pub fn auth(mut self, provider: DynAuthProvider) -> Self {
        self.auth_provider = Some(provider);
        self.require_auth = true;
        self
    }

    /// Accept an auth provider but allow unauthenticated requests through.
    ///
    /// Authenticated requests have an identity available via `Auth`; unauthenticated
    /// requests have no identity and may reach handlers with `None`.
    #[cfg(feature = "auth")]
    pub fn optional_auth(mut self, provider: DynAuthProvider) -> Self {
        self.auth_provider = Some(provider);
        self.require_auth = false;
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
        self.router.add_tool(
            Tool::no_params(name, description),
            handler.into_handler_fn(),
        );
        self
    }

    // ─── Resource registration ────────────────────────────────────────────────

    /// Register a static resource (exact URI match).
    pub fn resource<M>(mut self, resource: Resource, handler: impl ResourceHandler<M>) -> Self {
        self.router
            .add_resource(resource, handler.into_handler_fn());
        self
    }

    /// Register a URI-template resource (e.g. `"file://{path}"`).
    pub fn resource_template<M>(
        mut self,
        template: ResourceTemplate,
        handler: impl ResourceHandler<M>,
    ) -> Self {
        self.router
            .add_resource_template(template, handler.into_handler_fn());
        self
    }

    /// Register a resource using a pre-built `ResourceDef` (from the `#[resource]` macro).
    pub fn resource_def(mut self, def: ResourceDef) -> Self {
        match def {
            ResourceDef::Static { resource, handler } => {
                self.router.add_resource(resource, handler);
            }
            ResourceDef::Template { template, handler } => {
                self.router.add_resource_template(template, handler);
            }
        }
        self
    }

    // ─── Prompt registration ──────────────────────────────────────────────────

    /// Register a prompt template.
    pub fn prompt<M>(mut self, prompt: Prompt, handler: impl PromptHandler<M>) -> Self {
        self.router.add_prompt(prompt, handler.into_handler_fn());
        self
    }

    /// Register a prompt using a pre-built `PromptDef` (from the `#[prompt]` macro).
    pub fn prompt_def(mut self, def: PromptDef) -> Self {
        self.router.add_prompt(def.prompt, def.handler);
        self
    }

    // ─── Completion registration ──────────────────────────────────────────────

    /// Register a global completion handler for auto-completing prompt/resource arguments.
    ///
    /// This handler is called for any `completion/complete` request that doesn't have
    /// a more specific handler (prompt-specific or resource-specific).
    ///
    /// # Example
    /// ```rust,no_run
    /// use mcp_kit::prelude::*;
    /// use mcp_kit::types::messages::{CompleteRequest, CompleteResult};
    ///
    /// McpServer::builder()
    ///     .name("my-server")
    ///     .completion(|req: CompleteRequest| async move {
    ///         // Auto-complete based on argument name
    ///         let values = match req.argument.name.as_str() {
    ///             "language" => vec!["rust", "python", "javascript"],
    ///             _ => vec![],
    ///         };
    ///         Ok(CompleteResult::new(values))
    ///     })
    ///     .build();
    /// ```
    pub fn completion<M>(mut self, handler: impl CompletionHandler<M>) -> Self {
        self.router.set_completion_handler(handler.into_handler_fn());
        self
    }

    /// Register a completion handler for a specific resource URI pattern.
    ///
    /// The pattern can be an exact URI or a template like `"file://{path}"`.
    pub fn resource_completion<M>(
        mut self,
        uri_pattern: impl Into<String>,
        handler: impl CompletionHandler<M>,
    ) -> Self {
        self.router
            .add_resource_completion(uri_pattern.into(), handler.into_handler_fn());
        self
    }

    /// Register a prompt with an associated completion handler.
    ///
    /// The completion handler provides auto-complete suggestions for the prompt's arguments.
    pub fn prompt_with_completion<M1, M2>(
        mut self,
        prompt: Prompt,
        handler: impl PromptHandler<M1>,
        completion: impl CompletionHandler<M2>,
    ) -> Self {
        self.router.add_prompt_with_completion(
            prompt,
            handler.into_handler_fn(),
            completion.into_handler_fn(),
        );
        self
    }

    // ─── Build ────────────────────────────────────────────────────────────────

    pub fn build(self) -> McpServer {
        McpServer {
            info: ServerInfo::new(self.name, self.version),
            instructions: self.instructions,
            router: Arc::new(self.router),
            #[cfg(feature = "auth")]
            auth_provider: self.auth_provider,
            #[cfg(feature = "auth")]
            require_auth: self.require_auth,
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

// ─── ResourceDef ─────────────────────────────────────────────────────────────

/// A fully-described resource produced by the `#[resource]` proc macro.
pub enum ResourceDef {
    Static {
        resource: Resource,
        handler: ResourceHandlerFn,
    },
    Template {
        template: ResourceTemplate,
        handler: ResourceHandlerFn,
    },
}

impl ResourceDef {
    pub fn new_static(resource: Resource, handler: ResourceHandlerFn) -> Self {
        Self::Static { resource, handler }
    }

    pub fn new_template(template: ResourceTemplate, handler: ResourceHandlerFn) -> Self {
        Self::Template { template, handler }
    }
}

// ─── PromptDef ───────────────────────────────────────────────────────────────

/// A fully-described prompt produced by the `#[prompt]` proc macro.
pub struct PromptDef {
    pub prompt: Prompt,
    pub handler: PromptHandlerFn,
}

impl PromptDef {
    pub fn new(prompt: Prompt, handler: PromptHandlerFn) -> Self {
        Self { prompt, handler }
    }
}

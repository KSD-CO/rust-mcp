//! Core MCP server implementation for Cloudflare Workers.
//!
//! This module provides a stateless MCP server optimized for serverless environments.

use std::{collections::HashMap, pin::Pin, rc::Rc};

use futures::Future;
use mcp_kit::{
    error::{McpError, McpResult},
    protocol::{
        JsonRpcError, JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, MCP_PROTOCOL_VERSION,
    },
    types::{
        messages::{
            CallToolRequest, CompleteRequest, CompleteResult, GetPromptRequest, InitializeRequest,
            InitializeResult, ReadResourceRequest,
        },
        prompt::{GetPromptResult, Prompt},
        resource::{ReadResourceResult, Resource, ResourceTemplate},
        tool::{CallToolResult, Tool},
        LoggingCapability, PromptsCapability, ResourcesCapability, ServerCapabilities, ServerInfo,
        ToolsCapability,
    },
};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{completion, prompts, resources, tools};

// ─── Type Aliases ─────────────────────────────────────────────────────────────

/// Non-Send future for WASM compatibility.
pub type LocalFuture<T> = Pin<Box<dyn Future<Output = T>>>;

/// Tool handler function type.
pub type ToolFn = Rc<dyn Fn(Value) -> LocalFuture<McpResult<CallToolResult>>>;

/// Resource handler function type.
pub type ResourceFn = Rc<dyn Fn(ReadResourceRequest) -> LocalFuture<McpResult<ReadResourceResult>>>;

/// Prompt handler function type.
pub type PromptFn = Rc<dyn Fn(GetPromptRequest) -> LocalFuture<McpResult<GetPromptResult>>>;

/// Completion handler function type.
pub type CompletionFn = Rc<dyn Fn(CompleteRequest) -> LocalFuture<McpResult<CompleteResult>>>;

// ─── Registry Entries ─────────────────────────────────────────────────────────

pub struct ToolEntry {
    pub tool: Tool,
    pub handler: ToolFn,
}

pub struct ResourceEntry {
    pub resource: Resource,
    pub handler: ResourceFn,
}

pub struct ResourceTemplateEntry {
    pub template: ResourceTemplate,
    pub handler: ResourceFn,
}

pub struct PromptEntry {
    pub prompt: Prompt,
    pub handler: PromptFn,
    pub completion: Option<CompletionFn>,
}

// ─── CloudflareServer ─────────────────────────────────────────────────────────

/// Stateless MCP server for Cloudflare Workers.
///
/// Each request is self-contained. Session state is not persisted across
/// requests, which works well for the common "initialize → call tools" pattern.
pub struct CloudflareServer {
    info: ServerInfo,
    instructions: Option<String>,
    tools: HashMap<String, ToolEntry>,
    resources: HashMap<String, ResourceEntry>,
    resource_templates: Vec<ResourceTemplateEntry>,
    prompts: HashMap<String, PromptEntry>,
    global_completion: Option<CompletionFn>,
}

impl CloudflareServer {
    pub fn builder() -> CloudflareServerBuilder {
        CloudflareServerBuilder::default()
    }

    /// Get server info.
    pub fn info(&self) -> &ServerInfo {
        &self.info
    }

    /// Get capabilities summary for discovery endpoint.
    pub fn capabilities_summary(&self) -> Value {
        serde_json::json!({
            "tools": !self.tools.is_empty(),
            "resources": !self.resources.is_empty() || !self.resource_templates.is_empty(),
            "prompts": !self.prompts.is_empty(),
            "completion": self.global_completion.is_some() || self.prompts.values().any(|p| p.completion.is_some())
        })
    }

    /// Handle an incoming JSON-RPC message.
    pub async fn handle(&self, msg: JsonRpcMessage) -> Option<JsonRpcMessage> {
        match msg {
            JsonRpcMessage::Request(req) => {
                let id = req.id.clone();
                match self.dispatch(req).await {
                    Ok(result) => Some(JsonRpcMessage::Response(JsonRpcResponse {
                        jsonrpc: "2.0".into(),
                        id,
                        result,
                    })),
                    Err(e) => Some(JsonRpcMessage::Error(JsonRpcError::new(id, e))),
                }
            }
            JsonRpcMessage::Notification(_) => None,
            _ => None,
        }
    }

    async fn dispatch(&self, req: JsonRpcRequest) -> McpResult<Value> {
        let params = req.params.unwrap_or(Value::Null);

        match req.method.as_str() {
            // ── Lifecycle ─────────────────────────────────────────────────────
            "initialize" => self.handle_initialize(params),
            "ping" => Ok(serde_json::json!({})),

            // ── Tools ─────────────────────────────────────────────────────────
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(params).await,

            // ── Resources ─────────────────────────────────────────────────────
            "resources/list" => self.handle_resources_list(),
            "resources/templates/list" => self.handle_templates_list(),
            "resources/read" => self.handle_resources_read(params).await,

            // ── Prompts ───────────────────────────────────────────────────────
            "prompts/list" => self.handle_prompts_list(),
            "prompts/get" => self.handle_prompts_get(params).await,

            // ── Completion ────────────────────────────────────────────────────
            "completion/complete" => self.handle_completion(params).await,

            method => Err(McpError::MethodNotFound(method.to_owned())),
        }
    }

    // ── Lifecycle Handlers ────────────────────────────────────────────────────

    fn handle_initialize(&self, params: Value) -> McpResult<Value> {
        let _init: InitializeRequest =
            serde_json::from_value(params).map_err(|e| McpError::InvalidParams(e.to_string()))?;

        let result = InitializeResult {
            protocol_version: MCP_PROTOCOL_VERSION.to_owned(),
            capabilities: ServerCapabilities {
                tools: if self.tools.is_empty() {
                    None
                } else {
                    Some(ToolsCapability {
                        list_changed: Some(false),
                    })
                },
                resources: if self.resources.is_empty() && self.resource_templates.is_empty() {
                    None
                } else {
                    Some(ResourcesCapability {
                        subscribe: Some(false),
                        list_changed: Some(false),
                    })
                },
                prompts: if self.prompts.is_empty() {
                    None
                } else {
                    Some(PromptsCapability {
                        list_changed: Some(false),
                    })
                },
                logging: Some(LoggingCapability {}),
                experimental: None,
            },
            server_info: self.info.clone(),
            instructions: self.instructions.clone(),
        };
        Ok(serde_json::to_value(result)?)
    }

    // ── Tool Handlers ─────────────────────────────────────────────────────────

    fn handle_tools_list(&self) -> McpResult<Value> {
        let tools: Vec<&Tool> = self.tools.values().map(|e| &e.tool).collect();
        Ok(serde_json::json!({ "tools": tools }))
    }

    async fn handle_tools_call(&self, params: Value) -> McpResult<Value> {
        let req: CallToolRequest =
            serde_json::from_value(params).map_err(|e| McpError::InvalidParams(e.to_string()))?;

        let entry = self
            .tools
            .get(&req.name)
            .ok_or_else(|| McpError::ToolNotFound(req.name.clone()))?;

        let result = (entry.handler)(req.arguments).await?;
        Ok(serde_json::to_value(result)?)
    }

    // ── Resource Handlers ─────────────────────────────────────────────────────

    fn handle_resources_list(&self) -> McpResult<Value> {
        let resources: Vec<&Resource> = self.resources.values().map(|e| &e.resource).collect();
        Ok(serde_json::json!({ "resources": resources }))
    }

    fn handle_templates_list(&self) -> McpResult<Value> {
        let templates: Vec<&ResourceTemplate> =
            self.resource_templates.iter().map(|e| &e.template).collect();
        Ok(serde_json::json!({ "resourceTemplates": templates }))
    }

    async fn handle_resources_read(&self, params: Value) -> McpResult<Value> {
        let req: ReadResourceRequest =
            serde_json::from_value(params).map_err(|e| McpError::InvalidParams(e.to_string()))?;

        // Exact URI match first
        if let Some(entry) = self.resources.get(&req.uri) {
            let result = (entry.handler)(req).await?;
            return Ok(serde_json::to_value(result)?);
        }

        // Template match
        for entry in &self.resource_templates {
            if uri_matches_template(&req.uri, &entry.template.uri_template) {
                let result = (entry.handler)(req).await?;
                return Ok(serde_json::to_value(result)?);
            }
        }

        Err(McpError::ResourceNotFound(req.uri))
    }

    // ── Prompt Handlers ───────────────────────────────────────────────────────

    fn handle_prompts_list(&self) -> McpResult<Value> {
        let prompts: Vec<&Prompt> = self.prompts.values().map(|e| &e.prompt).collect();
        Ok(serde_json::json!({ "prompts": prompts }))
    }

    async fn handle_prompts_get(&self, params: Value) -> McpResult<Value> {
        let req: GetPromptRequest =
            serde_json::from_value(params).map_err(|e| McpError::InvalidParams(e.to_string()))?;

        let entry = self
            .prompts
            .get(&req.name)
            .ok_or_else(|| McpError::PromptNotFound(req.name.clone()))?;

        let result = (entry.handler)(req).await?;
        Ok(serde_json::to_value(result)?)
    }

    // ── Completion Handler ────────────────────────────────────────────────────

    async fn handle_completion(&self, params: Value) -> McpResult<Value> {
        let req: CompleteRequest =
            serde_json::from_value(params).map_err(|e| McpError::InvalidParams(e.to_string()))?;

        // Check prompt-specific completion first
        if let mcp_kit::types::messages::CompletionReference::Prompt { name } = &req.reference {
            if let Some(entry) = self.prompts.get(name) {
                if let Some(ref completion) = entry.completion {
                    let result = completion(req).await?;
                    return Ok(serde_json::to_value(result)?);
                }
            }
        }

        // Fall back to global completion
        if let Some(ref completion) = self.global_completion {
            let result = completion(req).await?;
            return Ok(serde_json::to_value(result)?);
        }

        // Default: empty completion
        Ok(serde_json::json!({ "completion": { "values": [], "hasMore": false } }))
    }
}

// ─── Builder ─────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct CloudflareServerBuilder {
    name: String,
    version: String,
    instructions: Option<String>,
    tools: HashMap<String, ToolEntry>,
    resources: HashMap<String, ResourceEntry>,
    resource_templates: Vec<ResourceTemplateEntry>,
    prompts: HashMap<String, PromptEntry>,
    global_completion: Option<CompletionFn>,
}

impl CloudflareServerBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }

    // ── Tools ─────────────────────────────────────────────────────────────────

    /// Register a tool with typed parameters (auto-deserialized from JSON).
    pub fn tool<T, F, Fut>(mut self, tool: Tool, handler: F) -> Self
    where
        T: DeserializeOwned + 'static,
        F: Fn(T) -> Fut + 'static,
        Fut: Future<Output = McpResult<CallToolResult>> + 'static,
    {
        let f = Rc::new(move |args: Value| -> LocalFuture<McpResult<CallToolResult>> {
            let params: T = match serde_json::from_value(args) {
                Ok(p) => p,
                Err(e) => {
                    return Box::pin(async move { Err(McpError::InvalidParams(e.to_string())) })
                }
            };
            Box::pin(handler(params))
        });
        self.tools
            .insert(tool.name.clone(), ToolEntry { tool, handler: f });
        self
    }

    /// Register a tool with raw JSON handler.
    pub fn tool_raw<F, Fut>(mut self, tool: Tool, handler: F) -> Self
    where
        F: Fn(Value) -> Fut + 'static,
        Fut: Future<Output = McpResult<CallToolResult>> + 'static,
    {
        let f = Rc::new(move |args: Value| -> LocalFuture<McpResult<CallToolResult>> {
            Box::pin(handler(args))
        });
        self.tools
            .insert(tool.name.clone(), ToolEntry { tool, handler: f });
        self
    }

    // ── Resources ─────────────────────────────────────────────────────────────

    /// Register a static resource.
    pub fn resource<F, Fut>(mut self, resource: Resource, handler: F) -> Self
    where
        F: Fn(ReadResourceRequest) -> Fut + 'static,
        Fut: Future<Output = McpResult<ReadResourceResult>> + 'static,
    {
        let f = Rc::new(
            move |req: ReadResourceRequest| -> LocalFuture<McpResult<ReadResourceResult>> {
                Box::pin(handler(req))
            },
        );
        self.resources.insert(
            resource.uri.clone(),
            ResourceEntry {
                resource,
                handler: f,
            },
        );
        self
    }

    /// Register a resource template (URI with variables like `user://{id}`).
    pub fn resource_template<F, Fut>(mut self, template: ResourceTemplate, handler: F) -> Self
    where
        F: Fn(ReadResourceRequest) -> Fut + 'static,
        Fut: Future<Output = McpResult<ReadResourceResult>> + 'static,
    {
        let f = Rc::new(
            move |req: ReadResourceRequest| -> LocalFuture<McpResult<ReadResourceResult>> {
                Box::pin(handler(req))
            },
        );
        self.resource_templates
            .push(ResourceTemplateEntry { template, handler: f });
        self
    }

    // ── Prompts ───────────────────────────────────────────────────────────────

    /// Register a prompt template.
    pub fn prompt<F, Fut>(mut self, prompt: Prompt, handler: F) -> Self
    where
        F: Fn(GetPromptRequest) -> Fut + 'static,
        Fut: Future<Output = McpResult<GetPromptResult>> + 'static,
    {
        let f = Rc::new(
            move |req: GetPromptRequest| -> LocalFuture<McpResult<GetPromptResult>> {
                Box::pin(handler(req))
            },
        );
        self.prompts.insert(
            prompt.name.clone(),
            PromptEntry {
                prompt,
                handler: f,
                completion: None,
            },
        );
        self
    }

    /// Register a prompt with completion handler.
    pub fn prompt_with_completion<F1, Fut1, F2, Fut2>(
        mut self,
        prompt: Prompt,
        handler: F1,
        completion: F2,
    ) -> Self
    where
        F1: Fn(GetPromptRequest) -> Fut1 + 'static,
        Fut1: Future<Output = McpResult<GetPromptResult>> + 'static,
        F2: Fn(CompleteRequest) -> Fut2 + 'static,
        Fut2: Future<Output = McpResult<CompleteResult>> + 'static,
    {
        let handler_fn = Rc::new(
            move |req: GetPromptRequest| -> LocalFuture<McpResult<GetPromptResult>> {
                Box::pin(handler(req))
            },
        );
        let completion_fn = Rc::new(
            move |req: CompleteRequest| -> LocalFuture<McpResult<CompleteResult>> {
                Box::pin(completion(req))
            },
        );
        self.prompts.insert(
            prompt.name.clone(),
            PromptEntry {
                prompt,
                handler: handler_fn,
                completion: Some(completion_fn),
            },
        );
        self
    }

    // ── Completion ────────────────────────────────────────────────────────────

    /// Register a global completion handler.
    pub fn completion<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(CompleteRequest) -> Fut + 'static,
        Fut: Future<Output = McpResult<CompleteResult>> + 'static,
    {
        self.global_completion = Some(Rc::new(
            move |req: CompleteRequest| -> LocalFuture<McpResult<CompleteResult>> {
                Box::pin(handler(req))
            },
        ));
        self
    }

    pub fn build(self) -> CloudflareServer {
        CloudflareServer {
            info: ServerInfo::new(
                if self.name.is_empty() {
                    "mcp-cloudflare"
                } else {
                    &self.name
                },
                if self.version.is_empty() {
                    "1.0.0"
                } else {
                    &self.version
                },
            ),
            instructions: self.instructions,
            tools: self.tools,
            resources: self.resources,
            resource_templates: self.resource_templates,
            prompts: self.prompts,
            global_completion: self.global_completion,
        }
    }
}

// ─── Thread-local server instance ─────────────────────────────────────────────

thread_local! {
    static SERVER: Rc<CloudflareServer> = Rc::new(build_server());
}

pub fn get_server() -> Rc<CloudflareServer> {
    SERVER.with(Rc::clone)
}

fn build_server() -> CloudflareServer {
    CloudflareServer::builder()
        .name("mcp-cloudflare-demo")
        .version("1.0.0")
        .instructions(include_str!("instructions.md"))
        // Register all tools
        .tool(tools::calculator::add_tool(), tools::calculator::add_handler)
        .tool(tools::calculator::subtract_tool(), tools::calculator::subtract_handler)
        .tool(tools::calculator::multiply_tool(), tools::calculator::multiply_handler)
        .tool(tools::calculator::divide_tool(), tools::calculator::divide_handler)
        .tool(tools::calculator::sqrt_tool(), tools::calculator::sqrt_handler)
        .tool(tools::text::uppercase_tool(), tools::text::uppercase_handler)
        .tool(tools::text::lowercase_tool(), tools::text::lowercase_handler)
        .tool(tools::text::reverse_tool(), tools::text::reverse_handler)
        .tool(tools::text::word_count_tool(), tools::text::word_count_handler)
        .tool(tools::text::echo_tool(), tools::text::echo_handler)
        // Register resources
        .resource(resources::config::app_config_resource(), resources::config::app_config_handler)
        .resource(resources::config::server_info_resource(), resources::config::server_info_handler)
        .resource(resources::config::readme_resource(), resources::config::readme_handler)
        // Register resource templates
        .resource_template(resources::templates::user_template(), resources::templates::user_handler)
        .resource_template(resources::templates::document_template(), resources::templates::document_handler)
        // Register prompts with completion
        .prompt_with_completion(
            prompts::code_review_prompt(),
            prompts::code_review_handler,
            prompts::code_review_completion,
        )
        .prompt(prompts::summarize_prompt(), prompts::summarize_handler)
        .prompt(prompts::translate_prompt(), prompts::translate_handler)
        // Global completion fallback
        .completion(completion::global_completion_handler)
        .build()
}

// ─── URI Template Matching ────────────────────────────────────────────────────

/// Simple URI template matcher — supports `{variable}` placeholders.
pub fn uri_matches_template(uri: &str, template: &str) -> bool {
    let mut uri_chars = uri.chars().peekable();
    let mut tpl_chars = template.chars().peekable();

    while let Some(&tc) = tpl_chars.peek() {
        if tc == '{' {
            // Skip variable name until '}'
            while tpl_chars.next().map(|c| c != '}').unwrap_or(false) {}
            // Consume non-'/' characters in uri
            if uri_chars.peek().is_none() {
                return false;
            }
            while uri_chars.peek().map(|&c| c != '/').unwrap_or(false) {
                uri_chars.next();
            }
        } else {
            tpl_chars.next();
            if uri_chars.next() != Some(tc) {
                return false;
            }
        }
    }

    uri_chars.peek().is_none()
}

/// Extract a variable from a URI given a template.
/// e.g., extract_uri_var("user://123", "user://{id}") -> Some("123")
pub fn extract_uri_var(uri: &str, template: &str) -> Option<String> {
    let var_start = template.find('{')?;
    let var_end = template.find('}')?;

    let prefix = &template[..var_start];
    let suffix = &template[var_end + 1..];

    if !uri.starts_with(prefix) {
        return None;
    }

    let remaining = &uri[prefix.len()..];

    if suffix.is_empty() {
        Some(remaining.to_string())
    } else if let Some(end_pos) = remaining.find(suffix) {
        Some(remaining[..end_pos].to_string())
    } else {
        None
    }
}

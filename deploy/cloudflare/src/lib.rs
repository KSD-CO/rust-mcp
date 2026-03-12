//! MCP server on Cloudflare Workers — Streamable HTTP transport.
//!
//! Transport: `POST /mcp` accepts a JSON-RPC message and returns a JSON response.
//!            `GET  /mcp` returns server metadata.
//!
//! This is stateless — every request is self-contained.  Session initialization
//! state is not persisted across requests, which is fine for the common pattern
//! of "initialize once, then call tools."
//!
//! # Build & deploy
//!
//!   cargo install worker-build
//!   wrangler deploy
//!
//! # Test locally
//!
//!   wrangler dev
//!   curl -X POST http://localhost:8787/mcp \
//!     -H "Content-Type: application/json" \
//!     -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'

use std::{collections::HashMap, pin::Pin, rc::Rc};

use futures::Future;
use rust_mcp::{
    error::{McpError, McpResult},
    protocol::{
        JsonRpcError, JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
        MCP_PROTOCOL_VERSION,
    },
    types::{
        content::Content,
        messages::{
            CallToolRequest, GetPromptRequest, InitializeRequest, InitializeResult,
            ReadResourceRequest,
        },
        prompt::{GetPromptResult, Prompt, PromptArgument},
        resource::{ReadResourceResult, Resource, ResourceTemplate},
        tool::{CallToolResult, Tool},
        Implementation, LoggingCapability, PromptsCapability, ResourcesCapability,
        ServerCapabilities, ServerInfo, ToolsCapability,
    },
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use worker::*;

// ─── Future type alias ────────────────────────────────────────────────────────

type LocalFuture<T> = Pin<Box<dyn Future<Output = T>>>;

// ─── Handler types (single-threaded WASM — no Send/Sync required) ─────────────

type ToolFn =
    Rc<dyn Fn(serde_json::Value) -> LocalFuture<McpResult<CallToolResult>>>;

type ResourceFn =
    Rc<dyn Fn(ReadResourceRequest) -> LocalFuture<McpResult<ReadResourceResult>>>;

type PromptFn =
    Rc<dyn Fn(GetPromptRequest) -> LocalFuture<McpResult<GetPromptResult>>>;

// ─── Registry entries ─────────────────────────────────────────────────────────

struct ToolEntry {
    tool: Tool,
    handler: ToolFn,
}

struct ResourceEntry {
    resource: Resource,
    handler: ResourceFn,
}

struct ResourceTemplateEntry {
    template: ResourceTemplate,
    handler: ResourceFn,
}

struct PromptEntry {
    prompt: Prompt,
    handler: PromptFn,
}

// ─── CloudflareServer ─────────────────────────────────────────────────────────

/// Stateless MCP server for Cloudflare Workers.
pub struct CloudflareServer {
    info: ServerInfo,
    instructions: Option<String>,
    tools: HashMap<String, ToolEntry>,
    resources: HashMap<String, ResourceEntry>,
    resource_templates: Vec<ResourceTemplateEntry>,
    prompts: HashMap<String, PromptEntry>,
}

impl CloudflareServer {
    pub fn builder() -> CloudflareServerBuilder {
        CloudflareServerBuilder::default()
    }

    async fn handle(&self, msg: JsonRpcMessage) -> Option<JsonRpcMessage> {
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
            // Notifications don't expect a response
            JsonRpcMessage::Notification(_) => None,
            _ => None,
        }
    }

    async fn dispatch(&self, req: JsonRpcRequest) -> McpResult<Value> {
        let params = req.params.unwrap_or(Value::Null);

        match req.method.as_str() {
            // ── Lifecycle ─────────────────────────────────────────────────────
            "initialize" => {
                let _init: InitializeRequest = serde_json::from_value(params)
                    .map_err(|e| McpError::InvalidParams(e.to_string()))?;

                let result = InitializeResult {
                    protocol_version: MCP_PROTOCOL_VERSION.to_owned(),
                    capabilities: ServerCapabilities {
                        tools: if !self.tools.is_empty() {
                            Some(ToolsCapability { list_changed: Some(false) })
                        } else {
                            None
                        },
                        resources: if !self.resources.is_empty()
                            || !self.resource_templates.is_empty()
                        {
                            Some(ResourcesCapability {
                                subscribe: Some(false),
                                list_changed: Some(false),
                            })
                        } else {
                            None
                        },
                        prompts: if !self.prompts.is_empty() {
                            Some(PromptsCapability { list_changed: Some(false) })
                        } else {
                            None
                        },
                        logging: Some(LoggingCapability {}),
                        experimental: None,
                    },
                    server_info: self.info.clone(),
                    instructions: self.instructions.clone(),
                };
                Ok(serde_json::to_value(result)?)
            }

            "ping" => Ok(serde_json::json!({})),

            // ── Tools ─────────────────────────────────────────────────────────
            "tools/list" => {
                let tools: Vec<&Tool> = self.tools.values().map(|e| &e.tool).collect();
                Ok(serde_json::json!({ "tools": tools }))
            }

            "tools/call" => {
                let req: CallToolRequest = serde_json::from_value(params)
                    .map_err(|e| McpError::InvalidParams(e.to_string()))?;
                let entry = self
                    .tools
                    .get(&req.name)
                    .ok_or_else(|| McpError::ToolNotFound(req.name.clone()))?;
                let result = (entry.handler)(req.arguments).await?;
                Ok(serde_json::to_value(result)?)
            }

            // ── Resources ─────────────────────────────────────────────────────
            "resources/list" => {
                let resources: Vec<&Resource> =
                    self.resources.values().map(|e| &e.resource).collect();
                Ok(serde_json::json!({ "resources": resources }))
            }

            "resources/read" => {
                let req: ReadResourceRequest = serde_json::from_value(params)
                    .map_err(|e| McpError::InvalidParams(e.to_string()))?;

                // Exact URI match
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

            // ── Prompts ───────────────────────────────────────────────────────
            "prompts/list" => {
                let prompts: Vec<&Prompt> = self.prompts.values().map(|e| &e.prompt).collect();
                Ok(serde_json::json!({ "prompts": prompts }))
            }

            "prompts/get" => {
                let req: GetPromptRequest = serde_json::from_value(params)
                    .map_err(|e| McpError::InvalidParams(e.to_string()))?;
                let entry = self
                    .prompts
                    .get(&req.name)
                    .ok_or_else(|| McpError::PromptNotFound(req.name.clone()))?;
                let result = (entry.handler)(req).await?;
                Ok(serde_json::to_value(result)?)
            }

            method => Err(McpError::MethodNotFound(method.to_owned())),
        }
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

    /// Register a tool with a raw JSON handler.
    pub fn tool_raw<F, Fut>(mut self, tool: Tool, handler: F) -> Self
    where
        F: Fn(Value) -> Fut + 'static,
        Fut: Future<Output = McpResult<CallToolResult>> + 'static,
    {
        let f = Rc::new(move |args: Value| -> LocalFuture<McpResult<CallToolResult>> {
            Box::pin(handler(args))
        });
        self.tools.insert(tool.name.clone(), ToolEntry { tool, handler: f });
        self
    }

    /// Register a tool with a typed handler — parameters are automatically
    /// deserialized from the arguments object.
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
                    return Box::pin(async move {
                        Err(McpError::InvalidParams(e.to_string()))
                    })
                }
            };
            Box::pin(handler(params))
        });
        self.tools.insert(tool.name.clone(), ToolEntry { tool, handler: f });
        self
    }

    // ── Resources ─────────────────────────────────────────────────────────────

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
        self.resources.insert(resource.uri.clone(), ResourceEntry { resource, handler: f });
        self
    }

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
        self.resource_templates.push(ResourceTemplateEntry { template, handler: f });
        self
    }

    // ── Prompts ───────────────────────────────────────────────────────────────

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
        self.prompts.insert(prompt.name.clone(), PromptEntry { prompt, handler: f });
        self
    }

    pub fn build(self) -> CloudflareServer {
        CloudflareServer {
            info: ServerInfo::new(
                if self.name.is_empty() { "mcp-server" } else { &self.name },
                if self.version.is_empty() { "0.1.0" } else { &self.version },
            ),
            instructions: self.instructions,
            tools: self.tools,
            resources: self.resources,
            resource_templates: self.resource_templates,
            prompts: self.prompts,
        }
    }
}

// ─── Thread-local server (initialised once per isolate) ──────────────────────

thread_local! {
    static SERVER: Rc<CloudflareServer> = Rc::new(build_server());
}

fn get_server() -> Rc<CloudflareServer> {
    SERVER.with(Rc::clone)
}

/// Define your tools, resources, and prompts here.
fn build_server() -> CloudflareServer {
    use rust_mcp::types::content::Content;
    use serde::Deserialize;

    // ── Example: Calculator ───────────────────────────────────────────────────

    #[derive(Deserialize)]
    struct BinaryInput {
        a: f64,
        b: f64,
    }

    #[derive(Deserialize)]
    struct SqrtInput {
        n: f64,
    }

    let binary_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "a": { "type": "number", "description": "First operand" },
            "b": { "type": "number", "description": "Second operand" }
        },
        "required": ["a", "b"]
    });

    CloudflareServerBuilder::default()
        .name("calculator")
        .version("1.0.0")
        .instructions("A calculator server running on Cloudflare Workers.")
        .tool(
            Tool::new("add", "Add two numbers", binary_schema.clone()),
            |p: BinaryInput| async move { Ok(CallToolResult::text(format!("{}", p.a + p.b))) },
        )
        .tool(
            Tool::new("subtract", "Subtract b from a", binary_schema.clone()),
            |p: BinaryInput| async move { Ok(CallToolResult::text(format!("{}", p.a - p.b))) },
        )
        .tool(
            Tool::new("multiply", "Multiply two numbers", binary_schema.clone()),
            |p: BinaryInput| async move { Ok(CallToolResult::text(format!("{}", p.a * p.b))) },
        )
        .tool(
            Tool::new("divide", "Divide a by b", binary_schema),
            |p: BinaryInput| async move {
                if p.b == 0.0 {
                    return Ok(CallToolResult::error("Division by zero"));
                }
                Ok(CallToolResult::text(format!("{}", p.a / p.b)))
            },
        )
        .tool(
            Tool::new(
                "sqrt",
                "Square root of n",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "n": { "type": "number", "description": "Non-negative number" }
                    },
                    "required": ["n"]
                }),
            ),
            |p: SqrtInput| async move {
                if p.n < 0.0 {
                    return Ok(CallToolResult::error("n must be non-negative"));
                }
                Ok(CallToolResult::text(format!("{}", p.n.sqrt())))
            },
        )
        .build()
}

// ─── Cloudflare Workers fetch handler ────────────────────────────────────────

#[event(fetch)]
pub async fn main(mut req: Request, _env: Env, _ctx: Context) -> Result<Response> {
    let method = req.method();
    let path = req.path();

    // ── CORS preflight ────────────────────────────────────────────────────────
    if method == Method::Options {
        return Ok(cors_headers(Response::empty()?));
    }

    match (method, path.as_str()) {
        // ── POST /mcp — main JSON-RPC endpoint ────────────────────────────────
        (Method::Post, "/mcp") => {
            let msg: JsonRpcMessage = req
                .json()
                .await
                .map_err(|e| Error::RustError(format!("Invalid JSON: {e}")))?;

            let server = get_server();
            let response = server.handle(msg).await;

            match response {
                Some(resp) => {
                    let body = serde_json::to_string(&resp)
                        .map_err(|e| Error::RustError(e.to_string()))?;
                    let mut headers = Headers::new();
                    headers.set("content-type", "application/json")?;
                    headers.set("access-control-allow-origin", "*")?;
                    Ok(Response::ok(body)?.with_headers(headers))
                }
                // Notification — no response body
                None => {
                    let mut headers = Headers::new();
                    headers.set("access-control-allow-origin", "*")?;
                    Ok(Response::empty()?.with_status(202).with_headers(headers))
                }
            }
        }

        // ── GET /mcp — server info & discovery ───────────────────────────────
        (Method::Get, "/mcp") => {
            let server = get_server();
            let body = serde_json::json!({
                "name":        server.info.name,
                "version":     server.info.version,
                "transport":   "streamable-http",
                "endpoint":    "/mcp",
            });
            let mut headers = Headers::new();
            headers.set("content-type", "application/json")?;
            headers.set("access-control-allow-origin", "*")?;
            Ok(Response::from_json(&body)?.with_headers(headers))
        }

        _ => Response::error("Not Found", 404),
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn cors_headers(mut resp: Response) -> Response {
    let mut h = Headers::new();
    let _ = h.set("access-control-allow-origin", "*");
    let _ = h.set("access-control-allow-methods", "GET, POST, OPTIONS");
    let _ = h.set("access-control-allow-headers", "content-type");
    resp.with_headers(h)
}

/// Simple URI template matcher — supports {variable} placeholders.
fn uri_matches_template(uri: &str, template: &str) -> bool {
    let mut uri_chars = uri.chars().peekable();
    let mut tpl_chars = template.chars().peekable();

    while let Some(&tc) = tpl_chars.peek() {
        if tc == '{' {
            // Skip variable name
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

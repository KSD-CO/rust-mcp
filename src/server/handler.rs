use std::{future::Future, pin::Pin, sync::Arc};

use crate::{
    error::{McpError, McpResult},
    types::{
        messages::{CallToolRequest, GetPromptRequest, ReadResourceRequest},
        prompt::GetPromptResult,
        resource::ReadResourceResult,
        tool::CallToolResult,
    },
};
use serde::de::DeserializeOwned;

/// A type-erased, boxed future
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Boxed handler function for tool calls
pub type HandlerFn<Req, Res> =
    Arc<dyn Fn(Req) -> BoxFuture<'static, McpResult<Res>> + Send + Sync + 'static>;

pub type ToolHandlerFn = HandlerFn<CallToolRequest, CallToolResult>;
pub type ResourceHandlerFn = HandlerFn<ReadResourceRequest, ReadResourceResult>;
pub type PromptHandlerFn = HandlerFn<GetPromptRequest, GetPromptResult>;

// ─── IntoToolResult ───────────────────────────────────────────────────────────

/// Anything that can be returned from a tool handler
pub trait IntoToolResult {
    fn into_tool_result(self) -> CallToolResult;
}

impl IntoToolResult for CallToolResult {
    fn into_tool_result(self) -> CallToolResult {
        self
    }
}

impl IntoToolResult for String {
    fn into_tool_result(self) -> CallToolResult {
        CallToolResult::text(self)
    }
}

impl IntoToolResult for &str {
    fn into_tool_result(self) -> CallToolResult {
        CallToolResult::text(self)
    }
}

impl IntoToolResult for serde_json::Value {
    fn into_tool_result(self) -> CallToolResult {
        CallToolResult::text(self.to_string())
    }
}

impl<T: IntoToolResult, E: std::fmt::Display> IntoToolResult for Result<T, E> {
    fn into_tool_result(self) -> CallToolResult {
        match self {
            Ok(v) => v.into_tool_result(),
            Err(e) => CallToolResult::error(e.to_string()),
        }
    }
}

// ─── ToolHandler trait ────────────────────────────────────────────────────────

/// Implemented for async functions that can serve as tool handlers.
///
/// Supports two calling conventions:
///   1. `|args: serde_json::Value| async { ... }` – raw JSON args
///   2. `|params: MyStruct| async { ... }` – typed, deserialized args
pub trait ToolHandler<Marker>: Clone + Send + Sync + 'static {
    fn into_handler_fn(self) -> ToolHandlerFn;
}

/// Marker for typed (deserialised) handlers.
/// Works for both `|params: MyStruct|` and `|args: serde_json::Value|` since
/// `Value` implements `DeserializeOwned`.
pub struct TypedMarker<T>(std::marker::PhantomData<T>);

impl<F, Fut, R, T> ToolHandler<TypedMarker<T>> for F
where
    F: Fn(T) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: IntoToolResult + Send + 'static,
    T: DeserializeOwned + Send + 'static,
{
    fn into_handler_fn(self) -> ToolHandlerFn {
        Arc::new(move |req: CallToolRequest| {
            let f = self.clone();
            let args = req.arguments.clone();
            Box::pin(async move {
                let params: T = serde_json::from_value(args)
                    .map_err(|e| McpError::InvalidParams(e.to_string()))?;
                Ok(f(params).await.into_tool_result())
            })
        })
    }
}

// ─── PromptHandler ───────────────────────────────────────────────────────────

pub trait PromptHandler<Marker>: Clone + Send + Sync + 'static {
    fn into_handler_fn(self) -> PromptHandlerFn;
}

pub struct PromptRawMarker;

impl<F, Fut> PromptHandler<PromptRawMarker> for F
where
    F: Fn(GetPromptRequest) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = McpResult<GetPromptResult>> + Send + 'static,
{
    fn into_handler_fn(self) -> PromptHandlerFn {
        Arc::new(move |req| {
            let f = self.clone();
            Box::pin(async move { f(req).await })
        })
    }
}

// ─── ResourceHandler ─────────────────────────────────────────────────────────

pub trait ResourceHandler<Marker>: Clone + Send + Sync + 'static {
    fn into_handler_fn(self) -> ResourceHandlerFn;
}

pub struct ResourceRawMarker;

impl<F, Fut> ResourceHandler<ResourceRawMarker> for F
where
    F: Fn(ReadResourceRequest) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = McpResult<ReadResourceResult>> + Send + 'static,
{
    fn into_handler_fn(self) -> ResourceHandlerFn {
        Arc::new(move |req| {
            let f = self.clone();
            Box::pin(async move { f(req).await })
        })
    }
}

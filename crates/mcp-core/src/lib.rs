pub mod error;
pub mod protocol;
pub mod types;

pub use error::{McpError, McpResult};
pub use protocol::{JsonRpcError, JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, RequestId};
pub use types::{
    // Content
    content::{
        Annotations, AudioContent, Content, EmbeddedResource, ImageContent, ResourceContents,
        TextContent,
    },
    // Messages
    messages::*,
    // Prompt
    prompt::{GetPromptResult, Prompt, PromptArgument, PromptMessage, PromptMessageRole},
    // Resource
    resource::{ListResourceTemplatesResult, Resource, ResourceTemplate},
    // Sampling
    sampling::{
        CreateMessageRequest, CreateMessageResult, MessageParam, ModelHint, ModelPreferences,
        SamplingMessage,
    },
    // Tool
    tool::{CallToolResult, Tool, ToolAnnotations},
    // Core
    CallToolError, ClientCapabilities, ClientInfo, Implementation, LoggingLevel,
    ServerCapabilities, ServerInfo,
};

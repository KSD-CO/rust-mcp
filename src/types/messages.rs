/// All MCP request/response/notification message types
use serde::{Deserialize, Serialize};

use crate::types::{
    prompt::Prompt, resource::Resource, tool::Tool, ClientCapabilities, ClientInfo, Cursor,
    LoggingLevel, Root, ServerCapabilities, ServerInfo,
};

// ─── Initialize ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequest {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: ClientInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

// ─── Tools ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListToolsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Cursor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsResult {
    pub tools: Vec<Tool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<Cursor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolRequest {
    pub name: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

// ─── Resources ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListResourcesRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Cursor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesResult {
    pub resources: Vec<Resource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<Cursor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadResourceRequest {
    pub uri: String,
}

pub use crate::types::resource::ReadResourceResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeRequest {
    pub uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsubscribeRequest {
    pub uri: String,
}

// ─── Prompts ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListPromptsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Cursor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsResult {
    pub prompts: Vec<Prompt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<Cursor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPromptRequest {
    pub name: String,
    #[serde(default)]
    pub arguments: std::collections::HashMap<String, String>,
}

pub use crate::types::prompt::GetPromptResult;

// ─── Completion ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteRequest {
    #[serde(rename = "ref")]
    pub reference: CompletionReference,
    pub argument: CompletionArgument,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CompletionReference {
    #[serde(rename = "ref/prompt")]
    Prompt { name: String },
    #[serde(rename = "ref/resource")]
    Resource { uri: String },
}

impl CompletionReference {
    /// Get the name/uri of the reference
    pub fn name(&self) -> &str {
        match self {
            Self::Prompt { name } => name,
            Self::Resource { uri } => uri,
        }
    }

    /// Check if this is a prompt reference
    pub fn is_prompt(&self) -> bool {
        matches!(self, Self::Prompt { .. })
    }

    /// Check if this is a resource reference
    pub fn is_resource(&self) -> bool {
        matches!(self, Self::Resource { .. })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionArgument {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteResult {
    pub completion: CompletionValues,
}

impl CompleteResult {
    /// Create a new completion result with the given values
    pub fn new(values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let values: Vec<String> = values.into_iter().map(Into::into).collect();
        Self {
            completion: CompletionValues {
                total: Some(values.len() as u32),
                values,
                has_more: Some(false),
            },
        }
    }

    /// Create an empty completion result
    pub fn empty() -> Self {
        Self {
            completion: CompletionValues {
                values: vec![],
                total: Some(0),
                has_more: Some(false),
            },
        }
    }

    /// Create a completion result with pagination info
    pub fn with_pagination(values: Vec<String>, total: u32, has_more: bool) -> Self {
        Self {
            completion: CompletionValues {
                values,
                total: Some(total),
                has_more: Some(has_more),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionValues {
    pub values: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_more: Option<bool>,
}

// ─── Logging ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetLevelRequest {
    pub level: LoggingLevel,
}

// ─── Roots ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListRootsResult {
    pub roots: Vec<Root>,
}

// ─── Notifications ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressNotification {
    pub progress_token: crate::protocol::ProgressToken,
    pub progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelledNotification {
    pub request_id: crate::protocol::RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMessageNotification {
    pub level: LoggingLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logger: Option<String>,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUpdatedNotification {
    pub uri: String,
}

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type McpResult<T> = Result<T, McpError>;

/// MCP protocol error codes (JSON-RPC 2.0 + MCP extensions)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // JSON-RPC 2.0 standard codes
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,

    // MCP-specific codes
    ConnectionClosed = -32000,
    RequestTimeout = -32001,
    ResourceNotFound = -32002,
    ToolNotFound = -32003,
    PromptNotFound = -32004,
    Unauthorized = -32005,
}

impl ErrorCode {
    pub fn code(self) -> i64 {
        self as i64
    }
}

/// Main error type for the MCP library
#[derive(Debug, Error)]
pub enum McpError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Method not found: {0}")]
    MethodNotFound(String),

    #[error("Invalid params: {0}")]
    InvalidParams(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Prompt not found: {0}")]
    PromptNotFound(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Request timeout")]
    Timeout,

    #[error("{0}")]
    Other(String),
}

impl McpError {
    pub fn code(&self) -> i64 {
        match self {
            Self::ParseError(_) => ErrorCode::ParseError.code(),
            Self::InvalidRequest(_) => ErrorCode::InvalidRequest.code(),
            Self::MethodNotFound(_) => ErrorCode::MethodNotFound.code(),
            Self::InvalidParams(_) => ErrorCode::InvalidParams.code(),
            Self::ResourceNotFound(_) => ErrorCode::ResourceNotFound.code(),
            Self::ToolNotFound(_) => ErrorCode::ToolNotFound.code(),
            Self::PromptNotFound(_) => ErrorCode::PromptNotFound.code(),
            Self::Unauthorized(_) => ErrorCode::Unauthorized.code(),
            Self::ConnectionClosed => ErrorCode::ConnectionClosed.code(),
            Self::Timeout => ErrorCode::RequestTimeout.code(),
            _ => ErrorCode::InternalError.code(),
        }
    }

    pub fn message(&self) -> String {
        self.to_string()
    }

    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self::InvalidParams(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::InternalError(msg.into())
    }
}

/// JSON-serializable error data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorData {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl From<&McpError> for ErrorData {
    fn from(err: &McpError) -> Self {
        Self {
            code: err.code(),
            message: err.message(),
            data: None,
        }
    }
}

impl From<McpError> for ErrorData {
    fn from(err: McpError) -> Self {
        Self::from(&err)
    }
}

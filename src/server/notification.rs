//! Notification sender for pushing updates from server to client.
//!
//! MCP servers can send notifications to clients to inform them of state changes:
//! - `notifications/resources/updated` - A resource's content has changed
//! - `notifications/resources/list_changed` - The list of available resources changed
//! - `notifications/tools/list_changed` - The list of available tools changed
//! - `notifications/prompts/list_changed` - The list of available prompts changed
//! - `notifications/message` - Log message
//! - `notifications/progress` - Progress update for long-running operations

use crate::{
    protocol::{JsonRpcNotification, ProgressToken},
    types::{
        messages::{LogMessageNotification, ProgressNotification, ResourceUpdatedNotification},
        LoggingLevel,
    },
};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::mpsc;

/// A sender for pushing notifications to the client.
///
/// This is a cloneable handle that can be passed to handlers and background tasks.
/// Notifications are sent through a channel and delivered to the client via the transport.
///
/// # Example
/// ```rust,no_run
/// use mcp_kit::server::NotificationSender;
///
/// async fn background_task(notifier: NotificationSender) {
///     // Notify client that a resource changed
///     notifier.resource_updated("file:///data.json").await;
///     
///     // Send a log message
///     notifier.log_info("data", "File updated successfully").await;
/// }
/// ```
#[derive(Clone)]
pub struct NotificationSender {
    tx: mpsc::Sender<JsonRpcNotification>,
}

impl NotificationSender {
    /// Create a new notification sender with the given channel.
    pub fn new(tx: mpsc::Sender<JsonRpcNotification>) -> Self {
        Self { tx }
    }

    /// Create a sender/receiver pair with the specified buffer size.
    pub fn channel(buffer: usize) -> (Self, NotificationReceiver) {
        let (tx, rx) = mpsc::channel(buffer);
        (Self { tx }, NotificationReceiver { rx })
    }

    /// Send a raw notification.
    pub async fn send(&self, notification: JsonRpcNotification) -> Result<(), SendError> {
        self.tx
            .send(notification)
            .await
            .map_err(|_| SendError::ChannelClosed)
    }

    /// Send a notification with the given method and params.
    pub async fn notify<T: Serialize>(&self, method: &str, params: T) -> Result<(), SendError> {
        let params = serde_json::to_value(params).map_err(SendError::Serialize)?;
        self.send(JsonRpcNotification::new(method, Some(params)))
            .await
    }

    // ─── Resource notifications ───────────────────────────────────────────────

    /// Notify the client that a resource's content has changed.
    ///
    /// The client may re-read the resource if it has subscribed to it.
    pub async fn resource_updated(&self, uri: impl Into<String>) -> Result<(), SendError> {
        self.notify(
            "notifications/resources/updated",
            ResourceUpdatedNotification { uri: uri.into() },
        )
        .await
    }

    /// Notify the client that the list of available resources has changed.
    ///
    /// The client should re-fetch the resource list with `resources/list`.
    pub async fn resources_list_changed(&self) -> Result<(), SendError> {
        self.send(JsonRpcNotification::new(
            "notifications/resources/list_changed",
            None,
        ))
        .await
    }

    // ─── Tool notifications ───────────────────────────────────────────────────

    /// Notify the client that the list of available tools has changed.
    ///
    /// The client should re-fetch the tool list with `tools/list`.
    pub async fn tools_list_changed(&self) -> Result<(), SendError> {
        self.send(JsonRpcNotification::new(
            "notifications/tools/list_changed",
            None,
        ))
        .await
    }

    // ─── Prompt notifications ─────────────────────────────────────────────────

    /// Notify the client that the list of available prompts has changed.
    ///
    /// The client should re-fetch the prompt list with `prompts/list`.
    pub async fn prompts_list_changed(&self) -> Result<(), SendError> {
        self.send(JsonRpcNotification::new(
            "notifications/prompts/list_changed",
            None,
        ))
        .await
    }

    // ─── Logging notifications ────────────────────────────────────────────────

    /// Send a log message notification to the client.
    pub async fn log(
        &self,
        level: LoggingLevel,
        logger: Option<String>,
        data: impl Serialize,
    ) -> Result<(), SendError> {
        let data = serde_json::to_value(data).map_err(SendError::Serialize)?;
        self.notify(
            "notifications/message",
            LogMessageNotification {
                level,
                logger,
                data,
            },
        )
        .await
    }

    /// Send a debug log message.
    pub async fn log_debug(
        &self,
        logger: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<(), SendError> {
        self.log(LoggingLevel::Debug, Some(logger.into()), message.into())
            .await
    }

    /// Send an info log message.
    pub async fn log_info(
        &self,
        logger: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<(), SendError> {
        self.log(LoggingLevel::Info, Some(logger.into()), message.into())
            .await
    }

    /// Send a warning log message.
    pub async fn log_warning(
        &self,
        logger: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<(), SendError> {
        self.log(LoggingLevel::Warning, Some(logger.into()), message.into())
            .await
    }

    /// Send an error log message.
    pub async fn log_error(
        &self,
        logger: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<(), SendError> {
        self.log(LoggingLevel::Error, Some(logger.into()), message.into())
            .await
    }

    // ─── Progress notifications ───────────────────────────────────────────────

    /// Send a progress update for a long-running operation.
    ///
    /// The `progress_token` should match the token provided in the original request.
    /// Progress values are typically 0.0 to 1.0, or absolute values when `total` is provided.
    pub async fn progress(
        &self,
        progress_token: impl Into<ProgressToken>,
        progress: f64,
        total: Option<f64>,
        message: Option<String>,
    ) -> Result<(), SendError> {
        self.notify(
            "notifications/progress",
            ProgressNotification {
                progress_token: progress_token.into(),
                progress,
                total,
                message,
            },
        )
        .await
    }

    /// Send a progress update with a message.
    pub async fn progress_with_message(
        &self,
        progress_token: impl Into<ProgressToken>,
        progress: f64,
        total: f64,
        message: impl Into<String>,
    ) -> Result<(), SendError> {
        self.progress(progress_token, progress, Some(total), Some(message.into()))
            .await
    }
}

/// Receiver end for notifications.
pub struct NotificationReceiver {
    rx: mpsc::Receiver<JsonRpcNotification>,
}

impl NotificationReceiver {
    /// Receive the next notification.
    pub async fn recv(&mut self) -> Option<JsonRpcNotification> {
        self.rx.recv().await
    }

    /// Try to receive a notification without blocking.
    pub fn try_recv(&mut self) -> Result<JsonRpcNotification, mpsc::error::TryRecvError> {
        self.rx.try_recv()
    }
}

/// Error type for notification sending.
#[derive(Debug, thiserror::Error)]
pub enum SendError {
    #[error("Notification channel closed")]
    ChannelClosed,
    #[error("Failed to serialize notification: {0}")]
    Serialize(serde_json::Error),
}

// ─── Convenience types ────────────────────────────────────────────────────────

/// A shared notification sender wrapped in Arc for easy sharing across handlers.
pub type SharedNotificationSender = Arc<NotificationSender>;

impl From<i64> for crate::protocol::ProgressToken {
    fn from(n: i64) -> Self {
        crate::protocol::ProgressToken::Number(n)
    }
}

impl From<String> for crate::protocol::ProgressToken {
    fn from(s: String) -> Self {
        crate::protocol::ProgressToken::String(s)
    }
}

impl From<&str> for crate::protocol::ProgressToken {
    fn from(s: &str) -> Self {
        crate::protocol::ProgressToken::String(s.to_owned())
    }
}

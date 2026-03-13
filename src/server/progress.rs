//! Progress tracking for long-running operations.
//!
//! When a client sends a request with a `_meta.progressToken`, the server can
//! send progress notifications back to the client during the operation.
//!
//! # Example
//! ```rust,no_run
//! use mcp_kit::server::progress::ProgressTracker;
//! use mcp_kit::server::NotificationSender;
//! use mcp_kit::protocol::ProgressToken;
//!
//! async fn long_operation(
//!     notifier: NotificationSender,
//!     progress_token: Option<ProgressToken>,
//! ) {
//!     let tracker = ProgressTracker::new(notifier, progress_token);
//!     
//!     for i in 0..100 {
//!         // Do some work...
//!         tracker.update(i as f64, 100.0, Some(format!("Processing item {}", i))).await;
//!     }
//!     
//!     tracker.complete("Done!").await;
//! }
//! ```

use crate::protocol::ProgressToken;
use crate::server::NotificationSender;

/// A helper for sending progress updates for a long-running operation.
///
/// This wraps a `NotificationSender` and an optional progress token, making it
/// easy to report progress without checking if the token is present each time.
pub struct ProgressTracker {
    notifier: NotificationSender,
    token: Option<ProgressToken>,
}

impl ProgressTracker {
    /// Create a new progress tracker.
    ///
    /// If `token` is `None`, all progress updates will be no-ops.
    pub fn new(notifier: NotificationSender, token: Option<ProgressToken>) -> Self {
        Self { notifier, token }
    }

    /// Create a tracker from a request's `_meta.progressToken`.
    ///
    /// Extracts the progress token from the `_meta` field of a request params object.
    pub fn from_meta(notifier: NotificationSender, meta: Option<&serde_json::Value>) -> Self {
        let token = meta
            .and_then(|m| m.get("progressToken"))
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(ProgressToken::String(s.to_owned()))
                } else {
                    v.as_i64().map(ProgressToken::Number)
                }
            });
        Self::new(notifier, token)
    }

    /// Send a progress update.
    ///
    /// - `progress`: Current progress value
    /// - `total`: Total value (progress/total gives percentage)
    /// - `message`: Optional status message
    ///
    /// Returns silently if no progress token was provided.
    pub async fn update(&self, progress: f64, total: f64, message: Option<String>) {
        if let Some(ref token) = self.token {
            let _ = self
                .notifier
                .progress(token.clone(), progress, Some(total), message)
                .await;
        }
    }

    /// Send a progress update with just a percentage (0.0 to 1.0).
    pub async fn update_percent(&self, percent: f64, message: Option<String>) {
        if let Some(ref token) = self.token {
            let _ = self
                .notifier
                .progress(token.clone(), percent, None, message)
                .await;
        }
    }

    /// Send a progress update with a message.
    pub async fn update_with_message(&self, progress: f64, total: f64, message: impl Into<String>) {
        self.update(progress, total, Some(message.into())).await;
    }

    /// Mark the operation as complete with a final message.
    pub async fn complete(&self, message: impl Into<String>) {
        if let Some(ref token) = self.token {
            let _ = self
                .notifier
                .progress(token.clone(), 1.0, Some(1.0), Some(message.into()))
                .await;
        }
    }

    /// Check if progress tracking is enabled (token was provided).
    pub fn is_tracking(&self) -> bool {
        self.token.is_some()
    }
}

/// Extension trait to extract progress token from request parameters.
pub trait ProgressTokenExt {
    /// Get the progress token from `_meta.progressToken` if present.
    fn progress_token(&self) -> Option<ProgressToken>;
}

impl ProgressTokenExt for serde_json::Value {
    fn progress_token(&self) -> Option<ProgressToken> {
        self.get("_meta")
            .and_then(|m| m.get("progressToken"))
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(ProgressToken::String(s.to_owned()))
                } else {
                    v.as_i64().map(ProgressToken::Number)
                }
            })
    }
}

//! Example: Server notifications and progress tracking
//!
//! This example demonstrates how to send notifications to clients and track progress
//! for long-running operations.
//!
//! Run: cargo run --example notifications

use mcp_kit::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Shared state for dynamic resources
struct AppState {
    items: RwLock<Vec<String>>,
    notifier: Option<NotificationSender>,
}

impl AppState {
    fn new() -> Self {
        Self {
            items: RwLock::new(vec!["item1".into(), "item2".into()]),
            notifier: None,
        }
    }

    fn with_notifier(mut self, notifier: NotificationSender) -> Self {
        self.notifier = Some(notifier);
        self
    }

    async fn add_item(&self, item: String) {
        self.items.write().await.push(item);
        // Notify clients that resources changed
        if let Some(ref notifier) = self.notifier {
            let _ = notifier.resources_list_changed().await;
            let _ = notifier
                .log_info("state", "New item added to collection")
                .await;
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ProcessInput {
    /// Number of items to process
    count: u32,
    /// Include progress updates
    #[serde(default)]
    with_progress: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AddItemInput {
    /// Item name to add
    name: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("notifications=debug,mcp=info")
        .init();

    // Create notification channel
    let (notifier, mut receiver) = NotificationSender::channel(100);

    // Shared state with notifier
    let state = Arc::new(AppState::new().with_notifier(notifier.clone()));

    // Spawn task to handle outgoing notifications
    // In a real server, this would be integrated with the transport
    tokio::spawn(async move {
        while let Some(notification) = receiver.recv().await {
            // In production, this would send to the client via the transport
            eprintln!("📢 Notification: {} - {:?}", notification.method, notification.params);
        }
    });

    let state_clone = state.clone();
    let notifier_clone = notifier.clone();

    McpServer::builder()
        .name("notifications-demo")
        .version("1.0.0")
        .instructions("Demonstrates notifications and progress tracking")
        // Tool with progress reporting
        .tool(
            Tool::new(
                "process_items",
                "Process items with optional progress tracking",
                serde_json::to_value(schemars::schema_for!(ProcessInput))?,
            ),
            move |input: ProcessInput| {
                let notifier = notifier_clone.clone();
                async move {
                    let tracker = ProgressTracker::new(notifier.clone(), None);

                    for i in 0..input.count {
                        // Simulate work
                        tokio::time::sleep(Duration::from_millis(100)).await;

                        if input.with_progress {
                            tracker
                                .update_with_message(
                                    i as f64 + 1.0,
                                    input.count as f64,
                                    format!("Processing item {}/{}", i + 1, input.count),
                                )
                                .await;
                        }
                    }

                    // Send log notification
                    let _ = notifier
                        .log_info("process", format!("Processed {} items", input.count))
                        .await;

                    CallToolResult::text(format!("Processed {} items successfully", input.count))
                }
            },
        )
        // Tool that modifies state and sends notifications
        .tool(
            Tool::new(
                "add_item",
                "Add an item and notify clients",
                serde_json::to_value(schemars::schema_for!(AddItemInput))?,
            ),
            move |input: AddItemInput| {
                let state = state_clone.clone();
                async move {
                    state.add_item(input.name.clone()).await;
                    CallToolResult::text(format!("Added item: {}", input.name))
                }
            },
        )
        // Resource that reads from state
        .resource(
            Resource::new("items://list", "Item List").with_description("Current items"),
            move |_req| {
                let state = state.clone();
                async move {
                    let items = state.items.read().await;
                    let content = items.join("\n");
                    Ok(ReadResourceResult::text("items://list", content))
                }
            },
        )
        .build()
        .serve_stdio()
        .await?;

    Ok(())
}

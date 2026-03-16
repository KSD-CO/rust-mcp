//! Database statistics and health tools.

use mcp_kit::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::domain::port::SharedDatabase;

// ─── Input Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StatsInput {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProcessListInput {
    /// Only show queries running longer than this many seconds
    pub min_elapsed_seconds: Option<f64>,
}

// ─── Tool Registration ───────────────────────────────────────────────────────

pub fn register_stats_tool(
    builder: McpServerBuilder,
    db: SharedDatabase,
) -> McpServerBuilder {
    builder.tool(
        Tool::new(
            "clickhouse_stats",
            "Get database-level statistics: table count, total rows, total size",
            serde_json::to_value(schemars::schema_for!(StatsInput)).unwrap(),
        ),
        {
            let db = db.clone();
            move |_params: StatsInput| {
                let db = db.clone();
                async move {
                    match db.database_stats().await {
                        Ok(stats) => CallToolResult::text(format!(
                            "Database: {}\n\
                             Tables: {}\n\
                             Total rows: {}\n\
                             Total size: {:.2} GB ({} bytes)",
                            stats.database,
                            stats.table_count,
                            stats.total_rows,
                            stats.size_gb(),
                            stats.total_bytes
                        )),
                        Err(e) => CallToolResult::error(format!("Failed to get stats: {e}")),
                    }
                }
            }
        },
    )
}

pub fn register_processlist_tool(
    builder: McpServerBuilder,
    db: SharedDatabase,
) -> McpServerBuilder {
    builder.tool(
        Tool::new(
            "clickhouse_processlist",
            "Show currently running queries (system.processes)",
            serde_json::to_value(schemars::schema_for!(ProcessListInput)).unwrap(),
        ),
        {
            let db = db.clone();
            move |params: ProcessListInput| {
                let db = db.clone();
                async move {
                    let min = params.min_elapsed_seconds.unwrap_or(0.0);
                    match db.process_list(min).await {
                        Ok(list) => {
                            if list.is_empty {
                                CallToolResult::text("No running queries found.")
                            } else {
                                CallToolResult::text(format!(
                                    "Running queries:\n\n{}",
                                    list.text
                                ))
                            }
                        }
                        Err(e) => {
                            CallToolResult::error(format!("Failed to get process list: {e}"))
                        }
                    }
                }
            }
        },
    )
}

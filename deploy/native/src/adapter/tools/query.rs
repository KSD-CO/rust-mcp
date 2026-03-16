//! SQL query execution tool.

use mcp_kit::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::domain::port::SharedDatabase;

// ─── Input Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryInput {
    /// SQL query to execute
    pub sql: String,
    /// Output format: TabSeparated (default), JSONEachRow, CSV, Pretty, etc.
    pub format: Option<String>,
    /// Maximum number of rows to return (safety limit)
    pub limit: Option<u64>,
}

// ─── Tool Registration ───────────────────────────────────────────────────────

/// Register the `clickhouse_query` tool on the server builder.
pub fn register_query_tool(
    builder: McpServerBuilder,
    db: SharedDatabase,
) -> McpServerBuilder {
    builder.tool(
        Tool::new(
            "clickhouse_query",
            "Execute a SQL query against ClickHouse and return results. \
             Supports all ClickHouse SQL including SELECT, SHOW, DESCRIBE, EXPLAIN.",
            serde_json::to_value(schemars::schema_for!(QueryInput)).unwrap(),
        ),
        {
            let db = db.clone();
            move |params: QueryInput| {
                let db = db.clone();
                async move {
                    let format = params.format.unwrap_or_else(|| "TabSeparated".to_string());

                    // Inject LIMIT if not already present and user specified one
                    let sql = if let Some(limit) = params.limit {
                        if !params.sql.to_uppercase().contains("LIMIT") {
                            format!("{} LIMIT {}", params.sql.trim_end_matches(';'), limit)
                        } else {
                            params.sql
                        }
                    } else {
                        params.sql
                    };

                    match db.query(&sql, &format).await {
                        Ok(result) => CallToolResult::text(format!(
                            "Query executed successfully\nFormat: {format}\n\nResults:\n{}",
                            result.data
                        )),
                        Err(e) => CallToolResult::error(format!("Query failed: {e}")),
                    }
                }
            }
        },
    )
}

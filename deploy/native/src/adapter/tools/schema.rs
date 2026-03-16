//! Schema introspection tools — list tables, describe columns.

use mcp_kit::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::domain::port::SharedDatabase;

// ─── Input Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTablesInput {
    /// Optional pattern to filter table names (SQL LIKE syntax, e.g. "report_%")
    pub pattern: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DescribeTableInput {
    /// Fully-qualified table name (e.g. "mydb.my_table")
    pub table_name: String,
}

// ─── Tool Registration ───────────────────────────────────────────────────────

pub fn register_list_tables_tool(
    builder: McpServerBuilder,
    db: SharedDatabase,
) -> McpServerBuilder {
    builder.tool(
        Tool::new(
            "clickhouse_list_tables",
            "List all tables in the database with engine type, row count, and size",
            serde_json::to_value(schemars::schema_for!(ListTablesInput)).unwrap(),
        ),
        {
            let db = db.clone();
            move |params: ListTablesInput| {
                let db = db.clone();
                async move {
                    match db.list_tables(params.pattern.as_deref()).await {
                        Ok(tables) => {
                            let list: Vec<String> = tables
                                .iter()
                                .map(|t| {
                                    format!(
                                        "  {} ({}) - {} rows, {:.2} MB",
                                        t.name,
                                        t.engine,
                                        t.total_rows,
                                        t.size_mb()
                                    )
                                })
                                .collect();

                            let db_name = db.database_name();
                            CallToolResult::text(format!(
                                "Tables in '{}' ({} found):\n\n{}",
                                db_name,
                                list.len(),
                                if list.is_empty() {
                                    "  (none)".to_string()
                                } else {
                                    list.join("\n")
                                }
                            ))
                        }
                        Err(e) => CallToolResult::error(format!("Failed to list tables: {e}")),
                    }
                }
            }
        },
    )
}

pub fn register_describe_table_tool(
    builder: McpServerBuilder,
    db: SharedDatabase,
) -> McpServerBuilder {
    builder.tool(
        Tool::new(
            "clickhouse_describe_table",
            "Show column names, types, and metadata for a table (DESCRIBE TABLE)",
            serde_json::to_value(schemars::schema_for!(DescribeTableInput)).unwrap(),
        ),
        {
            let db = db.clone();
            move |params: DescribeTableInput| {
                let db = db.clone();
                async move {
                    match db.describe_table(&params.table_name).await {
                        Ok(schema) => CallToolResult::text(format!(
                            "Table: {}\nRow count: {}\n\nSchema:\n{}",
                            schema.table_name, schema.row_count, schema.schema_text
                        )),
                        Err(e) => CallToolResult::error(format!(
                            "Failed to describe '{}': {e}",
                            params.table_name
                        )),
                    }
                }
            }
        },
    )
}

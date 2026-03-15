//! ClickHouse Plugin - Real Database Integration
//!
//! This plugin provides WORKING ClickHouse database integration for:
//! - Running SQL queries and getting results
//! - Generating analytics reports
//! - Querying time-series data
//! - Getting database statistics
//!
//! ## Setup
//!
//! 1. Install ClickHouse locally or use cloud instance
//!    Docker: `docker run -d -p 8123:8123 clickhouse/clickhouse-server`
//!
//! 2. Set environment variables:
//!    ```bash
//!    export CLICKHOUSE_URL="http://localhost:8123"
//!    export CLICKHOUSE_USER="default"
//!    export CLICKHOUSE_PASSWORD=""
//!    export CLICKHOUSE_DATABASE="default"
//!    ```
//!
//! 3. Run:
//!    ```bash
//!    cargo run --example plugin_clickhouse --features plugin,plugin-native
//!    ```

use mcp_kit::plugin::{McpPlugin, PluginConfig, ResourceDefinition, ToolDefinition};
use mcp_kit::prelude::*;
use serde::Deserialize;

// ─── ClickHouse Plugin ───────────────────────────────────────────────────────

/// ClickHouse database integration
struct ClickHousePlugin {
    url: String,
    user: String,
    password: String,
    database: String,
    client: reqwest::Client,
}

impl ClickHousePlugin {
    pub fn new() -> Self {
        Self {
            url: String::new(),
            user: String::new(),
            password: String::new(),
            database: String::new(),
            client: reqwest::Client::new(),
        }
    }

    /// Execute a query and get results
    async fn query(
        &self,
        sql: &str,
        format: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        tracing::debug!("ClickHouse query: {}", sql);

        let url = format!(
            "{}/?query={}&database={}&user={}&password={}&default_format={}",
            self.url,
            urlencoding::encode(sql),
            urlencoding::encode(&self.database),
            urlencoding::encode(&self.user),
            urlencoding::encode(&self.password),
            format
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(format!("ClickHouse error {}: {}", status, error_text).into());
        }

        let result = response.text().await?;
        Ok(result)
    }

    /// Execute query and parse as JSON
    async fn query_json(
        &self,
        sql: &str,
    ) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.query(sql, "JSONEachRow").await?;

        let rows: Vec<serde_json::Value> = result
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();

        Ok(rows)
    }
}

impl Clone for ClickHousePlugin {
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            user: self.user.clone(),
            password: self.password.clone(),
            database: self.database.clone(),
            client: reqwest::Client::new(),
        }
    }
}

impl McpPlugin for ClickHousePlugin {
    fn name(&self) -> &str {
        "clickhouse"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> Option<&str> {
        Some("ClickHouse database integration - run queries and generate reports")
    }

    fn author(&self) -> Option<&str> {
        Some("MCP Kit Team")
    }

    fn register_tools(&self) -> Vec<ToolDefinition> {
        vec![
            // Execute SQL query
            ToolDefinition::new(
                Tool::new(
                    "clickhouse_query",
                    "Execute a SQL query and get results",
                    serde_json::to_value(schemars::schema_for!(QueryInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: QueryInput| {
                        let plugin = plugin.clone();
                        async move {
                            let format =
                                params.format.unwrap_or_else(|| "TabSeparated".to_string());

                            match plugin.query(&params.sql, &format).await {
                                Ok(result) => {
                                    let preview = if result.len() > 1000 {
                                        format!(
                                            "{}...\n\n(truncated, {} bytes total)",
                                            &result[..1000],
                                            result.len()
                                        )
                                    } else {
                                        result
                                    };

                                    CallToolResult::text(format!(
                                        "✅ Query executed successfully\n\n\
                                         Format: {}\n\n\
                                         Results:\n{}",
                                        format, preview
                                    ))
                                }
                                Err(e) => CallToolResult::error(format!("Query failed: {}", e)),
                            }
                        }
                    }
                },
            ),
            // Get table info
            ToolDefinition::new(
                Tool::new(
                    "clickhouse_table_info",
                    "Get information about a table (schema, row count, size)",
                    serde_json::to_value(schemars::schema_for!(TableInfoInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: TableInfoInput| {
                        let plugin = plugin.clone();
                        async move {
                            // Get table schema
                            let schema_sql = format!("DESCRIBE TABLE {}", params.table_name);
                            let count_sql =
                                format!("SELECT count() as count FROM {}", params.table_name);

                            match plugin.query(&schema_sql, "TabSeparated").await {
                                Ok(schema) => {
                                    let count_result =
                                        plugin.query(&count_sql, "TabSeparated").await;
                                    let row_count = count_result
                                        .ok()
                                        .and_then(|c| c.trim().parse::<u64>().ok())
                                        .unwrap_or(0);

                                    CallToolResult::text(format!(
                                        "📊 Table: {}\n\
                                         Rows: {}\n\n\
                                         Schema:\n{}",
                                        params.table_name, row_count, schema
                                    ))
                                }
                                Err(e) => CallToolResult::error(format!(
                                    "Failed to get table info: {}",
                                    e
                                )),
                            }
                        }
                    }
                },
            ),
            // Generate analytics report
            ToolDefinition::new(
                Tool::new(
                    "clickhouse_analytics",
                    "Generate analytics report with common metrics",
                    serde_json::to_value(schemars::schema_for!(AnalyticsInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: AnalyticsInput| {
                        let plugin = plugin.clone();
                        async move {
                            let time_range =
                                params.time_range.unwrap_or_else(|| "24 HOUR".to_string());

                            let sql = format!(
                                "SELECT \
                                 toStartOfHour(timestamp) as hour, \
                                 count() as events, \
                                 uniq({}) as unique_users, \
                                 avg({}) as avg_value \
                                 FROM {} \
                                 WHERE timestamp >= now() - INTERVAL {} \
                                 GROUP BY hour \
                                 ORDER BY hour DESC \
                                 LIMIT 24",
                                params.user_column.as_deref().unwrap_or("user_id"),
                                params.value_column.as_deref().unwrap_or("value"),
                                params.table_name,
                                time_range
                            );

                            match plugin.query_json(&sql).await {
                                Ok(rows) => {
                                    let report: Vec<String> = rows
                                        .iter()
                                        .map(|row| {
                                            let hour = row["hour"].as_str().unwrap_or("unknown");
                                            let events = row["events"].as_u64().unwrap_or(0);
                                            let users = row["unique_users"].as_u64().unwrap_or(0);
                                            let avg = row["avg_value"].as_f64().unwrap_or(0.0);
                                            format!(
                                                "{}: {} events, {} users, avg {:.2}",
                                                hour, events, users, avg
                                            )
                                        })
                                        .collect();

                                    CallToolResult::text(format!(
                                        "📈 Analytics Report\n\
                                         Table: {}\n\
                                         Time Range: Last {}\n\n\
                                         {}",
                                        params.table_name,
                                        time_range,
                                        if report.is_empty() {
                                            "No data found".to_string()
                                        } else {
                                            report.join("\n")
                                        }
                                    ))
                                }
                                Err(e) => {
                                    CallToolResult::error(format!("Analytics query failed: {}", e))
                                }
                            }
                        }
                    }
                },
            ),
            // List tables
            ToolDefinition::new(
                Tool::new(
                    "clickhouse_list_tables",
                    "List all tables in the database",
                    serde_json::to_value(schemars::schema_for!(ListTablesInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |_params: ListTablesInput| {
                        let plugin = plugin.clone();
                        async move {
                            let sql = format!(
                                "SELECT name, engine, total_rows, total_bytes \
                                 FROM system.tables \
                                 WHERE database = '{}' \
                                 ORDER BY total_bytes DESC",
                                plugin.database
                            );

                            match plugin.query_json(&sql).await {
                                Ok(tables) => {
                                    let table_list: Vec<String> = tables
                                        .iter()
                                        .map(|t| {
                                            let name = t["name"].as_str().unwrap_or("unknown");
                                            let engine = t["engine"].as_str().unwrap_or("Unknown");
                                            let rows = t["total_rows"].as_u64().unwrap_or(0);
                                            let bytes = t["total_bytes"].as_u64().unwrap_or(0);
                                            let mb = bytes as f64 / 1024.0 / 1024.0;
                                            format!(
                                                "• {} ({}) - {} rows, {:.2} MB",
                                                name, engine, rows, mb
                                            )
                                        })
                                        .collect();

                                    CallToolResult::text(format!(
                                        "📊 Tables in database '{}':\n\n{}",
                                        plugin.database,
                                        if table_list.is_empty() {
                                            "No tables found".to_string()
                                        } else {
                                            table_list.join("\n")
                                        }
                                    ))
                                }
                                Err(e) => {
                                    CallToolResult::error(format!("Failed to list tables: {}", e))
                                }
                            }
                        }
                    }
                },
            ),
            // Get database stats
            ToolDefinition::new(
                Tool::new(
                    "clickhouse_stats",
                    "Get database statistics and performance metrics",
                    serde_json::to_value(schemars::schema_for!(StatsInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |_params: StatsInput| {
                        let plugin = plugin.clone();
                        async move {
                            let sql = format!(
                                "SELECT \
                                 countDistinct(table) as tables, \
                                 sum(rows) as total_rows, \
                                 sum(bytes) as total_bytes \
                                 FROM system.parts \
                                 WHERE database = '{}'",
                                plugin.database
                            );

                            match plugin.query_json(&sql).await {
                                Ok(rows) => {
                                    if let Some(stats) = rows.first() {
                                        let tables = stats["tables"].as_u64().unwrap_or(0);
                                        let total_rows = stats["total_rows"].as_u64().unwrap_or(0);
                                        let total_bytes =
                                            stats["total_bytes"].as_u64().unwrap_or(0);
                                        let gb = total_bytes as f64 / 1024.0 / 1024.0 / 1024.0;

                                        CallToolResult::text(format!(
                                            "📊 Database Statistics: {}\n\n\
                                             Tables: {}\n\
                                             Total Rows: {}\n\
                                             Total Size: {:.2} GB ({} bytes)",
                                            plugin.database, tables, total_rows, gb, total_bytes
                                        ))
                                    } else {
                                        CallToolResult::error("No statistics available")
                                    }
                                }
                                Err(e) => {
                                    CallToolResult::error(format!("Failed to get stats: {}", e))
                                }
                            }
                        }
                    }
                },
            ),
            // Run report query
            ToolDefinition::new(
                Tool::new(
                    "clickhouse_report",
                    "Run a predefined report query (daily summary, user activity, etc.)",
                    serde_json::to_value(schemars::schema_for!(ReportInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: ReportInput| {
                        let plugin = plugin.clone();
                        async move {
                            let sql = match params.report_type.as_str() {
                                "daily_summary" => {
                                    format!(
                                        "SELECT \
                                         toDate(timestamp) as date, \
                                         count() as total_events, \
                                         uniq(user_id) as unique_users \
                                         FROM {} \
                                         WHERE timestamp >= today() - INTERVAL 7 DAY \
                                         GROUP BY date \
                                         ORDER BY date DESC",
                                        params.table_name
                                    )
                                }
                                "hourly_activity" => {
                                    format!(
                                        "SELECT \
                                         toStartOfHour(timestamp) as hour, \
                                         count() as events, \
                                         uniq(user_id) as users \
                                         FROM {} \
                                         WHERE timestamp >= now() - INTERVAL 24 HOUR \
                                         GROUP BY hour \
                                         ORDER BY hour DESC",
                                        params.table_name
                                    )
                                }
                                "top_users" => {
                                    format!(
                                        "SELECT \
                                         user_id, \
                                         count() as event_count \
                                         FROM {} \
                                         WHERE timestamp >= now() - INTERVAL 7 DAY \
                                         GROUP BY user_id \
                                         ORDER BY event_count DESC \
                                         LIMIT 10",
                                        params.table_name
                                    )
                                }
                                _ => {
                                    return CallToolResult::error(format!(
                                        "Unknown report type: {}. Available: daily_summary, hourly_activity, top_users",
                                        params.report_type
                                    ));
                                }
                            };

                            match plugin.query_json(&sql).await {
                                Ok(rows) => {
                                    let formatted = rows
                                        .iter()
                                        .map(|row| {
                                            format!(
                                                "{}",
                                                serde_json::to_string_pretty(row)
                                                    .unwrap_or_default()
                                            )
                                        })
                                        .collect::<Vec<_>>()
                                        .join("\n");

                                    CallToolResult::text(format!(
                                        "📊 Report: {}\n\
                                         Table: {}\n\
                                         Rows: {}\n\n\
                                         Results:\n{}",
                                        params.report_type,
                                        params.table_name,
                                        rows.len(),
                                        if formatted.is_empty() {
                                            "No data found".to_string()
                                        } else {
                                            formatted
                                        }
                                    ))
                                }
                                Err(e) => CallToolResult::error(format!(
                                    "Report generation failed: {}",
                                    e
                                )),
                            }
                        }
                    }
                },
            ),
            // Insert data
            ToolDefinition::new(
                Tool::new(
                    "clickhouse_insert",
                    "Insert data into a table",
                    serde_json::to_value(schemars::schema_for!(InsertInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: InsertInput| {
                        let plugin = plugin.clone();
                        async move {
                            let sql = format!(
                                "INSERT INTO {} FORMAT JSONEachRow\n{}",
                                params.table_name, params.data
                            );

                            match plugin.query(&sql, "JSON").await {
                                Ok(_) => {
                                    let lines = params.data.lines().count();
                                    CallToolResult::text(format!(
                                        "✅ Inserted {} rows into {}",
                                        lines, params.table_name
                                    ))
                                }
                                Err(e) => CallToolResult::error(format!("Insert failed: {}", e)),
                            }
                        }
                    }
                },
            ),
        ]
    }

    fn register_resources(&self) -> Vec<ResourceDefinition> {
        vec![
            // Database info resource
            ResourceDefinition::new(
                Resource::new(
                    format!("clickhouse://database/{}", self.database),
                    "ClickHouse Database Information",
                )
                .with_mime_type("application/json")
                .with_description("Database metadata and connection info"),
                {
                    let plugin = self.clone();
                    move |req| {
                        let plugin = plugin.clone();
                        async move {
                            let info = serde_json::json!({
                                "database": plugin.database,
                                "url": plugin.url,
                                "user": plugin.user,
                                "connection": "active"
                            });

                            Ok(ReadResourceResult::text(
                                req.uri,
                                serde_json::to_string_pretty(&info).unwrap(),
                            ))
                        }
                    }
                },
            ),
        ]
    }

    fn on_load(&mut self, config: &PluginConfig) -> McpResult<()> {
        self.url = config
            .config
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("url required"))?
            .to_string();

        self.user = config
            .config
            .get("user")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        self.password = config
            .config
            .get("password")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        self.database = config
            .config
            .get("database")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        tracing::info!(
            "✅ ClickHouse plugin loaded: {} (database: {})",
            self.url,
            self.database
        );
        Ok(())
    }

    fn on_unload(&mut self) -> McpResult<()> {
        tracing::info!("ClickHouse plugin unloaded");
        Ok(())
    }
}

// ─── Input Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct QueryInput {
    /// SQL query to execute
    sql: String,
    /// Output format (TabSeparated, JSONEachRow, CSV, etc.)
    format: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TableInfoInput {
    /// Table name
    table_name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AnalyticsInput {
    /// Table name to analyze
    table_name: String,
    /// Time range (e.g., "24 HOUR", "7 DAY", "30 DAY")
    time_range: Option<String>,
    /// Column containing user ID (optional)
    user_column: Option<String>,
    /// Column containing numeric value (optional)
    value_column: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ReportInput {
    /// Report type (daily_summary, hourly_activity, top_users)
    report_type: String,
    /// Table name
    table_name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct InsertInput {
    /// Table name
    table_name: String,
    /// Data in JSONEachRow format (one JSON object per line)
    data: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ListTablesInput {}

#[derive(Debug, Deserialize, JsonSchema)]
struct StatsInput {}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("plugin_clickhouse=debug,mcp_kit=info")
        .init();

    tracing::info!("🚀 Starting ClickHouse plugin with REAL database");

    let mut plugin_manager = mcp_kit::plugin::PluginManager::new();

    let config = mcp_kit::plugin::PluginConfig {
        config: serde_json::json!({
            "url": std::env::var("CLICKHOUSE_URL")
                .unwrap_or_else(|_| {
                    tracing::warn!("CLICKHOUSE_URL not set, using default");
                    "http://localhost:8123".to_string()
                }),
            "user": std::env::var("CLICKHOUSE_USER")
                .unwrap_or_else(|_| "default".to_string()),
            "password": std::env::var("CLICKHOUSE_PASSWORD")
                .unwrap_or_else(|_| "".to_string()),
            "database": std::env::var("CLICKHOUSE_DATABASE")
                .unwrap_or_else(|_| "default".to_string()),
        }),
        enabled: true,
        priority: 0,
        permissions: mcp_kit::plugin::PluginPermissions {
            network: true,
            ..Default::default()
        },
    };

    plugin_manager.register_plugin(ClickHousePlugin::new(), config)?;

    let plugins = plugin_manager.list_plugins();
    for plugin in plugins {
        tracing::info!(
            "📦 Plugin: {} v{} ({} tools, {} resources)",
            plugin.name,
            plugin.version,
            plugin.tool_count,
            plugin.resource_count
        );
    }

    let server = McpServer::builder()
        .name("clickhouse-server")
        .version("1.0.0")
        .instructions(
            "ClickHouse database integration with REAL queries.\n\n\
             Tools:\n\
             - clickhouse_query: Execute SQL queries\n\
             - clickhouse_table_info: Get table schema and stats\n\
             - clickhouse_analytics: Generate analytics reports\n\
             - clickhouse_list_tables: List all tables\n\
             - clickhouse_stats: Database statistics\n\
             - clickhouse_insert: Insert data\n\n\
             Environment variables:\n\
             - CLICKHOUSE_URL (default: http://localhost:8123)\n\
             - CLICKHOUSE_USER (default: default)\n\
             - CLICKHOUSE_PASSWORD (default: empty)\n\
             - CLICKHOUSE_DATABASE (default: default)\n\n\
             Quick start with Docker:\n\
             docker run -d -p 8123:8123 clickhouse/clickhouse-server",
        )
        .with_plugin_manager(plugin_manager)
        .build();

    tracing::info!("✅ Server ready");

    server.serve_stdio().await?;
    Ok(())
}

//! ClickHouse HTTP client — implements [`DatabasePort`].

use async_trait::async_trait;
use reqwest::Client;

use crate::domain::model::{
    DatabaseInfo, DatabaseStats, ProcessList, QueryResult, TableInfo, TableSchema,
};
use crate::domain::port::DatabasePort;

// ─── Configuration ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ClickHouseConfig {
    pub url: String,
    pub user: String,
    pub password: String,
    pub database: String,
}

impl ClickHouseConfig {
    pub fn from_env() -> Self {
        Self {
            url: std::env::var("CLICKHOUSE_URL")
                .unwrap_or_else(|_| "http://localhost:8123".to_string()),
            user: std::env::var("CLICKHOUSE_USER")
                .unwrap_or_else(|_| "default".to_string()),
            password: std::env::var("CLICKHOUSE_PASSWORD").unwrap_or_default(),
            database: std::env::var("CLICKHOUSE_DATABASE")
                .unwrap_or_else(|_| "default".to_string()),
        }
    }
}

// ─── Client ──────────────────────────────────────────────────────────────────

/// ClickHouse HTTP interface client.
pub struct ClickHouseClient {
    config: ClickHouseConfig,
    http: Client,
}

impl ClickHouseClient {
    pub fn new(config: ClickHouseConfig) -> Self {
        Self {
            config,
            http: Client::new(),
        }
    }

    /// Low-level: execute a raw SQL query and return the response body as text.
    async fn raw_query(&self, sql: &str, format: &str) -> anyhow::Result<String> {
        tracing::debug!(sql, format, "Executing ClickHouse query");

        let url = format!(
            "{}/?query={}&database={}&user={}&password={}&default_format={}",
            self.config.url,
            urlencoding::encode(sql),
            urlencoding::encode(&self.config.database),
            urlencoding::encode(&self.config.user),
            urlencoding::encode(&self.config.password),
            format,
        );

        let response = self.http.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            anyhow::bail!("ClickHouse error {status}: {body}");
        }

        Ok(response.text().await?)
    }

    /// Low-level: execute a query and parse rows as JSON values (JSONEachRow).
    async fn query_json(&self, sql: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        let raw = self.raw_query(sql, "JSONEachRow").await?;
        let rows: Vec<serde_json::Value> = raw
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        Ok(rows)
    }
}

// ─── DatabasePort Implementation ─────────────────────────────────────────────

#[async_trait]
impl DatabasePort for ClickHouseClient {
    async fn query(&self, sql: &str, format: &str) -> anyhow::Result<QueryResult> {
        let data = self.raw_query(sql, format).await?;
        let total_bytes = data.len();
        let truncated = total_bytes > 5000;
        let data = if truncated {
            // Find a valid UTF-8 char boundary at or before 5000
            let end = floor_char_boundary(&data, 5000);
            format!(
                "{}...\n\n(truncated, {} bytes total)",
                &data[..end],
                total_bytes
            )
        } else {
            data
        };

        Ok(QueryResult {
            data,
            total_bytes,
            truncated,
        })
    }

    async fn list_tables(&self, pattern: Option<&str>) -> anyhow::Result<Vec<TableInfo>> {
        let mut sql = format!(
            "SELECT name, engine, total_rows, total_bytes \
             FROM system.tables \
             WHERE database = '{}'",
            self.config.database
        );

        if let Some(pattern) = pattern {
            sql.push_str(&format!(" AND name LIKE '{}'", pattern));
        }

        sql.push_str(" ORDER BY total_bytes DESC");

        let rows = self.query_json(&sql).await?;

        Ok(rows
            .iter()
            .map(|t| TableInfo {
                name: t["name"].as_str().unwrap_or("?").to_string(),
                engine: t["engine"].as_str().unwrap_or("?").to_string(),
                total_rows: t["total_rows"].as_u64().unwrap_or(0),
                total_bytes: t["total_bytes"].as_u64().unwrap_or(0),
            })
            .collect())
    }

    async fn describe_table(&self, table_name: &str) -> anyhow::Result<TableSchema> {
        let describe_sql = format!("DESCRIBE TABLE {}", table_name);
        let count_sql = format!("SELECT count() as cnt FROM {}", table_name);

        let schema_text = self.raw_query(&describe_sql, "TabSeparated").await?;
        let row_count = self
            .raw_query(&count_sql, "TabSeparated")
            .await
            .ok()
            .and_then(|c| c.trim().parse::<u64>().ok())
            .unwrap_or(0);

        Ok(TableSchema {
            table_name: table_name.to_string(),
            schema_text,
            row_count,
        })
    }

    async fn database_stats(&self) -> anyhow::Result<DatabaseStats> {
        let sql = format!(
            "SELECT \
             countDistinct(table) as tables, \
             sum(rows) as total_rows, \
             sum(bytes) as total_bytes \
             FROM system.parts \
             WHERE database = '{}'",
            self.config.database
        );

        let rows = self.query_json(&sql).await?;

        if let Some(s) = rows.first() {
            Ok(DatabaseStats {
                database: self.config.database.clone(),
                table_count: s["tables"].as_u64().unwrap_or(0),
                total_rows: s["total_rows"].as_u64().unwrap_or(0),
                total_bytes: s["total_bytes"].as_u64().unwrap_or(0),
            })
        } else {
            anyhow::bail!("No statistics available")
        }
    }

    async fn process_list(&self, min_elapsed_seconds: f64) -> anyhow::Result<ProcessList> {
        let sql = format!(
            "SELECT query_id, user, elapsed, read_rows, read_bytes, \
             memory_usage, query \
             FROM system.processes \
             WHERE elapsed >= {} \
             ORDER BY elapsed DESC \
             LIMIT 20",
            min_elapsed_seconds
        );

        let text = self.raw_query(&sql, "PrettyCompact").await?;
        let is_empty = text.trim().is_empty();
        Ok(ProcessList { text, is_empty })
    }

    async fn database_info(&self) -> anyhow::Result<DatabaseInfo> {
        let stats = self.database_stats().await.ok();
        Ok(DatabaseInfo {
            database: self.config.database.clone(),
            url: self.config.url.clone(),
            user: self.config.user.clone(),
            stats,
        })
    }

    async fn ping(&self) -> anyhow::Result<()> {
        let result = self.raw_query("SELECT 1", "TabSeparated").await?;
        if result.trim() == "1" {
            Ok(())
        } else {
            anyhow::bail!("Unexpected ping response: {result}")
        }
    }

    fn database_name(&self) -> &str {
        &self.config.database
    }

    fn connection_url(&self) -> &str {
        &self.config.url
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Find the largest byte index <= `max` that is a valid UTF-8 char boundary.
fn floor_char_boundary(s: &str, max: usize) -> usize {
    if max >= s.len() {
        return s.len();
    }
    let mut i = max;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

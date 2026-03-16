//! Port definitions — abstract interfaces the domain exposes.
//!
//! Infrastructure (ClickHouse HTTP client) implements these traits.
//! Adapters (MCP tools/resources) depend only on these traits.

use std::sync::Arc;

use async_trait::async_trait;

use crate::domain::model::{
    DatabaseInfo, DatabaseStats, ProcessList, QueryResult, TableInfo, TableSchema,
};

/// Abstract database operations port.
///
/// This trait decouples domain logic from the concrete ClickHouse HTTP client.
/// Any database backend that implements this trait can be used with the MCP
/// adapter layer (tools, resources).
#[async_trait]
pub trait DatabasePort: Send + Sync + 'static {
    /// Execute a raw SQL query with the given output format.
    async fn query(&self, sql: &str, format: &str) -> anyhow::Result<QueryResult>;

    /// List tables in the configured database, optionally filtered by pattern.
    async fn list_tables(&self, pattern: Option<&str>) -> anyhow::Result<Vec<TableInfo>>;

    /// Describe table schema and return row count.
    async fn describe_table(&self, table_name: &str) -> anyhow::Result<TableSchema>;

    /// Get aggregated database statistics.
    async fn database_stats(&self) -> anyhow::Result<DatabaseStats>;

    /// Get the running process list, filtered by minimum elapsed seconds.
    async fn process_list(&self, min_elapsed_seconds: f64) -> anyhow::Result<ProcessList>;

    /// Get database connection info and summary stats.
    async fn database_info(&self) -> anyhow::Result<DatabaseInfo>;

    /// Health check — returns `Ok(())` if the database is reachable.
    async fn ping(&self) -> anyhow::Result<()>;

    /// The configured database name.
    fn database_name(&self) -> &str;

    /// The configured connection URL.
    fn connection_url(&self) -> &str;
}

/// Shared handle to a [`DatabasePort`] implementation.
pub type SharedDatabase = Arc<dyn DatabasePort>;

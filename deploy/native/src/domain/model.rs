//! Domain models — pure data types with no framework dependencies.

use serde::Serialize;

// ─── Query Result ─────────────────────────────────────────────────────────────

/// Result of executing an arbitrary SQL query.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct QueryResult {
    /// Raw text output from the database (formatted per the requested format).
    pub data: String,
    /// Total byte length of the original (untruncated) response.
    pub total_bytes: usize,
    /// Whether the result was truncated for display.
    pub truncated: bool,
}

// ─── Table Info ───────────────────────────────────────────────────────────────

/// Summary information about a database table.
#[derive(Debug, Clone, Serialize)]
pub struct TableInfo {
    pub name: String,
    pub engine: String,
    pub total_rows: u64,
    pub total_bytes: u64,
}

impl TableInfo {
    /// Human-readable size in MB.
    pub fn size_mb(&self) -> f64 {
        self.total_bytes as f64 / 1024.0 / 1024.0
    }
}

// ─── Table Schema ─────────────────────────────────────────────────────────────

/// Column schema for a table (tab-separated DESCRIBE output) plus row count.
#[derive(Debug, Clone)]
pub struct TableSchema {
    pub table_name: String,
    pub schema_text: String,
    pub row_count: u64,
}

// ─── Database Statistics ──────────────────────────────────────────────────────

/// Aggregated statistics for a database.
#[derive(Debug, Clone, Serialize)]
pub struct DatabaseStats {
    pub database: String,
    pub table_count: u64,
    pub total_rows: u64,
    pub total_bytes: u64,
}

impl DatabaseStats {
    /// Human-readable size in GB.
    pub fn size_gb(&self) -> f64 {
        self.total_bytes as f64 / 1024.0 / 1024.0 / 1024.0
    }
}

// ─── Process Info ─────────────────────────────────────────────────────────────

/// Running query process list (pre-formatted text from the database).
#[derive(Debug, Clone)]
pub struct ProcessList {
    pub text: String,
    pub is_empty: bool,
}

// ─── Database Info ────────────────────────────────────────────────────────────

/// Combined connection info and summary stats (for resources).
#[derive(Debug, Clone, Serialize)]
pub struct DatabaseInfo {
    pub database: String,
    pub url: String,
    pub user: String,
    pub stats: Option<DatabaseStats>,
}

//! CLI and configuration — parsed from command-line args and environment.

use std::path::PathBuf;

use clap::Parser;

use crate::infrastructure::clickhouse::ClickHouseConfig;

// ─── CLI ─────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "mcp-gateway",
    about = "MCP Gateway — aggregates ClickHouse (internal) + Grafana (external) tools"
)]
pub struct Cli {
    /// Transport: stdio, sse, ws
    #[arg(short, long, default_value = "stdio")]
    pub transport: String,

    /// Port for SSE / WebSocket transports
    #[arg(short, long, default_value = "3000")]
    pub port: u16,

    /// Path to .env file (loads environment variables from file)
    #[arg(long, value_name = "FILE")]
    pub env_file: Option<PathBuf>,

    // ── ClickHouse (internal backend) ──
    /// ClickHouse HTTP URL (overrides env)
    #[arg(long, env = "CLICKHOUSE_URL")]
    url: Option<String>,

    /// ClickHouse user (overrides env)
    #[arg(long, env = "CLICKHOUSE_USER")]
    user: Option<String>,

    /// ClickHouse password (overrides env)
    #[arg(long, env = "CLICKHOUSE_PASSWORD")]
    password: Option<String>,

    /// ClickHouse database (overrides env)
    #[arg(long, env = "CLICKHOUSE_DATABASE")]
    database: Option<String>,

    // ── Grafana (external upstream) ──
    /// Grafana instance URL. When set, the gateway spawns mcp-grafana and
    /// proxies its tools (dashboards, datasources, alerting, Prometheus,
    /// Loki, etc.) through this gateway.
    #[arg(long, env = "GRAFANA_URL")]
    pub grafana_url: Option<String>,

    /// Grafana service account token for authentication.
    #[arg(long, env = "GRAFANA_SERVICE_ACCOUNT_TOKEN")]
    pub grafana_token: Option<String>,

    /// Path to mcp-grafana binary. Defaults to "uvx" which auto-downloads
    /// the mcp-grafana package. Set to e.g. "/usr/local/bin/mcp-grafana"
    /// to use a pre-installed binary.
    #[arg(long, env = "GRAFANA_MCP_BIN", default_value = "uvx")]
    pub grafana_mcp_bin: String,

    /// Prefix for Grafana tools (e.g. "grafana" → tools become "grafana/search_dashboards").
    /// Set to empty string to disable prefixing.
    #[arg(long, env = "GRAFANA_PREFIX", default_value = "grafana")]
    pub grafana_prefix: String,
}

impl Cli {
    /// Load environment from .env file.
    ///
    /// Priority: `--env-file <path>` > `.env` in cwd > no file (silent).
    pub fn load_env(&self) {
        if let Some(ref path) = self.env_file {
            dotenvy::from_path(path)
                .unwrap_or_else(|e| panic!("Failed to load env file '{}': {e}", path.display()));
        } else {
            dotenvy::dotenv().ok();
        }
    }

    /// Whether the Grafana upstream is configured.
    pub fn grafana_enabled(&self) -> bool {
        self.grafana_url.is_some()
    }

    /// Build a [`ClickHouseConfig`] by merging CLI args (higher priority) with env vars.
    pub fn into_clickhouse_config(self) -> (Self, ClickHouseConfig) {
        let mut config = ClickHouseConfig::from_env();

        if let Some(ref url) = self.url {
            config.url = url.clone();
        }
        if let Some(ref user) = self.user {
            config.user = user.clone();
        }
        if let Some(ref password) = self.password {
            config.password = password.clone();
        }
        if let Some(ref database) = self.database {
            config.database = database.clone();
        }

        (self, config)
    }
}

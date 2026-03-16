//! MCP Gateway — ClickHouse (internal) + Grafana (external)
//!
//! A gateway MCP server that aggregates:
//! - **ClickHouse** tools (internal) — direct HTTP connection to ClickHouse
//! - **Grafana** tools (external) — proxied from [mcp-grafana](https://github.com/grafana/mcp-grafana)
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────┐        ┌──────────────────────────┐        ┌───────────────────┐
//! │ AI Agent │ ─────> │      mcp-gateway         │ stdio  │   mcp-grafana     │
//! │          │        │                          │ ─────> │   (external)      │
//! │          │        │  ┌────────────────────┐  │        └───────────────────┘
//! │          │        │  │ ClickHouse tools   │  │
//! │          │        │  │ (internal, HTTP)   │──│──> ClickHouse :8123
//! │          │        │  └────────────────────┘  │
//! └──────────┘        └──────────────────────────┘
//! ```
//!
//! - **Internal backends** are implemented directly in this binary (ClickHouse
//!   via HTTP client). No proxy overhead.
//! - **External upstreams** are MCP servers proxied via `mcp-kit-gateway`. The
//!   gateway spawns them as subprocesses and forwards tool calls over stdio.
//!
//! ## Usage
//!
//! ```bash
//! # ClickHouse only
//! mcp-gateway
//!
//! # ClickHouse + Grafana
//! GRAFANA_URL=http://localhost:3000 \
//! GRAFANA_SERVICE_ACCOUNT_TOKEN=<token> \
//!   mcp-gateway --transport sse --port 3000
//! ```

mod adapter;
mod config;
mod domain;
mod infrastructure;

use std::sync::Arc;

use clap::Parser;
use mcp_kit::prelude::*;
use mcp_kit_gateway::{GatewayManager, UpstreamConfig, UpstreamTransport};

use crate::adapter::{resources, tools};
use crate::config::Cli;
use crate::domain::port::SharedDatabase;
use crate::infrastructure::clickhouse::ClickHouseClient;

// ─── Main (Composition Root) ─────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI first (before loading .env, so --env-file is available)
    let cli = Cli::parse();

    // Load .env file: --env-file <path> takes priority, else .env in cwd
    cli.load_env();

    // Init tracing (always stderr — required for stdio transport)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mcp_gateway=info,mcp_kit=info,mcp_kit_gateway=info".into()),
        )
        .init();

    // Build ClickHouse config (CLI args > env vars > defaults)
    let (cli, ch_config) = cli.into_clickhouse_config();

    tracing::info!(
        clickhouse_url = %ch_config.url,
        database = %ch_config.database,
        grafana = %cli.grafana_enabled(),
        "Starting MCP Gateway"
    );

    // ── Internal backend: ClickHouse ──
    let db: SharedDatabase = Arc::new(ClickHouseClient::new(ch_config.clone()));

    match db.ping().await {
        Ok(()) => tracing::info!("ClickHouse connection OK"),
        Err(e) => {
            tracing::warn!(error = %e, "ClickHouse ping failed — starting anyway");
        }
    }

    // ── Build gateway server ──
    let server = build_gateway(db, &ch_config, &cli).await?;

    // ── Serve ──
    match cli.transport.as_str() {
        "stdio" => {
            tracing::info!("Serving over stdio");
            server.serve_stdio().await?;
        }
        "sse" => {
            let addr = ([0, 0, 0, 0], cli.port);
            tracing::info!(port = cli.port, "Serving over SSE");
            server.serve_sse(addr).await?;
        }
        "ws" | "websocket" => {
            let addr = ([0, 0, 0, 0], cli.port);
            tracing::info!(port = cli.port, "Serving over WebSocket");
            server.serve_websocket(addr).await?;
        }
        other => {
            anyhow::bail!("Unknown transport '{other}'. Use: stdio, sse, ws");
        }
    }

    Ok(())
}

// ─── Gateway Assembly ─────────────────────────────────────────────────────────

/// Build the gateway MCP server:
/// 1. Register internal ClickHouse tools (direct HTTP, no proxy)
/// 2. Connect external Grafana upstream (if configured) and proxy its tools
async fn build_gateway(
    db: SharedDatabase,
    ch_config: &infrastructure::clickhouse::ClickHouseConfig,
    cli: &Cli,
) -> anyhow::Result<McpServer> {
    // ── Instructions ──
    let mut instructions = format!(
        "MCP Gateway — ClickHouse + Grafana\n\n\
         Internal backends:\n\
         - ClickHouse ({}, database: {})\n\
         - clickhouse_query, clickhouse_list_tables, clickhouse_describe_table\n\
         - clickhouse_stats, clickhouse_processlist",
        ch_config.url, ch_config.database
    );

    if cli.grafana_enabled() {
        instructions.push_str(&format!(
            "\n\nExternal upstreams:\n\
             - Grafana (tools prefixed with `{}/`)\n\
             - search_dashboards, get_dashboard_by_uid, list_datasources\n\
             - query_prometheus, query_loki_logs, alerting_manage_rules, etc.",
            cli.grafana_prefix
        ));
    }

    let mut builder = McpServer::builder()
        .name("mcp-gateway")
        .version("0.2.0")
        .instructions(instructions);

    // ── Internal: ClickHouse tools ──
    builder = tools::register_query_tool(builder, db.clone());
    builder = tools::register_list_tables_tool(builder, db.clone());
    builder = tools::register_describe_table_tool(builder, db.clone());
    builder = tools::register_stats_tool(builder, db.clone());
    builder = tools::register_processlist_tool(builder, db.clone());
    builder = resources::register_database_resource(builder, db.clone());

    // ── External: Grafana upstream (optional) ──
    if let Some(ref grafana_url) = cli.grafana_url {
        tracing::info!(
            grafana_url = %grafana_url,
            prefix = %cli.grafana_prefix,
            bin = %cli.grafana_mcp_bin,
            "Connecting to external upstream: Grafana"
        );

        let mut grafana_args: Vec<String> = Vec::new();
        let mut grafana_env: Vec<(String, String)> =
            vec![("GRAFANA_URL".into(), grafana_url.clone())];

        // If using uvx, first arg is the package name
        if cli.grafana_mcp_bin == "uvx" {
            grafana_args.push("mcp-grafana".into());
        }

        // Pass the service account token via env
        if let Some(ref token) = cli.grafana_token {
            grafana_env.push(("GRAFANA_SERVICE_ACCOUNT_TOKEN".into(), token.clone()));
        }

        let mut gw = GatewayManager::new();
        gw.add_upstream(UpstreamConfig {
            name: "grafana".into(),
            transport: UpstreamTransport::Stdio {
                program: cli.grafana_mcp_bin.clone(),
                args: grafana_args,
                env: grafana_env,
            },
            prefix: Some(cli.grafana_prefix.clone()),
            client_name: Some("mcp-gateway".into()),
            client_version: Some(env!("CARGO_PKG_VERSION").into()),
        });

        match gw.connect_and_discover().await {
            Ok((tool_defs, resource_defs, prompt_defs)) => {
                for td in tool_defs {
                    builder = builder.tool_def(td);
                }
                for rd in resource_defs {
                    builder = builder.resource_def(rd);
                }
                for pd in prompt_defs {
                    builder = builder.prompt_def(pd);
                }
                tracing::info!(
                    connected = gw.connected_count(),
                    "External upstream ready: Grafana"
                );
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "Failed to connect Grafana upstream — starting without Grafana tools"
                );
            }
        }
    }

    Ok(builder.build())
}

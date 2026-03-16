# MCP Gateway — ClickHouse + Grafana

An MCP gateway server built with `mcp-kit` that aggregates multiple backends through a single endpoint:

- **ClickHouse** (internal) — direct HTTP connection, no proxy overhead
- **Grafana** (external) — proxied from [mcp-grafana](https://github.com/grafana/mcp-grafana) via `mcp-kit-gateway`

## Architecture

```
┌──────────────┐        ┌──────────────────────────────┐        ┌───────────────────┐
│  AI Agent    │        │         mcp-gateway           │ stdio  │   mcp-grafana     │
│  (Claude,    │ ─────> │                              │ ─────> │   (external)      │
│   Cursor)    │        │  ┌────────────────────────┐  │        │ - dashboards      │
│              │        │  │ ClickHouse (internal)  │  │        │ - datasources     │
│              │        │  │ direct HTTP, no proxy  │──│──>     │ - alerting        │
│              │        │  └────────────────────────┘  │   CH   │ - Prometheus/Loki │
└──────────────┘        └──────────────────────────────┘  :8123 └───────────────────┘
```

**Internal backends** are implemented directly in this binary (ClickHouse via HTTP client). No proxy overhead, no subprocess.

**External upstreams** are separate MCP servers proxied via `mcp-kit-gateway`. The gateway spawns them as subprocesses and forwards tool calls over stdio.

## Project Structure

```
src/
├── main.rs                          # Gateway composition root
├── config.rs                        # CLI (clap) + env file loading
├── domain/
│   ├── mod.rs                       # Domain layer entry
│   ├── model.rs                     # Pure types: QueryResult, TableInfo, etc.
│   └── port.rs                      # trait DatabasePort (abstract interface)
├── infrastructure/
│   ├── mod.rs                       # Infrastructure layer entry
│   └── clickhouse.rs                # ClickHouseClient (impl DatabasePort via HTTP)
└── adapter/
    ├── mod.rs                       # Adapter layer entry
    ├── tools/
    │   ├── mod.rs                   # Re-exports
    │   ├── query.rs                 # clickhouse_query tool
    │   ├── schema.rs                # clickhouse_list_tables, clickhouse_describe_table
    │   └── stats.rs                 # clickhouse_stats, clickhouse_processlist
    └── resources/
        ├── mod.rs                   # Re-exports
        └── database.rs             # clickhouse://database/{name} resource
```

## Prerequisites

- [Rust](https://rustup.rs/) 1.85+
- A running ClickHouse instance with HTTP interface enabled (default port `8123`)
- (Optional) [Grafana](https://grafana.com/) instance + [mcp-grafana](https://github.com/grafana/mcp-grafana)

## Build

```bash
cd deploy/native

# Debug build
cargo build

# Release build (optimized, smaller binary)
cargo build --release
```

The binary is output as `target/release/mcp-gateway`.

## Quick Start

```bash
# ClickHouse only
CLICKHOUSE_URL=http://localhost:8123 \
  mcp-gateway

# ClickHouse + Grafana
CLICKHOUSE_URL=http://localhost:8123 \
GRAFANA_URL=http://localhost:3000 \
GRAFANA_SERVICE_ACCOUNT_TOKEN=<your-token> \
  mcp-gateway --transport sse --port 3000
```

## Configuration

Configure via environment variables or CLI flags. CLI flags take priority over env vars.

### Environment Variables

| Variable | Default | Description |
|---|---|---|
| **ClickHouse (internal)** | | |
| `CLICKHOUSE_URL` | `http://localhost:8123` | ClickHouse HTTP interface |
| `CLICKHOUSE_USER` | `default` | Username |
| `CLICKHOUSE_PASSWORD` | *(empty)* | Password |
| `CLICKHOUSE_DATABASE` | `default` | Default database |
| **Grafana (external)** | | |
| `GRAFANA_URL` | *(none)* | Grafana URL — enables Grafana upstream |
| `GRAFANA_SERVICE_ACCOUNT_TOKEN` | *(none)* | Service account token |
| `GRAFANA_MCP_BIN` | `uvx` | mcp-grafana binary or `uvx` (auto-download) |
| `GRAFANA_PREFIX` | `grafana` | Tool name prefix |
| **General** | | |
| `RUST_LOG` | `mcp_gateway=info` | Log level (tracing filter) |

You can place these in a `.env` file in the working directory (auto-loaded on startup), or pass `--env-file /path/to/.env`.

```bash
# .env example
CLICKHOUSE_URL=http://your-clickhouse-host:8123
CLICKHOUSE_USER=your_user
CLICKHOUSE_PASSWORD=your_password
CLICKHOUSE_DATABASE=your_database
GRAFANA_URL=http://localhost:3000
GRAFANA_SERVICE_ACCOUNT_TOKEN=glsa_xxxxxxxxxxxx
```

### CLI Flags

```
mcp-gateway [OPTIONS]

Options:
  -t, --transport <TRANSPORT>  Transport: stdio, sse, ws [default: stdio]
  -p, --port <PORT>            Port for SSE/WebSocket [default: 3000]
      --env-file <FILE>        Path to .env file (overrides auto-detection)

  ClickHouse (internal):
      --url <URL>              ClickHouse HTTP URL (overrides env)
      --user <USER>            ClickHouse user (overrides env)
      --password <PASSWORD>    ClickHouse password (overrides env)
      --database <DATABASE>    ClickHouse database (overrides env)

  Grafana (external):
      --grafana-url <URL>      Grafana instance URL (enables upstream)
      --grafana-token <TOKEN>  Service account token
      --grafana-mcp-bin <BIN>  mcp-grafana binary path [default: uvx]
      --grafana-prefix <PFX>   Tool name prefix [default: grafana]

  -h, --help                   Print help
```

**Config priority** (highest to lowest): CLI flags > env vars > `.env` file > defaults

## Running

### Stdio Transport (AI Agent Integration)

```bash
# ClickHouse only
CLICKHOUSE_URL=http://localhost:8123 \
  mcp-gateway

# ClickHouse + Grafana
GRAFANA_URL=http://localhost:3000 \
GRAFANA_SERVICE_ACCOUNT_TOKEN=<your-token> \
  mcp-gateway
```

### SSE Transport (HTTP)

```bash
mcp-gateway --transport sse --port 3000
```

### WebSocket Transport

```bash
mcp-gateway --transport ws --port 3000
```

## Available Tools

### ClickHouse (internal)

| Tool | Description | Parameters |
|------|-------------|------------|
| `clickhouse_query` | Execute arbitrary SQL queries | `sql` (required), `format` (optional), `limit` (optional) |
| `clickhouse_list_tables` | List all tables with engine, row count, and size | `pattern` (optional, SQL LIKE syntax) |
| `clickhouse_describe_table` | Show column schema and row count for a table | `table_name` (required) |
| `clickhouse_stats` | Database-level statistics (tables, rows, size) | *(none)* |
| `clickhouse_processlist` | Show currently running queries | `min_elapsed_seconds` (optional) |

### Grafana (external, prefixed with `grafana/`)

When `GRAFANA_URL` is set, the gateway proxies 50+ tools from mcp-grafana:

| Tool | Description |
|------|-------------|
| `grafana/search_dashboards` | Search dashboards by title |
| `grafana/get_dashboard_by_uid` | Get full dashboard by UID |
| `grafana/get_dashboard_summary` | Compact dashboard overview |
| `grafana/list_datasources` | List all datasources |
| `grafana/query_prometheus` | Execute PromQL queries |
| `grafana/query_loki_logs` | Query Loki logs via LogQL |
| `grafana/alerting_manage_rules` | Manage alert rules |
| `grafana/list_incidents` | List Grafana Incidents |
| `grafana/list_oncall_schedules` | OnCall schedules |
| `grafana/generate_deeplink` | Generate Grafana URLs |
| ... | [Full list](https://github.com/grafana/mcp-grafana#tools) |

### Resources

| URI | Description |
|-----|-------------|
| `clickhouse://database/{name}` | Database connection info and summary statistics (JSON) |

## Grafana Setup

### Prerequisites

1. **Grafana instance** (local or Grafana Cloud) with a Service Account Token:
   - Grafana → Administration → Service Accounts → Add service account
   - Assign `Editor` role (or fine-grained RBAC)
   - Generate a token

2. **mcp-grafana** — install via one of:
   ```bash
   # Option A: uvx (default, auto-downloads — requires uv)
   # Nothing to install, just set GRAFANA_MCP_BIN=uvx (default)

   # Option B: Go install
   go install github.com/grafana/mcp-grafana/cmd/mcp-grafana@latest

   # Option C: Download binary
   # https://github.com/grafana/mcp-grafana/releases
   ```

### Error Handling

- If mcp-grafana fails to start or connect, the gateway logs a warning and starts with only ClickHouse tools.
- ClickHouse tools always work regardless of Grafana upstream status.

## LLM / AI Agent Integration

> **Tip:** Build a release binary first:
> ```bash
> cd deploy/native && cargo build --release
> # Binary at: target/release/mcp-gateway
> ```

### Claude Desktop

File: `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS)
or `%APPDATA%\Claude\claude_desktop_config.json` (Windows)

```json
{
  "mcpServers": {
    "gateway": {
      "command": "/absolute/path/to/mcp-gateway",
      "env": {
        "CLICKHOUSE_URL": "http://localhost:8123",
        "CLICKHOUSE_USER": "default",
        "CLICKHOUSE_DATABASE": "default",
        "GRAFANA_URL": "http://localhost:3000",
        "GRAFANA_SERVICE_ACCOUNT_TOKEN": "<your-token>"
      }
    }
  }
}
```

### Cursor

File: `.cursor/mcp.json` in your project root or `~/.cursor/mcp.json` (global)

```json
{
  "mcpServers": {
    "gateway": {
      "command": "/absolute/path/to/mcp-gateway",
      "env": {
        "CLICKHOUSE_URL": "http://localhost:8123",
        "GRAFANA_URL": "http://localhost:3000",
        "GRAFANA_SERVICE_ACCOUNT_TOKEN": "<your-token>"
      }
    }
  }
}
```

### Windsurf

File: `~/.codeium/windsurf/mcp_config.json`

```json
{
  "mcpServers": {
    "gateway": {
      "command": "/absolute/path/to/mcp-gateway",
      "args": ["--env-file", "/absolute/path/to/.env"]
    }
  }
}
```

### VS Code (GitHub Copilot)

File: `.vscode/mcp.json` in your project root

```json
{
  "servers": {
    "gateway": {
      "command": "/absolute/path/to/mcp-gateway",
      "args": ["--env-file", "/absolute/path/to/.env"]
    }
  }
}
```

### Zed

File: `~/.config/zed/settings.json`

```json
{
  "context_servers": {
    "gateway": {
      "command": {
        "path": "/absolute/path/to/mcp-gateway",
        "args": ["--env-file", "/absolute/path/to/.env"],
        "env": {}
      }
    }
  }
}
```

### Continue.dev

File: `~/.continue/config.yaml`

```yaml
mcpServers:
  - name: gateway
    command: /absolute/path/to/mcp-gateway
    args:
      - --env-file
      - /absolute/path/to/.env
```

### SSE Mode (Remote / Web Clients)

```bash
mcp-gateway --transport sse --port 3000
```

```json
{
  "mcpServers": {
    "gateway": {
      "url": "http://localhost:3000/sse"
    }
  }
}
```

### What the AI Agent Can Do

| What you can ask | Tool used |
|---|---|
| "List all tables in the database" | `clickhouse_list_tables` |
| "Describe the schema of table X" | `clickhouse_describe_table` |
| "Show me the top 10 orders" | `clickhouse_query` |
| "What's the total database size?" | `clickhouse_stats` |
| "Search for dashboards about revenue" | `grafana/search_dashboards` |
| "Query Prometheus for CPU usage" | `grafana/query_prometheus` |
| "Show me Loki logs with errors" | `grafana/query_loki_logs` |
| "List alert rules" | `grafana/alerting_manage_rules` |

## Testing with curl (SSE Transport)

```bash
# Start the gateway
mcp-gateway --transport sse --port 3000

# In another terminal — open SSE stream (keep running)
curl -N http://localhost:3000/sse
# Output: event: endpoint
#         data: /message?sessionId=<SESSION_ID>

# Initialize (replace <SESSION_ID>)
curl -X POST "http://localhost:3000/message?sessionId=<SESSION_ID>" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0", "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2024-11-05",
      "capabilities": {},
      "clientInfo": { "name": "test", "version": "1.0" }
    }
  }'

# Send initialized notification
curl -X POST "http://localhost:3000/message?sessionId=<SESSION_ID>" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc": "2.0", "method": "notifications/initialized"}'

# Call a ClickHouse tool (internal)
curl -X POST "http://localhost:3000/message?sessionId=<SESSION_ID>" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0", "id": 2,
    "method": "tools/call",
    "params": { "name": "clickhouse_list_tables", "arguments": {} }
  }'

# Call a Grafana tool (external, proxied)
curl -X POST "http://localhost:3000/message?sessionId=<SESSION_ID>" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0", "id": 3,
    "method": "tools/call",
    "params": { "name": "grafana/search_dashboards", "arguments": {"query": ""} }
  }'
```

Responses are delivered via the SSE stream, not the POST response body.

## Logging

```bash
# Debug logging
RUST_LOG=mcp_gateway=debug cargo run

# Debug with mcp-kit internals
RUST_LOG=mcp_gateway=debug,mcp_kit=debug,mcp_kit_gateway=debug cargo run

# Quiet mode
RUST_LOG=warn cargo run
```

## Notes

- Use fully-qualified table names (e.g. `mydb.my_table`) when querying tables outside the configured default database.
- The `clickhouse_stats` tool queries `system.parts`, which requires `SELECT` privileges on that system table.
- The server performs a ClickHouse health check (`SELECT 1`) on startup. If it fails, the server still starts but logs a warning.

## License

MIT License - See [LICENSE](../../LICENSE) for details.

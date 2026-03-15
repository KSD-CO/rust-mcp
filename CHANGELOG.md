# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-03-15

### Added

#### 🧩 Plugin System
- **Plugin trait and infrastructure** - Core `McpPlugin` trait for dynamic tool/resource/prompt loading
- **PluginManager** - Lifecycle management, configuration, and priority-based loading
- **Native plugin loading** - Load plugins from shared libraries (.so, .dylib, .dll)
- **WASM plugin support** - Skeleton for WebAssembly plugin loading (coming soon)
- **Plugin configuration** - JSON-based config with permissions system
- **Plugin lifecycle hooks** - `on_load`, `on_unload`, `can_unload` callbacks
- **Builder integration** - `.with_plugin_manager()`, `.load_plugin()` methods
- **Plugin metadata** - Track plugin info, tool/resource/prompt counts

#### 📦 Production-Ready Plugin Examples
- **GitHub Plugin** (`plugin_github.rs`) - 4 working tools with GitHub REST API v3
  - Get repository info
  - List repositories
  - Create issues
  - List pull requests
- **Jira Plugin** (`plugin_jira.rs`) - 4 working tools with Jira REST API v3
  - Create issues
  - Get issue details
  - Search with JQL
  - Add comments
- **Confluence Plugin** (`plugin_confluence.rs`) - 4 working tools with Confluence REST API
  - Create pages
  - Get page content
  - Search with CQL
  - List pages in space
- **ClickHouse Plugin** (`plugin_clickhouse.rs`) - 6 working tools with ClickHouse HTTP interface
  - Execute SQL queries
  - Get table schema and stats
  - Generate analytics reports (daily/hourly/top users)
  - List all tables
  - Database statistics
  - Insert data
- **Weather Plugin** (`plugin_weather.rs`) - Demo plugin with mock API

#### 📚 Documentation
- **Plugin System Guide** (`docs/PLUGINS.md`) - Complete plugin development guide
- **Plugin Examples Guide** (`examples/PLUGINS.md`) - Detailed examples with setup instructions
- **GitHub Plugin Guide** (`examples/GITHUB_REAL_PLUGIN.md`) - GitHub API integration guide

#### 🔧 New Features
- **Feature flags** - `plugin`, `plugin-native`, `plugin-wasm`, `plugin-hot-reload`
- **Dynamic tool registration** - Register tools from plugins at runtime
- **Plugin priorities** - Control plugin load order
- **Plugin permissions** - Network, filesystem, env, process permissions

### Changed
- Updated `full` feature to include `plugin` and `plugin-native`
- Enhanced `McpServerBuilder` with plugin methods
- Updated README with comprehensive plugin documentation

### Dependencies
- Added `libloading = "0.8"` for native plugin loading
- Added `wasmtime = "26"` for WASM support (optional)
- Added `wasmtime-wasi = "26"` for WASI support (optional)
- Added `notify = "7"` for hot reload (optional)
- Added `reqwest = "0.12"` (dev) for real API examples
- Added `base64 = "0.22"` (dev) for Basic Auth
- Added `urlencoding = "2.1"` (dev) for URL encoding

## [0.1.6] - Previous Release

### Features
- Core MCP server functionality
- Multiple transports (stdio, SSE, WebSocket)
- Authentication (Bearer, API Key, Basic, OAuth 2.0, mTLS)
- Progress tracking
- Notifications and subscriptions
- Elicitation
- Client SDK

---

[0.2.0]: https://github.com/KSD-CO/mcp-kit/compare/v0.1.6...v0.2.0
[0.1.6]: https://github.com/KSD-CO/mcp-kit/releases/tag/v0.1.6

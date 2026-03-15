//! Confluence Plugin - Real API Implementation
//!
//! WORKING Confluence plugin with real Atlassian Confluence REST API integration.
//!
//! ## Setup
//!
//! 1. Get Confluence API token:
//!    https://id.atlassian.com/manage-profile/security/api-tokens
//!
//! 2. Set environment variables:
//!    ```bash
//!    export CONFLUENCE_BASE_URL="https://your-domain.atlassian.net"
//!    export CONFLUENCE_EMAIL="your-email@example.com"
//!    export CONFLUENCE_API_TOKEN="your-api-token"
//!    export CONFLUENCE_SPACE_KEY="TEAM"
//!    ```
//!
//! 3. Run:
//!    ```bash
//!    cargo run --example plugin_confluence --features plugin,plugin-native
//!    ```

use base64::Engine;
use mcp_kit::plugin::{McpPlugin, PluginConfig, ToolDefinition};
use mcp_kit::prelude::*;
use serde::Deserialize;

// ─── Confluence Plugin ───────────────────────────────────────────────────────

/// Confluence integration with REAL API
struct ConfluencePlugin {
    base_url: String,
    email: String,
    api_token: String,
    default_space: String,
    client: reqwest::Client,
}

impl ConfluencePlugin {
    pub fn new() -> Self {
        Self {
            base_url: String::new(),
            email: String::new(),
            api_token: String::new(),
            default_space: String::new(),
            client: reqwest::Client::new(),
        }
    }

    /// Make REAL request to Confluence REST API
    async fn api_request(
        &self,
        endpoint: &str,
        method: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}{}", self.base_url, endpoint);

        tracing::debug!("Confluence API: {} {}", method, url);

        let auth = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", self.email, self.api_token));

        let mut request = match method {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            _ => return Err("Invalid HTTP method".into()),
        };

        request = request
            .header("Authorization", format!("Basic {}", auth))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json");

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(format!("Confluence API error {}: {}", status, error_text).into());
        }

        let json: serde_json::Value = response.json().await?;
        Ok(json)
    }
}

impl Clone for ConfluencePlugin {
    fn clone(&self) -> Self {
        Self {
            base_url: self.base_url.clone(),
            email: self.email.clone(),
            api_token: self.api_token.clone(),
            default_space: self.default_space.clone(),
            client: reqwest::Client::new(),
        }
    }
}

impl McpPlugin for ConfluencePlugin {
    fn name(&self) -> &str {
        "confluence"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> Option<&str> {
        Some("Atlassian Confluence integration with REAL API - manage wiki pages")
    }

    fn author(&self) -> Option<&str> {
        Some("MCP Kit Team")
    }

    fn register_tools(&self) -> Vec<ToolDefinition> {
        vec![
            // Create page
            ToolDefinition::new(
                Tool::new(
                    "confluence_create_page",
                    "Create a new Confluence page",
                    serde_json::to_value(schemars::schema_for!(CreatePageInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: CreatePageInput| {
                        let plugin = plugin.clone();
                        async move {
                            let space =
                                params.space.unwrap_or_else(|| plugin.default_space.clone());

                            let body = serde_json::json!({
                                "type": "page",
                                "title": params.title,
                                "space": { "key": space },
                                "body": {
                                    "storage": {
                                        "value": params.content,
                                        "representation": "storage"
                                    }
                                }
                            });

                            match plugin
                                .api_request("/wiki/rest/api/content", "POST", Some(body))
                                .await
                            {
                                Ok(data) => {
                                    let id = data["id"].as_str().unwrap_or("unknown");
                                    let url = format!(
                                        "{}/wiki/spaces/{}/pages/{}",
                                        plugin.base_url, space, id
                                    );

                                    CallToolResult::text(format!(
                                        "✅ Created page: {}\n\
                                         Space: {}\n\
                                         URL: {}",
                                        params.title, space, url
                                    ))
                                }
                                Err(e) => {
                                    CallToolResult::error(format!("Failed to create page: {}", e))
                                }
                            }
                        }
                    }
                },
            ),
            // Get page
            ToolDefinition::new(
                Tool::new(
                    "confluence_get_page",
                    "Get content of a Confluence page",
                    serde_json::to_value(schemars::schema_for!(GetPageInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: GetPageInput| {
                        let plugin = plugin.clone();
                        async move {
                            match plugin
                                .api_request(
                                    &format!(
                                        "/wiki/rest/api/content/{}?expand=body.storage,version",
                                        params.page_id
                                    ),
                                    "GET",
                                    None,
                                )
                                .await
                            {
                                Ok(data) => {
                                    let title = data["title"].as_str().unwrap_or("No title");
                                    let version = data["version"]["number"].as_u64().unwrap_or(0);
                                    let content = data["body"]["storage"]["value"]
                                        .as_str()
                                        .unwrap_or("No content");

                                    CallToolResult::text(format!(
                                        "📄 Page: {}\n\
                                         Version: {}\n\
                                         URL: {}/wiki/spaces/{}/pages/{}\n\n\
                                         Content preview:\n{}",
                                        title,
                                        version,
                                        plugin.base_url,
                                        plugin.default_space,
                                        params.page_id,
                                        &content[..content.len().min(500)]
                                    ))
                                }
                                Err(e) => {
                                    CallToolResult::error(format!("Failed to get page: {}", e))
                                }
                            }
                        }
                    }
                },
            ),
            // Search content
            ToolDefinition::new(
                Tool::new(
                    "confluence_search",
                    "Search Confluence content using CQL",
                    serde_json::to_value(schemars::schema_for!(SearchInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: SearchInput| {
                        let plugin = plugin.clone();
                        async move {
                            let cql = params.cql.unwrap_or_else(|| {
                                format!("type=page and space={}", plugin.default_space)
                            });

                            let query = urlencoding::encode(&cql);

                            match plugin
                                .api_request(
                                    &format!(
                                        "/wiki/rest/api/content/search?cql={}&limit={}",
                                        query, params.limit
                                    ),
                                    "GET",
                                    None,
                                )
                                .await
                            {
                                Ok(data) => {
                                    if let Some(results) = data["results"].as_array() {
                                        let page_list: Vec<String> = results
                                            .iter()
                                            .map(|page| {
                                                let title =
                                                    page["title"].as_str().unwrap_or("No title");
                                                let id = page["id"].as_str().unwrap_or("unknown");
                                                format!("📄 {} (ID: {})", title, id)
                                            })
                                            .collect();

                                        CallToolResult::text(format!(
                                            "🔍 Search results:\n\n{}",
                                            if page_list.is_empty() {
                                                "No pages found".to_string()
                                            } else {
                                                page_list.join("\n")
                                            }
                                        ))
                                    } else {
                                        CallToolResult::error("Invalid response format")
                                    }
                                }
                                Err(e) => CallToolResult::error(format!("Search failed: {}", e)),
                            }
                        }
                    }
                },
            ),
            // List pages in space
            ToolDefinition::new(
                Tool::new(
                    "confluence_list_pages",
                    "List pages in a space",
                    serde_json::to_value(schemars::schema_for!(ListPagesInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: ListPagesInput| {
                        let plugin = plugin.clone();
                        async move {
                            let space =
                                params.space.unwrap_or_else(|| plugin.default_space.clone());

                            match plugin
                                .api_request(
                                    &format!(
                                        "/wiki/rest/api/space/{}/content?limit={}",
                                        space, params.limit
                                    ),
                                    "GET",
                                    None,
                                )
                                .await
                            {
                                Ok(data) => {
                                    if let Some(pages) = data["page"]["results"].as_array() {
                                        let page_list: Vec<String> = pages
                                            .iter()
                                            .map(|page| {
                                                let title =
                                                    page["title"].as_str().unwrap_or("No title");
                                                let id = page["id"].as_str().unwrap_or("unknown");
                                                format!("• {} ({})", title, id)
                                            })
                                            .collect();

                                        CallToolResult::text(format!(
                                            "📚 Pages in space '{}':\n\n{}",
                                            space,
                                            if page_list.is_empty() {
                                                "No pages found".to_string()
                                            } else {
                                                page_list.join("\n")
                                            }
                                        ))
                                    } else {
                                        CallToolResult::error("Invalid response format")
                                    }
                                }
                                Err(e) => {
                                    CallToolResult::error(format!("Failed to list pages: {}", e))
                                }
                            }
                        }
                    }
                },
            ),
        ]
    }

    fn on_load(&mut self, config: &PluginConfig) -> McpResult<()> {
        self.base_url = config
            .config
            .get("base_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("base_url required"))?
            .to_string();

        self.email = config
            .config
            .get("email")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("email required"))?
            .to_string();

        self.api_token = config
            .config
            .get("api_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("api_token required"))?
            .to_string();

        self.default_space = config
            .config
            .get("default_space")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("default_space required"))?
            .to_string();

        tracing::info!(
            "✅ Confluence plugin loaded: {} ({})",
            self.base_url,
            self.default_space
        );
        Ok(())
    }

    fn on_unload(&mut self) -> McpResult<()> {
        tracing::info!("Confluence plugin unloaded");
        Ok(())
    }
}

// ─── Input Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct CreatePageInput {
    /// Page title
    title: String,
    /// Page content (Confluence storage format)
    content: String,
    /// Space key (optional)
    space: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GetPageInput {
    /// Page ID
    page_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchInput {
    /// CQL query (optional)
    cql: Option<String>,
    /// Maximum results
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    25
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ListPagesInput {
    /// Space key (optional)
    space: Option<String>,
    /// Maximum results
    #[serde(default = "default_limit")]
    limit: u32,
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("plugin_confluence=debug,mcp_kit=info")
        .init();

    tracing::info!("🚀 Starting Confluence plugin with REAL API");

    let mut plugin_manager = mcp_kit::plugin::PluginManager::new();

    let config = mcp_kit::plugin::PluginConfig {
        config: serde_json::json!({
            "base_url": std::env::var("CONFLUENCE_BASE_URL")
                .unwrap_or_else(|_| {
                    tracing::warn!("CONFLUENCE_BASE_URL not set");
                    "https://your-domain.atlassian.net".to_string()
                }),
            "email": std::env::var("CONFLUENCE_EMAIL")
                .unwrap_or_else(|_| {
                    tracing::warn!("CONFLUENCE_EMAIL not set");
                    "your-email@example.com".to_string()
                }),
            "api_token": std::env::var("CONFLUENCE_API_TOKEN")
                .unwrap_or_else(|_| {
                    tracing::warn!("CONFLUENCE_API_TOKEN not set");
                    "your-api-token".to_string()
                }),
            "default_space": std::env::var("CONFLUENCE_SPACE_KEY")
                .unwrap_or_else(|_| "TEAM".to_string()),
        }),
        enabled: true,
        priority: 0,
        permissions: mcp_kit::plugin::PluginPermissions {
            network: true,
            ..Default::default()
        },
    };

    plugin_manager.register_plugin(ConfluencePlugin::new(), config)?;

    let plugins = plugin_manager.list_plugins();
    for plugin in plugins {
        tracing::info!(
            "📦 Plugin: {} v{} ({} tools)",
            plugin.name,
            plugin.version,
            plugin.tool_count
        );
    }

    let server = McpServer::builder()
        .name("confluence-real-server")
        .version("1.0.0")
        .instructions(
            "Confluence integration with REAL API.\n\n\
             Tools:\n\
             - confluence_create_page: Create pages\n\
             - confluence_get_page: Get page content\n\
             - confluence_search: Search with CQL\n\
             - confluence_list_pages: List pages\n\n\
             Set environment variables:\n\
             - CONFLUENCE_BASE_URL\n\
             - CONFLUENCE_EMAIL\n\
             - CONFLUENCE_API_TOKEN\n\
             - CONFLUENCE_SPACE_KEY",
        )
        .with_plugin_manager(plugin_manager)
        .build();

    tracing::info!("✅ Server ready");

    server.serve_stdio().await?;
    Ok(())
}

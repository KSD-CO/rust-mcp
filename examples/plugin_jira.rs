//! Jira Plugin - Real API Implementation
//!
//! WORKING Jira plugin with real Atlassian Jira REST API integration.
//!
//! ## Setup
//!
//! 1. Get Jira API token:
//!    https://id.atlassian.com/manage-profile/security/api-tokens
//!
//! 2. Set environment variables:
//!    ```bash
//!    export JIRA_BASE_URL="https://your-domain.atlassian.net"
//!    export JIRA_EMAIL="your-email@example.com"
//!    export JIRA_API_TOKEN="your-api-token"
//!    export JIRA_PROJECT_KEY="PROJ"
//!    ```
//!
//! 3. Run:
//!    ```bash
//!    cargo run --example plugin_jira --features plugin,plugin-native
//!    ```

use base64::Engine;
use mcp_kit::plugin::{McpPlugin, PluginConfig, ToolDefinition};
use mcp_kit::prelude::*;
use serde::Deserialize;

// ─── Jira Plugin ─────────────────────────────────────────────────────────────

/// Jira integration with REAL API
struct JiraPlugin {
    base_url: String,
    email: String,
    api_token: String,
    project_key: String,
    client: reqwest::Client,
}

impl JiraPlugin {
    pub fn new() -> Self {
        Self {
            base_url: String::new(),
            email: String::new(),
            api_token: String::new(),
            project_key: String::new(),
            client: reqwest::Client::new(),
        }
    }

    /// Make REAL request to Jira REST API v3
    async fn api_request(
        &self,
        endpoint: &str,
        method: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}{}", self.base_url, endpoint);

        tracing::debug!("Jira API: {} {}", method, url);

        // Create Basic Auth
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
            return Err(format!("Jira API error {}: {}", status, error_text).into());
        }

        let json: serde_json::Value = response.json().await?;
        Ok(json)
    }
}

impl Clone for JiraPlugin {
    fn clone(&self) -> Self {
        Self {
            base_url: self.base_url.clone(),
            email: self.email.clone(),
            api_token: self.api_token.clone(),
            project_key: self.project_key.clone(),
            client: reqwest::Client::new(),
        }
    }
}

impl McpPlugin for JiraPlugin {
    fn name(&self) -> &str {
        "jira"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> Option<&str> {
        Some("Atlassian Jira integration with REAL API - create and manage issues")
    }

    fn author(&self) -> Option<&str> {
        Some("MCP Kit Team")
    }

    fn register_tools(&self) -> Vec<ToolDefinition> {
        vec![
            // Create issue
            ToolDefinition::new(
                Tool::new(
                    "jira_create_issue",
                    "Create a new Jira issue",
                    serde_json::to_value(schemars::schema_for!(CreateIssueInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: CreateIssueInput| {
                        let plugin = plugin.clone();
                        async move {
                            let body = serde_json::json!({
                                "fields": {
                                    "project": { "key": plugin.project_key },
                                    "summary": params.summary,
                                    "description": {
                                        "type": "doc",
                                        "version": 1,
                                        "content": [{
                                            "type": "paragraph",
                                            "content": [{
                                                "type": "text",
                                                "text": params.description
                                            }]
                                        }]
                                    },
                                    "issuetype": { "name": params.issue_type },
                                }
                            });

                            match plugin
                                .api_request("/rest/api/3/issue", "POST", Some(body))
                                .await
                            {
                                Ok(data) => {
                                    let key = data["key"].as_str().unwrap_or("unknown");
                                    let issue_url = format!("{}/browse/{}", plugin.base_url, key);

                                    CallToolResult::text(format!(
                                        "✅ Created Jira issue: {}\n\
                                         Summary: {}\n\
                                         Type: {}\n\
                                         URL: {}",
                                        key, params.summary, params.issue_type, issue_url
                                    ))
                                }
                                Err(e) => {
                                    CallToolResult::error(format!("Failed to create issue: {}", e))
                                }
                            }
                        }
                    }
                },
            ),
            // Get issue
            ToolDefinition::new(
                Tool::new(
                    "jira_get_issue",
                    "Get details of a Jira issue",
                    serde_json::to_value(schemars::schema_for!(GetIssueInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: GetIssueInput| {
                        let plugin = plugin.clone();
                        async move {
                            match plugin
                                .api_request(
                                    &format!("/rest/api/3/issue/{}", params.issue_key),
                                    "GET",
                                    None,
                                )
                                .await
                            {
                                Ok(data) => {
                                    let key = data["key"].as_str().unwrap_or("unknown");
                                    let summary =
                                        data["fields"]["summary"].as_str().unwrap_or("No summary");
                                    let status = data["fields"]["status"]["name"]
                                        .as_str()
                                        .unwrap_or("Unknown");
                                    let issue_type = data["fields"]["issuetype"]["name"]
                                        .as_str()
                                        .unwrap_or("Unknown");
                                    let assignee = data["fields"]["assignee"]["displayName"]
                                        .as_str()
                                        .unwrap_or("Unassigned");

                                    CallToolResult::text(format!(
                                        "📋 Issue: {}\n\
                                         Summary: {}\n\
                                         Type: {}\n\
                                         Status: {}\n\
                                         Assignee: {}\n\
                                         URL: {}/browse/{}",
                                        key,
                                        summary,
                                        issue_type,
                                        status,
                                        assignee,
                                        plugin.base_url,
                                        key
                                    ))
                                }
                                Err(e) => {
                                    CallToolResult::error(format!("Failed to get issue: {}", e))
                                }
                            }
                        }
                    }
                },
            ),
            // Search issues
            ToolDefinition::new(
                Tool::new(
                    "jira_search",
                    "Search Jira issues using JQL",
                    serde_json::to_value(schemars::schema_for!(SearchIssuesInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: SearchIssuesInput| {
                        let plugin = plugin.clone();
                        async move {
                            let jql = params.jql.unwrap_or_else(|| {
                                format!("project = {} ORDER BY created DESC", plugin.project_key)
                            });

                            let query = urlencoding::encode(&jql);

                            match plugin
                                .api_request(
                                    &format!(
                                        "/rest/api/3/search?jql={}&maxResults={}",
                                        query, params.max_results
                                    ),
                                    "GET",
                                    None,
                                )
                                .await
                            {
                                Ok(data) => {
                                    if let Some(issues) = data["issues"].as_array() {
                                        let issue_list: Vec<String> = issues
                                            .iter()
                                            .map(|issue| {
                                                let key =
                                                    issue["key"].as_str().unwrap_or("unknown");
                                                let summary = issue["fields"]["summary"]
                                                    .as_str()
                                                    .unwrap_or("No summary");
                                                let status = issue["fields"]["status"]["name"]
                                                    .as_str()
                                                    .unwrap_or("Unknown");
                                                format!("{}: {} ({})", key, summary, status)
                                            })
                                            .collect();

                                        CallToolResult::text(format!(
                                            "🔍 Found {} issues:\n\n{}",
                                            issues.len(),
                                            if issue_list.is_empty() {
                                                "No issues found".to_string()
                                            } else {
                                                issue_list.join("\n")
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
            // Add comment
            ToolDefinition::new(
                Tool::new(
                    "jira_add_comment",
                    "Add a comment to a Jira issue",
                    serde_json::to_value(schemars::schema_for!(AddCommentInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: AddCommentInput| {
                        let plugin = plugin.clone();
                        async move {
                            let body = serde_json::json!({
                                "body": {
                                    "type": "doc",
                                    "version": 1,
                                    "content": [{
                                        "type": "paragraph",
                                        "content": [{
                                            "type": "text",
                                            "text": params.comment
                                        }]
                                    }]
                                }
                            });

                            match plugin
                                .api_request(
                                    &format!("/rest/api/3/issue/{}/comment", params.issue_key),
                                    "POST",
                                    Some(body),
                                )
                                .await
                            {
                                Ok(_) => CallToolResult::text(format!(
                                    "💬 Added comment to {}",
                                    params.issue_key
                                )),
                                Err(e) => {
                                    CallToolResult::error(format!("Failed to add comment: {}", e))
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

        self.project_key = config
            .config
            .get("project_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("project_key required"))?
            .to_string();

        tracing::info!(
            "✅ Jira plugin loaded: {} ({})",
            self.base_url,
            self.project_key
        );
        Ok(())
    }

    fn on_unload(&mut self) -> McpResult<()> {
        tracing::info!("Jira plugin unloaded");
        Ok(())
    }
}

// ─── Input Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct CreateIssueInput {
    /// Issue summary/title
    summary: String,
    /// Issue description
    description: String,
    /// Issue type (Story, Task, Bug, Epic)
    #[serde(default = "default_issue_type")]
    issue_type: String,
}

fn default_issue_type() -> String {
    "Task".to_string()
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GetIssueInput {
    /// Issue key (e.g., PROJ-123)
    issue_key: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchIssuesInput {
    /// JQL query (optional)
    jql: Option<String>,
    /// Maximum results
    #[serde(default = "default_max_results")]
    max_results: u32,
}

fn default_max_results() -> u32 {
    20
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AddCommentInput {
    /// Issue key
    issue_key: String,
    /// Comment text
    comment: String,
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("plugin_jira=debug,mcp_kit=info")
        .init();

    tracing::info!("🚀 Starting Jira plugin with REAL API");

    let mut plugin_manager = mcp_kit::plugin::PluginManager::new();

    let config = mcp_kit::plugin::PluginConfig {
        config: serde_json::json!({
            "base_url": std::env::var("JIRA_BASE_URL")
                .unwrap_or_else(|_| {
                    tracing::warn!("JIRA_BASE_URL not set");
                    "https://your-domain.atlassian.net".to_string()
                }),
            "email": std::env::var("JIRA_EMAIL")
                .unwrap_or_else(|_| {
                    tracing::warn!("JIRA_EMAIL not set");
                    "your-email@example.com".to_string()
                }),
            "api_token": std::env::var("JIRA_API_TOKEN")
                .unwrap_or_else(|_| {
                    tracing::warn!("JIRA_API_TOKEN not set");
                    "your-api-token".to_string()
                }),
            "project_key": std::env::var("JIRA_PROJECT_KEY")
                .unwrap_or_else(|_| "DEMO".to_string()),
        }),
        enabled: true,
        priority: 0,
        permissions: mcp_kit::plugin::PluginPermissions {
            network: true,
            ..Default::default()
        },
    };

    plugin_manager.register_plugin(JiraPlugin::new(), config)?;

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
        .name("jira-real-server")
        .version("1.0.0")
        .instructions(
            "Jira integration with REAL API.\n\n\
             Tools:\n\
             - jira_create_issue: Create issues\n\
             - jira_get_issue: Get issue details\n\
             - jira_search: Search with JQL\n\
             - jira_add_comment: Add comments\n\n\
             Set environment variables:\n\
             - JIRA_BASE_URL\n\
             - JIRA_EMAIL\n\
             - JIRA_API_TOKEN\n\
             - JIRA_PROJECT_KEY",
        )
        .with_plugin_manager(plugin_manager)
        .build();

    tracing::info!("✅ Server ready");

    server.serve_stdio().await?;
    Ok(())
}

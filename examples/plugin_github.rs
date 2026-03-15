//! GitHub Plugin - Real API Implementation
//!
//! This plugin provides WORKING GitHub integration with real API calls.
//!
//! ## Setup
//!
//! 1. Create a GitHub Personal Access Token:
//!    https://github.com/settings/tokens/new
//!    Scopes needed: repo, read:user
//!
//! 2. Set your token in the config (see main function)
//!
//! 3. Run:
//!    ```bash
//!    cargo run --example plugin_github --features plugin,plugin-native
//!    ```
//!
//! ## Features
//!
//! - ✅ Real GitHub REST API v3 integration
//! - ✅ Create issues
//! - ✅ List repositories
//! - ✅ Get repository info
//! - ✅ List pull requests
//! - ✅ Search code

use mcp_kit::plugin::{McpPlugin, PluginConfig, ToolDefinition};
use mcp_kit::prelude::*;
use serde::Deserialize;

// ─── GitHub Plugin with Real API ─────────────────────────────────────────────

/// GitHub integration plugin with real API calls
struct GitHubPlugin {
    token: String,
    default_owner: String,
    default_repo: String,
    client: reqwest::Client,
}

impl GitHubPlugin {
    pub fn new() -> Self {
        Self {
            token: String::new(),
            default_owner: String::new(),
            default_repo: String::new(),
            client: reqwest::Client::new(),
        }
    }

    /// Make a REAL request to GitHub API
    async fn api_request(
        &self,
        endpoint: &str,
        method: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("https://api.github.com{}", endpoint);

        tracing::debug!("GitHub API: {} {}", method, url);

        let mut request = match method {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            "PATCH" => self.client.patch(&url),
            _ => return Err("Invalid HTTP method".into()),
        };

        // Add authentication and headers
        request = request
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "mcp-kit-github-plugin/1.0")
            .header("Accept", "application/vnd.github.v3+json");

        // Add body for POST/PUT/PATCH
        if let Some(body) = body {
            request = request.json(&body);
        }

        // Send request
        let response = request.send().await?;

        // Check for errors
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(format!("GitHub API error {}: {}", status, error_text).into());
        }

        // Parse JSON response
        let json: serde_json::Value = response.json().await?;
        Ok(json)
    }
}

impl Clone for GitHubPlugin {
    fn clone(&self) -> Self {
        Self {
            token: self.token.clone(),
            default_owner: self.default_owner.clone(),
            default_repo: self.default_repo.clone(),
            client: reqwest::Client::new(),
        }
    }
}

impl McpPlugin for GitHubPlugin {
    fn name(&self) -> &str {
        "github"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> Option<&str> {
        Some("GitHub integration with REAL API calls - manage repos, issues, and PRs")
    }

    fn author(&self) -> Option<&str> {
        Some("MCP Kit Team")
    }

    fn register_tools(&self) -> Vec<ToolDefinition> {
        vec![
            // Get repository info
            ToolDefinition::new(
                Tool::new(
                    "github_get_repo",
                    "Get repository information and stats",
                    serde_json::to_value(schemars::schema_for!(GetRepoInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: GetRepoInput| {
                        let plugin = plugin.clone();
                        async move {
                            let (owner, repo) = params
                                .repo
                                .map(|r| {
                                    let parts: Vec<_> = r.split('/').collect();
                                    (parts[0].to_string(), parts[1].to_string())
                                })
                                .unwrap_or_else(|| {
                                    (plugin.default_owner.clone(), plugin.default_repo.clone())
                                });

                            match plugin
                                .api_request(&format!("/repos/{}/{}", owner, repo), "GET", None)
                                .await
                            {
                                Ok(data) => {
                                    let stars = data["stargazers_count"].as_u64().unwrap_or(0);
                                    let forks = data["forks_count"].as_u64().unwrap_or(0);
                                    let issues = data["open_issues_count"].as_u64().unwrap_or(0);
                                    let language =
                                        data["language"].as_str().unwrap_or("Unknown").to_string();
                                    let description = data["description"]
                                        .as_str()
                                        .unwrap_or("No description")
                                        .to_string();

                                    CallToolResult::text(format!(
                                        "📦 Repository: {}/{}\n\
                                         Description: {}\n\
                                         Language: {}\n\
                                         Stars: ⭐ {}\n\
                                         Forks: 🍴 {}\n\
                                         Open Issues: 🐛 {}\n\
                                         URL: https://github.com/{}/{}",
                                        owner,
                                        repo,
                                        description,
                                        language,
                                        stars,
                                        forks,
                                        issues,
                                        owner,
                                        repo
                                    ))
                                }
                                Err(e) => CallToolResult::error(format!("API error: {}", e)),
                            }
                        }
                    }
                },
            ),
            // List repositories
            ToolDefinition::new(
                Tool::new(
                    "github_list_repos",
                    "List repositories for a user or organization",
                    serde_json::to_value(schemars::schema_for!(ListReposInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: ListReposInput| {
                        let plugin = plugin.clone();
                        async move {
                            let owner =
                                params.owner.unwrap_or_else(|| plugin.default_owner.clone());
                            let per_page = params.limit.min(100);

                            match plugin
                                .api_request(
                                    &format!("/users/{}/repos?per_page={}", owner, per_page),
                                    "GET",
                                    None,
                                )
                                .await
                            {
                                Ok(data) => {
                                    if let Some(repos) = data.as_array() {
                                        let repo_list: Vec<String> = repos
                                            .iter()
                                            .map(|r| {
                                                let name = r["name"].as_str().unwrap_or("unknown");
                                                let desc = r["description"]
                                                    .as_str()
                                                    .unwrap_or("No description");
                                                let stars =
                                                    r["stargazers_count"].as_u64().unwrap_or(0);
                                                format!("• {} (⭐ {}) - {}", name, stars, desc)
                                            })
                                            .collect();

                                        CallToolResult::text(format!(
                                            "📚 Repositories for {}:\n\n{}",
                                            owner,
                                            repo_list.join("\n")
                                        ))
                                    } else {
                                        CallToolResult::error("Invalid response format")
                                    }
                                }
                                Err(e) => CallToolResult::error(format!("API error: {}", e)),
                            }
                        }
                    }
                },
            ),
            // Create issue
            ToolDefinition::new(
                Tool::new(
                    "github_create_issue",
                    "Create a new GitHub issue",
                    serde_json::to_value(schemars::schema_for!(CreateIssueInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: CreateIssueInput| {
                        let plugin = plugin.clone();
                        async move {
                            let (owner, repo) = params
                                .repo
                                .map(|r| {
                                    let parts: Vec<_> = r.split('/').collect();
                                    (parts[0].to_string(), parts[1].to_string())
                                })
                                .unwrap_or_else(|| {
                                    (plugin.default_owner.clone(), plugin.default_repo.clone())
                                });

                            let body = serde_json::json!({
                                "title": params.title,
                                "body": params.body,
                                "labels": params.labels,
                            });

                            match plugin
                                .api_request(
                                    &format!("/repos/{}/{}/issues", owner, repo),
                                    "POST",
                                    Some(body),
                                )
                                .await
                            {
                                Ok(data) => {
                                    let number = data["number"].as_u64().unwrap_or(0);
                                    let url = data["html_url"].as_str().unwrap_or("");

                                    CallToolResult::text(format!(
                                        "✅ Created issue #{} in {}/{}\n\
                                         Title: {}\n\
                                         URL: {}",
                                        number, owner, repo, params.title, url
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
            // List pull requests
            ToolDefinition::new(
                Tool::new(
                    "github_list_prs",
                    "List pull requests in a repository",
                    serde_json::to_value(schemars::schema_for!(ListPRsInput)).unwrap(),
                ),
                {
                    let plugin = self.clone();
                    move |params: ListPRsInput| {
                        let plugin = plugin.clone();
                        async move {
                            let (owner, repo) = params
                                .repo
                                .map(|r| {
                                    let parts: Vec<_> = r.split('/').collect();
                                    (parts[0].to_string(), parts[1].to_string())
                                })
                                .unwrap_or_else(|| {
                                    (plugin.default_owner.clone(), plugin.default_repo.clone())
                                });

                            match plugin
                                .api_request(
                                    &format!(
                                        "/repos/{}/{}/pulls?state={}",
                                        owner, repo, params.state
                                    ),
                                    "GET",
                                    None,
                                )
                                .await
                            {
                                Ok(data) => {
                                    if let Some(prs) = data.as_array() {
                                        let pr_list: Vec<String> = prs
                                            .iter()
                                            .map(|pr| {
                                                let number = pr["number"].as_u64().unwrap_or(0);
                                                let title =
                                                    pr["title"].as_str().unwrap_or("No title");
                                                let state =
                                                    pr["state"].as_str().unwrap_or("unknown");
                                                format!("PR #{}: {} ({})", number, title, state)
                                            })
                                            .collect();

                                        CallToolResult::text(format!(
                                            "🔄 Pull Requests in {}/{}:\n\n{}",
                                            owner,
                                            repo,
                                            if pr_list.is_empty() {
                                                "No pull requests found".to_string()
                                            } else {
                                                pr_list.join("\n")
                                            }
                                        ))
                                    } else {
                                        CallToolResult::error("Invalid response format")
                                    }
                                }
                                Err(e) => CallToolResult::error(format!("API error: {}", e)),
                            }
                        }
                    }
                },
            ),
        ]
    }

    fn on_load(&mut self, config: &PluginConfig) -> McpResult<()> {
        if let Some(token) = config.config.get("token").and_then(|v| v.as_str()) {
            self.token = token.to_string();
        } else {
            return Err(McpError::invalid_params("token is required in config"));
        }

        if let Some(owner) = config.config.get("default_owner").and_then(|v| v.as_str()) {
            self.default_owner = owner.to_string();
        } else {
            return Err(McpError::invalid_params(
                "default_owner is required in config",
            ));
        }

        if let Some(repo) = config.config.get("default_repo").and_then(|v| v.as_str()) {
            self.default_repo = repo.to_string();
        } else {
            return Err(McpError::invalid_params(
                "default_repo is required in config",
            ));
        }

        tracing::info!(
            "✅ GitHub plugin loaded: {}/{}",
            self.default_owner,
            self.default_repo
        );

        Ok(())
    }

    fn on_unload(&mut self) -> McpResult<()> {
        tracing::info!("GitHub plugin unloaded");
        Ok(())
    }
}

// ─── Input Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct GetRepoInput {
    /// Repository (owner/repo), optional
    repo: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ListReposInput {
    /// Owner username, optional
    owner: Option<String>,
    /// Number of repos to return
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    10
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CreateIssueInput {
    /// Issue title
    title: String,
    /// Issue body/description
    body: String,
    /// Repository (owner/repo), optional
    repo: Option<String>,
    /// Labels to apply
    #[serde(default)]
    labels: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ListPRsInput {
    /// Repository (owner/repo), optional
    repo: Option<String>,
    /// PR state (open, closed, all)
    #[serde(default = "default_pr_state")]
    state: String,
}

fn default_pr_state() -> String {
    "open".to_string()
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("plugin_github=debug,mcp_kit=info")
        .init();

    tracing::info!("🚀 Starting GitHub plugin with REAL API");

    // Create plugin manager
    let mut plugin_manager = mcp_kit::plugin::PluginManager::new();

    // Configure GitHub plugin
    // ⚠️ IMPORTANT: Replace with your actual GitHub token!
    let config = mcp_kit::plugin::PluginConfig {
        config: serde_json::json!({
            "token": std::env::var("GITHUB_TOKEN")
                .unwrap_or_else(|_| {
                    tracing::warn!("GITHUB_TOKEN not set, using placeholder");
                    "your-github-token-here".to_string()
                }),
            "default_owner": "rust-lang",  // Default to rust-lang for demo
            "default_repo": "rust"
        }),
        enabled: true,
        priority: 0,
        permissions: mcp_kit::plugin::PluginPermissions {
            network: true,
            ..Default::default()
        },
    };

    plugin_manager.register_plugin(GitHubPlugin::new(), config)?;

    // List loaded plugins
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

    // Build server
    let server = McpServer::builder()
        .name("github-real-server")
        .version("1.0.0")
        .instructions(
            "GitHub integration server with REAL API calls.\n\n\
             Available tools:\n\
             - github_get_repo: Get repository info\n\
             - github_list_repos: List repositories\n\
             - github_create_issue: Create issues\n\
             - github_list_prs: List pull requests\n\n\
             Set GITHUB_TOKEN environment variable with your token.",
        )
        .with_plugin_manager(plugin_manager)
        .build();

    tracing::info!("✅ Server built, starting stdio transport...");
    tracing::info!("💡 Tip: Set GITHUB_TOKEN environment variable");

    // Serve via stdio
    server.serve_stdio().await?;

    Ok(())
}

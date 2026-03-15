# Plugin Examples

This directory contains real-world plugin examples demonstrating the mcp-kit plugin system.

## Available Plugins

### 1. Weather Plugin (`plugin_weather.rs`)

**Status:** ✅ Working  
**Complexity:** Simple  
**Features:**
- Get current weather for a city
- Get weather forecast
- Mock API implementation

**Usage:**
```bash
cargo run --example plugin_weather --features plugin,plugin-native
```

**Configuration:**
```json
{
  "api_key": "demo-api-key-12345"
}
```

**Tools:**
- `get_weather` - Get current weather
- `get_forecast` - Get multi-day forecast

---

### 2. Jira Plugin (`plugin_jira.rs`)

**Status:** 🚧 Demo/Template  
**Complexity:** Medium  
**Features:**
- Create and update Jira issues
- Search using JQL
- Manage sprints and transitions
- Add comments

**Configuration:**
```json
{
  "base_url": "https://your-domain.atlassian.net",
  "email": "your-email@example.com",
  "api_token": "your-api-token",
  "project_key": "PROJ"
}
```

**Tools:**
- `jira_create_issue` - Create new issues
- `jira_search` - Search with JQL
- `jira_get_issue` - Get issue details
- `jira_update_issue` - Update issues
- `jira_transition` - Change status
- `jira_add_comment` - Add comments
- `jira_list_sprints` - List sprints

**Production Implementation:**
To use with real Jira API, add these dependencies to your plugin:
```toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
base64 = "0.22"
```

Then implement actual API calls in the `api_request` method:
```rust
async fn api_request(&self, endpoint: &str, method: &str, body: Option<serde_json::Value>) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let url = format!("{}{}", self.base_url, endpoint);
    let auth = base64::encode(format!("{}:{}", self.email, self.api_token));
    
    let client = reqwest::Client::new();
    let mut request = match method {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        _ => return Err("Invalid method".into()),
    };
    
    request = request
        .header("Authorization", format!("Basic {}", auth))
        .header("Content-Type", "application/json");
    
    if let Some(body) = body {
        request = request.json(&body);
    }
    
    let response = request.send().await?;
    let json = response.json().await?;
    Ok(json)
}
```

---

### 3. Confluence Plugin (`plugin_confluence.rs`)

**Status:** 🚧 Demo/Template  
**Complexity:** Medium  
**Features:**
- Create and update pages
- Search content with CQL
- Manage spaces
- Add attachments
- View page hierarchy

**Configuration:**
```json
{
  "base_url": "https://your-domain.atlassian.net",
  "email": "your-email@example.com",
  "api_token": "your-api-token",
  "default_space": "TEAM"
}
```

**Tools:**
- `confluence_create_page` - Create pages
- `confluence_get_page` - Get page content
- `confluence_update_page` - Update pages
- `confluence_search` - Search with CQL
- `confluence_list_pages` - List pages in space
- `confluence_create_space` - Create spaces
- `confluence_add_attachment` - Add files
- `confluence_page_tree` - View hierarchy

---

### 4. GitHub Plugin (`plugin_github.rs`)

**Status:** 🚧 Demo/Template  
**Complexity:** Medium  
**Features:**
- Manage issues and pull requests
- List commits and branches
- Trigger GitHub Actions
- Search code

**Configuration:**
```json
{
  "token": "ghp_your_token_here",
  "default_owner": "your-username",
  "default_repo": "your-repo"
}
```

**Tools:**
- `github_create_issue` - Create issues
- `github_create_pr` - Create PRs
- `github_list_prs` - List pull requests
- `github_repo_info` - Get repo info
- `github_list_commits` - List commits
- `github_create_branch` - Create branches
- `github_trigger_workflow` - Run workflows
- `github_search_code` - Search code

**Production Implementation:**
For real GitHub API integration, use the `octocrab` crate:
```toml
[dependencies]
octocrab = "0.38"
```

```rust
use octocrab::Octocrab;

async fn create_issue(&self, title: &str, body: &str) -> Result<u64> {
    let octocrab = Octocrab::builder()
        .personal_token(self.token.clone())
        .build()?;
    
    let issue = octocrab
        .issues(&self.owner, &self.repo)
        .create(title)
        .body(body)
        .send()
        .await?;
    
    Ok(issue.number)
}
```

---

## Plugin Architecture

### Plugin Trait Implementation

All plugins implement the `McpPlugin` trait:

```rust
impl McpPlugin for MyPlugin {
    fn name(&self) -> &str { "my-plugin" }
    fn version(&self) -> &str { "1.0.0" }
    
    fn register_tools(&self) -> Vec<ToolDefinition> {
        // Return tools
    }
    
    fn register_resources(&self) -> Vec<ResourceDefinition> {
        // Return resources (optional)
    }
    
    fn on_load(&mut self, config: &PluginConfig) -> McpResult<()> {
        // Initialize from config
        Ok(())
    }
}
```

### Configuration Pattern

Extract config in `on_load`:

```rust
fn on_load(&mut self, config: &PluginConfig) -> McpResult<()> {
    self.api_key = config.config
        .get("api_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("api_key required"))?
        .to_string();
    
    Ok(())
}
```

### Tool Definition Pattern

Create tools with typed inputs:

```rust
ToolDefinition::new(
    Tool::new(
        "tool_name",
        "Tool description",
        serde_json::to_value(schemars::schema_for!(InputType)).unwrap(),
    ),
    {
        let plugin = self.clone(); // Clone for closure
        move |params: InputType| {
            let plugin = plugin.clone();
            async move {
                // Do work...
                CallToolResult::text("Result")
            }
        }
    },
)
```

### Error Handling

For production plugins, handle errors properly:

```rust
async move {
    match do_api_call().await {
        Ok(result) => CallToolResult::text(result),
        Err(e) => CallToolResult::error(format!("API error: {}", e))
    }
}
```

---

## Creating Your Own Plugin

### 1. Basic Structure

```rust
use mcp_kit::prelude::*;
use mcp_kit::plugin::{McpPlugin, PluginConfig, ToolDefinition};

struct MyPlugin {
    config_value: String,
}

impl MyPlugin {
    fn new() -> Self {
        Self {
            config_value: String::new(),
        }
    }
}

impl Clone for MyPlugin {
    fn clone(&self) -> Self {
        Self {
            config_value: self.config_value.clone(),
        }
    }
}

impl McpPlugin for MyPlugin {
    // Implement trait methods...
}
```

### 2. Register with Server

```rust
let mut plugin_manager = PluginManager::new();
plugin_manager.register_plugin(MyPlugin::new(), config)?;

let server = McpServer::builder()
    .with_plugin_manager(plugin_manager)
    .build();
```

### 3. Best Practices

- ✅ Validate configuration in `on_load`
- ✅ Use typed input structs with `JsonSchema`
- ✅ Provide clear error messages
- ✅ Include usage examples in tool descriptions
- ✅ Clone plugin for each handler closure
- ✅ Log important operations
- ✅ Clean up resources in `on_unload`

---

## Testing Plugins

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_plugin_load() {
        let mut plugin = MyPlugin::new();
        let config = PluginConfig {
            config: serde_json::json!({"key": "value"}),
            ..Default::default()
        };
        
        assert!(plugin.on_load(&config).is_ok());
    }
}
```

### Integration Tests

Test with a real MCP server:

```rust
#[tokio::test]
async fn test_plugin_integration() {
    let mut manager = PluginManager::new();
    manager.register_plugin(MyPlugin::new(), config).unwrap();
    
    let server = McpServer::builder()
        .with_plugin_manager(manager)
        .build();
    
    // Test tool calls...
}
```

---

## Contributing

To add a new plugin example:

1. Create `examples/plugin_your_service.rs`
2. Implement `McpPlugin` trait
3. Add configuration documentation
4. Update this README
5. Submit a pull request

---

## Resources

- [Plugin System Documentation](../docs/PLUGINS.md)
- [MCP Protocol Specification](https://modelcontextprotocol.io/)
- [mcp-kit API Documentation](https://docs.rs/mcp-kit)

---

## Notes

⚠️ **Security Warning:** The plugin examples are for demonstration purposes. For production use:
- Validate all inputs
- Use proper error handling
- Implement rate limiting
- Store credentials securely
- Follow API best practices
- Test thoroughly

💡 **Performance Tips:**
- Cache API responses when appropriate
- Use connection pooling
- Implement request batching
- Add timeout handling
- Monitor resource usage

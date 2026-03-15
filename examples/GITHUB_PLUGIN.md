# GitHub Plugin - Real API Example

This is a **PRODUCTION-READY** GitHub plugin with real API integration using `reqwest`.

## Features

✅ **Real GitHub REST API v3**
- Get repository information with live data
- List user/org repositories  
- Create issues
- List pull requests

✅ **Production Patterns**
- Proper error handling
- HTTP client with authentication
- Type-safe inputs with JsonSchema
- Environment variable configuration

## Setup

### 1. Get a GitHub Personal Access Token

1. Go to https://github.com/settings/tokens/new
2. Give it a name: "MCP Kit Plugin"
3. Select scopes:
   - `repo` (full control of private repositories)
   - `read:user` (read user profile data)
4. Generate token
5. Copy the token (starts with `ghp_`)

### 2. Set Environment Variable

```bash
export GITHUB_TOKEN=ghp_your_token_here
```

Or add to your `~/.bashrc` / `~/.zshrc`:
```bash
echo 'export GITHUB_TOKEN=ghp_your_token_here' >> ~/.zshrc
```

### 3. Run the Plugin

```bash
cargo run --example plugin_github_real --features plugin,plugin-native
```

## Usage Examples

### Get Repository Info

The plugin defaults to `rust-lang/rust` but you can query any repo:

```json
{
  "tool": "github_get_repo",
  "arguments": {
    "repo": "KSD-CO/mcp-kit"
  }
}
```

Response:
```
📦 Repository: KSD-CO/mcp-kit
Description: Ergonomic Rust library for building MCP servers
Language: Rust
Stars: ⭐ 42
Forks: 🍴 5
Open Issues: 🐛 3
URL: https://github.com/KSD-CO/mcp-kit
```

### List Repositories

```json
{
  "tool": "github_list_repos",
  "arguments": {
    "owner": "rust-lang",
    "limit": 5
  }
}
```

### Create an Issue

```json
{
  "tool": "github_create_issue",
  "arguments": {
    "repo": "your-username/your-repo",
    "title": "Feature request from MCP",
    "body": "This issue was created via the MCP GitHub plugin!",
    "labels": ["enhancement"]
  }
}
```

### List Pull Requests

```json
{
  "tool": "github_list_prs",
  "arguments": {
    "repo": "rust-lang/rust",
    "state": "open"
  }
}
```

## Code Structure

```rust
// Real API request implementation
async fn api_request(&self, endpoint: &str, method: &str, body: Option<Value>) 
    -> Result<Value> 
{
    let url = format!("https://api.github.com{}", endpoint);
    
    let mut request = match method {
        "GET" => self.client.get(&url),
        "POST" => self.client.post(&url),
        // ... other methods
    };
    
    // Add authentication
    request = request
        .header("Authorization", format!("Bearer {}", self.token))
        .header("User-Agent", "mcp-kit-github-plugin/1.0")
        .header("Accept", "application/vnd.github.v3+json");
    
    // Send and parse
    let response = request.send().await?;
    let json: Value = response.json().await?;
    Ok(json)
}
```

## API Rate Limits

GitHub API has rate limits:
- **Authenticated**: 5,000 requests per hour
- **Unauthenticated**: 60 requests per hour

Check your rate limit:
```bash
curl -H "Authorization: Bearer $GITHUB_TOKEN" \
  https://api.github.com/rate_limit
```

## Error Handling

The plugin handles common errors:

- **401 Unauthorized**: Invalid or expired token
- **404 Not Found**: Repository doesn't exist or you don't have access
- **403 Forbidden**: Rate limit exceeded or insufficient permissions
- **422 Unprocessable**: Invalid data (e.g., creating issue with bad format)

Example error response:
```
API error 404: {"message": "Not Found", "documentation_url": "..."}
```

## Extending the Plugin

To add more tools:

```rust
ToolDefinition::new(
    Tool::new(
        "github_get_commit",
        "Get commit details",
        schema,
    ),
    {
        let plugin = self.clone();
        move |params: GetCommitInput| {
            let plugin = plugin.clone();
            async move {
                match plugin.api_request(
                    &format!("/repos/{}/{}/commits/{}", owner, repo, sha),
                    "GET",
                    None
                ).await {
                    Ok(data) => {
                        // Parse and return
                        CallToolResult::text(format!("Commit: {}", data["sha"]))
                    }
                    Err(e) => CallToolResult::error(format!("Error: {}", e)),
                }
            }
        }
    },
)
```

## Dependencies

This example uses:
- `reqwest` - HTTP client for API calls
- `serde_json` - JSON parsing
- `tokio` - Async runtime

## Comparison: Template vs Real

| Feature | Template (`plugin_github.rs`) | Real (`plugin_github_real.rs`) |
|---------|-------------------------------|--------------------------------|
| API Calls | ❌ Mock | ✅ Real |
| Data | 🎭 Fake | ✅ Live |
| Dependencies | None | reqwest |
| Authentication | 🔒 Mock | ✅ Real tokens |
| Error Handling | Basic | ✅ Production |
| Tools | 8 | 4 (core ones) |
| Use Case | Learning | Production |

## Security Notes

⚠️ **Never commit your token to git!**

✅ Good practices:
- Use environment variables
- Use `.env` files (add to `.gitignore`)
- Rotate tokens regularly
- Use minimal scopes needed

❌ Bad practices:
- Hardcode tokens in code
- Commit tokens to repos
- Use admin tokens for read-only tasks

## Troubleshooting

**Token not working?**
```bash
# Test your token
curl -H "Authorization: Bearer $GITHUB_TOKEN" https://api.github.com/user
```

**Rate limit exceeded?**
```bash
# Check rate limit status
curl -H "Authorization: Bearer $GITHUB_TOKEN" https://api.github.com/rate_limit
```

**403 Forbidden on create issue?**
```bash
# Check if you have write access to the repo
curl -H "Authorization: Bearer $GITHUB_TOKEN" \
  https://api.github.com/repos/owner/repo
# Look for "permissions": {"push": true}
```

## Next Steps

1. Try the plugin with your own repos
2. Add more tools (commits, branches, releases)
3. Implement caching for better performance
4. Add webhook support
5. Create a full GitHub management suite

## References

- [GitHub REST API Documentation](https://docs.github.com/en/rest)
- [reqwest Documentation](https://docs.rs/reqwest)
- [mcp-kit Plugin System](../../docs/PLUGINS.md)

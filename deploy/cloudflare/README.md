# MCP Kit - Cloudflare Workers Example

A production-ready MCP server running on Cloudflare Workers, demonstrating the full capabilities of `mcp-kit` with a clean, modular architecture.

## 🚀 Features

- **Calculator Tools** - Basic arithmetic operations (add, subtract, multiply, divide, sqrt)
- **Text Processing Tools** - String manipulation (uppercase, lowercase, reverse, word_count, echo)
- **Static Resources** - Configuration, server info, and documentation
- **URI Templates** - Dynamic resources with parameter completion (user://{id}, doc://{id})
- **Prompts with Completion** - Code review, summarization, and translation prompts
- **Global Completion Handler** - Custom autocomplete for any input
- **Authentication** - API Key, Bearer Token, Basic Auth support

## 📁 Project Structure

```
src/
├── lib.rs              # Entry point & HTTP routing
├── server.rs           # MCP server builder & wrapper
├── completion.rs       # Global completion handler
├── instructions.md     # Server documentation (loaded at runtime)
│
├── auth/
│   ├── mod.rs          # Auth module, CompositeAuth, Identity
│   ├── apikey.rs       # API Key authentication (X-API-Key header)
│   ├── bearer.rs       # Bearer token authentication
│   └── basic.rs        # Basic authentication (username:password)
│
├── tools/
│   ├── mod.rs          # Tool module exports
│   ├── calculator.rs   # Mathematical operations
│   └── text.rs         # Text processing utilities
│
├── resources/
│   ├── mod.rs          # Resource module exports
│   ├── config.rs       # Static resources (config, server_info, readme)
│   └── templates.rs    # URI template resources (user://{id}, doc://{id})
│
└── prompts/
    └── mod.rs          # Prompts with argument completions
```

## 🛠️ Setup

### Prerequisites

- [Rust](https://rustup.rs/) (1.85+)
- [wrangler](https://developers.cloudflare.com/workers/wrangler/install-and-update/) CLI
- Cloudflare account

### Installation

```bash
# Install wrangler if you haven't
npm install -g wrangler

# Login to Cloudflare
wrangler login

# Navigate to the example
cd deploy/cloudflare
```

## 🏃 Running

### Local Development

```bash
# Start local dev server
wrangler dev

# Server will be available at http://localhost:8787
```

### Deploy to Cloudflare

```bash
# Deploy to production
wrangler deploy

# Your server will be available at https://mcp-kit-cloudflare.<your-subdomain>.workers.dev
```

## � Authentication

Authentication is **disabled by default**. To enable:

### 1. Enable in wrangler.toml

```toml
[vars]
AUTH_ENABLED = "true"
```

### 2. Set Production Secrets

```bash
# API Key authentication
wrangler secret put API_KEY

# Bearer token authentication  
wrangler secret put BEARER_TOKEN
```

### 3. Supported Methods

| Method | Header/Format | Example |
|--------|---------------|---------|
| **API Key** | `X-API-Key: <key>` | `X-API-Key: my-secret-key` |
| **Bearer** | `Authorization: Bearer <token>` | `Authorization: Bearer my-token` |
| **Basic** | `Authorization: Basic <base64>` | `Authorization: Basic YWRtaW46cGFzcw==` |

### Demo Credentials (Development Only)

When secrets are not set, these demo credentials work:

**API Keys:**
- `demo-key` (role: demo)
- `admin-key` (role: admin, user)

**Bearer Tokens:**
- `demo-token` (role: demo)
- `secret-token` (role: admin)

**Basic Auth:**
- `demo:demo123` (role: user)
- `admin:admin123` (role: admin)

### Testing Authentication

```bash
# With API Key
curl -X POST https://your-worker.workers.dev/mcp \
  -H "X-API-Key: demo-key" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}'

# With Bearer Token
curl -X POST https://your-worker.workers.dev/mcp \
  -H "Authorization: Bearer demo-token" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}'

# With Basic Auth
curl -X POST https://your-worker.workers.dev/mcp \
  -H "Authorization: Basic ZGVtbzpkZW1vMTIz" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}'
```

## �🔌 Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/mcp` | POST | JSON-RPC endpoint for MCP requests |
| `/mcp` | GET | Server-Sent Events (SSE) for streaming |
| `/health` | GET | Health check endpoint |

## 📖 Usage Examples

### Using with Claude Desktop

Add to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "cloudflare-mcp": {
      "url": "https://your-worker.workers.dev/mcp"
    }
  }
}
```

### Testing with curl

```bash
# Health check
curl https://your-worker.workers.dev/health

# Initialize
curl -X POST https://your-worker.workers.dev/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2024-11-05",
      "capabilities": {},
      "clientInfo": { "name": "test", "version": "1.0" }
    }
  }'

# List tools
curl -X POST https://your-worker.workers.dev/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc": "2.0", "id": 2, "method": "tools/list"}'

# Call a tool
curl -X POST https://your-worker.workers.dev/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 3,
    "method": "tools/call",
    "params": {
      "name": "add",
      "arguments": { "a": 5, "b": 3 }
    }
  }'
```

## 🧰 Available Tools

### Calculator Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `add` | Add two numbers | `a`, `b` |
| `subtract` | Subtract b from a | `a`, `b` |
| `multiply` | Multiply two numbers | `a`, `b` |
| `divide` | Divide a by b | `a`, `b` |
| `sqrt` | Square root | `number` |

### Text Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `uppercase` | Convert to uppercase | `text` |
| `lowercase` | Convert to lowercase | `text` |
| `reverse` | Reverse a string | `text` |
| `word_count` | Count words | `text` |
| `echo` | Echo back input | `message` |

## 📚 Available Resources

### Static Resources

| URI | Description |
|-----|-------------|
| `config://app` | Application configuration |
| `config://server` | Server information |
| `docs://readme` | Server documentation |

### URI Templates

| Template | Description |
|----------|-------------|
| `user://{id}` | User profile by ID |
| `doc://{id}` | Document by ID |

## 💬 Available Prompts

| Prompt | Description | Arguments |
|--------|-------------|-----------|
| `code_review` | Review code for issues | `code`, `language` |
| `summarize` | Summarize text | `text`, `style` |
| `translate` | Translate text | `text`, `target_language` |

## 🔧 Customization

### Adding a New Tool

1. Create a new file in `src/tools/` or add to existing file
2. Export from `src/tools/mod.rs`
3. Register in `src/server.rs` using `.tool()`

```rust
// src/tools/my_tool.rs
use mcp_kit::types::Tool;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct MyInput {
    pub value: String,
}

pub fn my_tool() -> Tool {
    Tool::new("my_tool", "Description", Tool::schema::<MyInput>())
}

pub async fn my_handler(input: MyInput) -> mcp_kit::types::CallToolResult {
    mcp_kit::types::CallToolResult::text(format!("Result: {}", input.value))
}
```

### Adding a New Resource

1. Create handler in `src/resources/`
2. Register in `src/server.rs` using `.resource()` or `.resource_template()`

### Adding a New Prompt

1. Add to `src/prompts/mod.rs`
2. Register in `src/server.rs` using `.prompt()` or `.prompt_with_completion()`

## 📝 Configuration

Edit `wrangler.toml` to customize:

```toml
name = "mcp-kit-cloudflare"
main = "src/lib.rs"
compatibility_date = "2024-01-01"

[build]
command = "cargo install -q worker-build && worker-build --release"
```

## 🔒 Security Considerations

For production deployments, consider:

1. **Enable Authentication** - Set `AUTH_ENABLED=true` and configure secrets
2. **Use Strong Secrets** - Generate random API keys and tokens
3. **Rate Limiting** - Use Cloudflare's rate limiting features
4. **Input Validation** - Validate all inputs thoroughly
5. **CORS** - Configure appropriate CORS headers for your domain
6. **Audit Logging** - Log authentication events for security monitoring

## 📄 License

MIT License - See [LICENSE](../../LICENSE) for details.

## 🔗 Related

- [mcp-kit](https://github.com/KSD-CO/mcp-kit) - Main library
- [MCP Specification](https://modelcontextprotocol.io/) - Protocol documentation
- [Cloudflare Workers](https://workers.cloudflare.com/) - Runtime platform
- [Architecture Guide](docs/architecture.md) - Detailed architecture for MCP Gateway

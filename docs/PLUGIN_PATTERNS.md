# Plugin Development Guide

## Why ToolDefinition::new?

You might notice that plugin examples use `ToolDefinition::new()` instead of the `#[tool]` macro. This is intentional and provides several benefits:

### The `#[tool]` Macro (For Regular Tools)

```rust
#[tool(description = "Add numbers")]
async fn add(a: f64, b: f64) -> String {
    format!("{}", a + b)
}

// Generates: add_tool_def() function
```

**Limitations for Plugins:**
- ❌ Cannot access plugin state (`self`)
- ❌ Cannot use plugin configuration
- ❌ Static function only
- ❌ No dynamic behavior

### ToolDefinition::new (For Plugins)

```rust
fn register_tools(&self) -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            Tool::new("add", "Add numbers", schema),
            {
                let plugin = self.clone();  // ✅ Capture plugin state
                move |params: AddInput| {
                    let plugin = plugin.clone();
                    async move {
                        // ✅ Access plugin config
                        let precision = plugin.precision;
                        // ✅ Call plugin methods  
                        plugin.format_result(params.a + params.b)
                    }
                }
            },
        ),
    ]
}
```

**Benefits:**
- ✅ Access plugin state and configuration
- ✅ Call plugin methods
- ✅ Share HTTP clients, connections
- ✅ Dynamic behavior based on config
- ✅ Full control over closure

---

## Pattern Explanation

### The Clone Pattern

```rust
{
    let plugin = self.clone();  // Clone 1: Capture for move closure
    move |params: InputType| {
        let plugin = plugin.clone();  // Clone 2: For async move
        async move {
            // Use plugin here
            plugin.method()
        }
    }
}
```

**Why two clones?**
1. First clone: Move into closure
2. Second clone: Move into async block

This is standard Rust async closure pattern.

### Type Safety

```rust
ToolDefinition::new(
    Tool::new("name", "desc", schema),  // Tool metadata
    |params: InputType| async move {    // Type-safe input
        CallToolResult::text("result")  // Type-safe output
    },
)
```

**Benefits:**
- Compile-time type checking
- IDE autocomplete
- Refactoring safety
- Clear API contracts

---

## Comparison: Macro vs Manual

### Using `#[tool]` Macro (Top Level)

**Pros:**
- ✅ Less boilerplate
- ✅ Cleaner syntax
- ✅ Auto schema generation

**Cons:**
- ❌ No plugin state access
- ❌ No configuration
- ❌ Static only

**Use for:** Simple standalone tools

### Using `ToolDefinition::new` (In Plugins)

**Pros:**
- ✅ Full plugin state access
- ✅ Configuration support
- ✅ HTTP clients, DB connections
- ✅ Dynamic behavior

**Cons:**
- ⚠️ More verbose (but still clean)

**Use for:** Plugin tools that need state

---

## Best Practices

### 1. Clone Your Plugin

Always implement `Clone` for your plugin:

```rust
impl Clone for MyPlugin {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            client: reqwest::Client::new(),  // New client per clone
        }
    }
}
```

### 2. Handle Errors Properly

```rust
async move {
    match plugin.api_call().await {
        Ok(data) => CallToolResult::text(data),
        Err(e) => CallToolResult::error(format!("Error: {}", e)),
    }
}
```

### 3. Use Type-Safe Inputs

```rust
#[derive(Deserialize, JsonSchema)]
struct MyInput {
    /// Field description for docs
    field: String,
    /// Optional field with default
    #[serde(default)]
    optional: Option<String>,
}
```

### 4. Keep Handlers Small

```rust
// ✅ Good: Delegate to plugin methods
|params: Input| async move {
    plugin.handle_request(params).await
}

// ❌ Bad: Complex logic in closure
|params: Input| async move {
    // 50 lines of complex logic...
}
```

---

## Advanced Patterns

### Shared HTTP Client

```rust
struct MyPlugin {
    client: reqwest::Client,  // Reused across requests
}

impl Clone for MyPlugin {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),  // Cheap Arc clone
        }
    }
}
```

### Configuration-Driven Behavior

```rust
fn register_tools(&self) -> Vec<ToolDefinition> {
    let mut tools = vec![/* base tools */];
    
    // Add optional tools based on config
    if self.config.enable_advanced {
        tools.push(/* advanced tool */);
    }
    
    tools
}
```

### Error Context

```rust
match plugin.api_call().await {
    Ok(data) => CallToolResult::text(format!("Success: {}", data)),
    Err(e) => CallToolResult::error(format!(
        "Failed to call {}: {}",
        plugin.service_name,
        e
    )),
}
```

---

## Why This Pattern Works

1. **Flexibility** - Full control over tool behavior
2. **Type Safety** - Compile-time guarantees
3. **State Management** - Easy plugin state access
4. **Testability** - Can unit test plugin methods
5. **Performance** - Shared resources (HTTP clients, etc.)

---

## Future Improvements

We're exploring:
- Procedural macro `#[plugin]` that generates McpPlugin impl
- Derive macro for simpler plugin definition
- Builder pattern helpers

But current pattern is already clean and works well! 

---

See working examples:
- `examples/plugin_github.rs` - Real GitHub API
- `examples/plugin_clickhouse.rs` - Real database
- All examples use this pattern consistently

# Plugin System

The mcp-kit plugin system allows you to dynamically load and manage tools, resources, and prompts from external libraries.

## Features

- 🔌 **Dynamic Loading** - Load plugins from shared libraries (.so, .dylib, .dll) or WASM modules
- 🔄 **Hot Reload** - Automatically reload plugins when files change (development mode)
- 🎯 **Type Safety** - Strongly typed plugin API with compile-time checks
- 🛡️ **Sandboxing** - WASM plugins run in a secure sandbox
- 📦 **Plugin Registry** - Discover and install community plugins
- ⚙️ **Configuration** - Pass configuration to plugins at load time
- 🔐 **Permissions** - Control plugin access to filesystem, network, etc.

## Quick Start

### Creating a Plugin

```rust
use mcp_kit::prelude::*;
use mcp_kit::plugin::{McpPlugin, PluginConfig, ToolDefinition};

struct MyPlugin;

impl McpPlugin for MyPlugin {
    fn name(&self) -> &str {
        "my-plugin"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> Option<&str> {
        Some("My awesome plugin")
    }

    fn register_tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition::new(
                Tool::new(
                    "hello",
                    "Say hello",
                    serde_json::json!({"type": "object"}),
                ),
                |_params: serde_json::Value| async move {
                    CallToolResult::text("Hello from plugin!")
                },
            ),
        ]
    }

    fn on_load(&mut self, config: &PluginConfig) -> McpResult<()> {
        tracing::info!("Plugin loaded with config: {:?}", config);
        Ok(())
    }
}
```

### Loading Plugins

```rust
use mcp_kit::prelude::*;
use mcp_kit::plugin::PluginManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut plugin_manager = PluginManager::new();
    
    // Load a plugin from a shared library
    plugin_manager.load_from_path("./plugins/my_plugin.so")?;
    
    // Or register an in-process plugin
    plugin_manager.register_plugin(MyPlugin, PluginConfig::default())?;
    
    // Build server with plugins
    let server = McpServer::builder()
        .name("my-server")
        .version("1.0.0")
        .with_plugin_manager(plugin_manager)
        .build();
    
    server.serve_stdio().await?;
    Ok(())
}
```

### Using the Builder API

```rust
// Load plugins directly in the builder
let server = McpServer::builder()
    .name("my-server")
    .load_plugin("./plugins/weather.so")?
    .load_plugin("./plugins/database.so")?
    .build();
```

## Plugin Configuration

Pass configuration to plugins at load time:

```rust
use mcp_kit::plugin::PluginConfig;

let config = PluginConfig {
    config: serde_json::json!({
        "api_key": "secret-key-123",
        "endpoint": "https://api.example.com"
    }),
    enabled: true,
    priority: 10,  // Higher priority plugins load first
    permissions: PluginPermissions {
        network: true,
        filesystem: false,
        env: false,
        process: false,
        ..Default::default()
    },
};

plugin_manager.load_from_path_with_config("./plugins/api.so", config)?;
```

## Plugin Lifecycle

Plugins have lifecycle hooks you can implement:

```rust
impl McpPlugin for MyPlugin {
    fn on_load(&mut self, config: &PluginConfig) -> McpResult<()> {
        // Called when plugin is loaded
        // Initialize resources, validate config, etc.
        Ok(())
    }

    fn on_unload(&mut self) -> McpResult<()> {
        // Called when plugin is unloaded
        // Cleanup resources, close connections, etc.
        Ok(())
    }

    fn can_unload(&self) -> bool {
        // Return false to prevent unloading
        // (e.g., if there are active connections)
        true
    }
}
```

## Native Plugins (Shared Libraries)

To create a plugin as a shared library, export a constructor function:

```rust
// lib.rs in your plugin crate

use mcp_kit::plugin::McpPlugin;

struct MyPlugin;

// Implement McpPlugin...

#[no_mangle]
pub extern "C" fn _mcp_plugin_create() -> *mut dyn McpPlugin {
    Box::into_raw(Box::new(MyPlugin))
}
```

Build as a dynamic library:

```toml
# Cargo.toml

[lib]
crate-type = ["cdylib"]

[dependencies]
mcp-kit = { version = "0.1", features = ["plugin"] }
```

```bash
cargo build --release
# Produces target/release/libmy_plugin.so (or .dylib on macOS, .dll on Windows)
```

## WASM Plugins (Coming Soon)

WASM plugins will provide secure sandboxing:

```rust
let wasm_bytes = std::fs::read("./plugins/my_plugin.wasm")?;
plugin_manager.load_wasm(&wasm_bytes)?;
```

## Hot Reload (Development)

Enable automatic reloading when plugin files change:

```rust
plugin_manager.enable_hot_reload()?;
```

## Plugin Registry (Coming Soon)

Install plugins from a registry:

```rust
use mcp_kit::plugin::registry::PluginRegistry;

let registry = PluginRegistry::default_registry();

// Search for plugins
let results = registry.search("weather").await?;

// Install a plugin
let path = registry.install("mcp-plugins/weather", Some("^1.0")).await?;
plugin_manager.load_from_path(&path)?;
```

## Plugin Management

```rust
// List all loaded plugins
let plugins = plugin_manager.list_plugins();
for plugin in plugins {
    println!("{} v{}: {}", plugin.name, plugin.version, plugin.description);
    println!("  Tools: {}", plugin.tool_count);
    println!("  Resources: {}", plugin.resource_count);
}

// Get metadata for a specific plugin
if let Some(meta) = plugin_manager.get_metadata("weather") {
    println!("Weather plugin: {:?}", meta);
}

// Unload a plugin
plugin_manager.unload("weather")?;
```

## Examples

See [`examples/plugin_weather.rs`](../examples/plugin_weather.rs) for a complete working example.

Run it with:

```bash
cargo run --example plugin_weather --features plugin,plugin-native
```

## Feature Flags

Enable plugin features in your `Cargo.toml`:

```toml
[dependencies]
mcp-kit = { version = "0.1", features = ["plugin", "plugin-native"] }
```

Available plugin features:
- `plugin` - Core plugin system (required)
- `plugin-native` - Native shared library loading
- `plugin-wasm` - WASM plugin support
- `plugin-hot-reload` - Hot reload during development

## Best Practices

1. **Version Compatibility** - Specify `min_mcp_version()` in your plugin
2. **Error Handling** - Return proper errors from plugin methods
3. **Configuration Validation** - Validate config in `on_load()`
4. **Resource Cleanup** - Clean up in `on_unload()`
5. **Permission Minimization** - Request only needed permissions
6. **Testing** - Write unit tests for your plugin
7. **Documentation** - Document your plugin's tools and config options

## Security Considerations

- Native plugins have full system access (use with trusted code only)
- WASM plugins run in a sandbox with limited capabilities
- Use the permissions system to restrict plugin access
- Validate all plugin inputs and outputs
- Keep plugins updated to patch security issues

## Troubleshoads

**Plugin fails to load:**
- Check that the plugin exports `_mcp_plugin_create` function
- Verify the plugin is built with compatible mcp-kit version
- Check file permissions and path

**Hot reload not working:**
- Enable the `plugin-hot-reload` feature
- Check that the file watcher has permission to watch the directory
- Verify the plugin can be safely unloaded (`can_unload() == true`)

**Plugin tools not appearing:**
- Verify `register_tools()` returns the tools
- Check plugin is enabled in config
- Look for errors in plugin `on_load()` method

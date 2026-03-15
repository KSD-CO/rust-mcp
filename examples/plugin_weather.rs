//! Example: Weather plugin demonstrating the plugin system
//!
//! This example shows how to:
//! 1. Create a plugin that provides tools
//! 2. Load the plugin into an MCP server
//! 3. Use the plugin's tools
//!
//! Run with: cargo run --example plugin_weather --features plugin,plugin-native

use mcp_kit::plugin::{McpPlugin, PluginConfig, ToolDefinition};
use mcp_kit::prelude::*;
use serde::Deserialize;

// ─── Weather Plugin ──────────────────────────────────────────────────────────

/// A simple weather plugin that provides weather-related tools
struct WeatherPlugin {
    api_key: Option<String>,
}

impl WeatherPlugin {
    pub fn new() -> Self {
        Self { api_key: None }
    }
}

impl McpPlugin for WeatherPlugin {
    fn name(&self) -> &str {
        "weather"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> Option<&str> {
        Some("Provides weather information for cities")
    }

    fn author(&self) -> Option<&str> {
        Some("MCP Kit Team")
    }

    fn register_tools(&self) -> Vec<ToolDefinition> {
        vec![
            // Get current weather
            ToolDefinition::new(
                Tool::new(
                    "get_weather",
                    "Get current weather for a city",
                    serde_json::to_value(schemars::schema_for!(GetWeatherInput)).unwrap(),
                ),
                |params: GetWeatherInput| async move {
                    // Simulate API call
                    let weather = format!(
                        "Weather in {}: {} and {}°C",
                        params.city,
                        if params.city.to_lowercase().contains("london") {
                            "Cloudy"
                        } else {
                            "Sunny"
                        },
                        if params.city.to_lowercase().contains("london") {
                            15
                        } else {
                            25
                        }
                    );

                    CallToolResult::text(weather)
                },
            ),
            // Get forecast
            ToolDefinition::new(
                Tool::new(
                    "get_forecast",
                    "Get weather forecast for a city",
                    serde_json::to_value(schemars::schema_for!(GetForecastInput)).unwrap(),
                ),
                |params: GetForecastInput| async move {
                    let forecast = format!(
                        "Forecast for {} ({} days): Mostly {} with temperatures ranging 15-25°C",
                        params.city,
                        params.days,
                        if params.city.to_lowercase().contains("london") {
                            "cloudy"
                        } else {
                            "sunny"
                        }
                    );

                    CallToolResult::text(forecast)
                },
            ),
        ]
    }

    fn on_load(&mut self, config: &PluginConfig) -> McpResult<()> {
        // Extract API key from config
        if let Some(key) = config.config.get("api_key").and_then(|v| v.as_str()) {
            self.api_key = Some(key.to_string());
            tracing::info!("Weather plugin loaded with API key");
        } else {
            tracing::warn!("Weather plugin loaded without API key (using mock data)");
        }

        Ok(())
    }

    fn on_unload(&mut self) -> McpResult<()> {
        tracing::info!("Weather plugin unloaded");
        Ok(())
    }
}

// ─── Input Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct GetWeatherInput {
    /// City name
    city: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GetForecastInput {
    /// City name
    city: String,
    /// Number of days to forecast
    #[serde(default = "default_days")]
    days: u32,
}

fn default_days() -> u32 {
    3
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("plugin_weather=debug,mcp_kit=info")
        .init();

    tracing::info!("Starting weather plugin example");

    // Create plugin manager and register weather plugin
    let mut plugin_manager = mcp_kit::plugin::PluginManager::new();

    // Register plugin with custom config
    let config = mcp_kit::plugin::PluginConfig {
        config: serde_json::json!({
            "api_key": "demo-api-key-12345"
        }),
        enabled: true,
        priority: 0,
        ..Default::default()
    };

    plugin_manager.register_plugin(WeatherPlugin::new(), config)?;

    // List loaded plugins
    let plugins = plugin_manager.list_plugins();
    tracing::info!("Loaded plugins: {:?}", plugins);

    // Build server with plugin manager
    let server = McpServer::builder()
        .name("weather-server")
        .version("1.0.0")
        .instructions("Weather information server powered by plugins")
        .with_plugin_manager(plugin_manager)
        .build();

    tracing::info!("Server built, starting stdio transport...");

    // Serve via stdio
    server.serve_stdio().await?;

    Ok(())
}

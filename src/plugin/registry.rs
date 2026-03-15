//! Plugin registry for discovering and downloading plugins

use crate::error::{McpError, McpResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Plugin registry client
#[allow(dead_code)]
pub struct PluginRegistry {
    registry_url: String,
    cache_dir: PathBuf,
}

impl PluginRegistry {
    /// Create a new registry client
    pub fn new(registry_url: String) -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("mcp-kit")
            .join("plugins");

        Self {
            registry_url,
            cache_dir,
        }
    }

    /// Default registry (https://mcp-plugins.io or similar)
    pub fn default_registry() -> Self {
        Self::new("https://mcp-plugins.io".to_string())
    }

    /// Search for plugins
    pub async fn search(&self, query: &str) -> McpResult<Vec<PluginInfo>> {
        // TODO: Implement registry API client
        let _ = query;
        Err(McpError::internal(
            "Registry search not yet implemented".to_string(),
        ))
    }

    /// Download and install a plugin
    pub async fn install(&self, name: &str, version: Option<&str>) -> McpResult<PathBuf> {
        // TODO: Implement plugin download and caching
        let _ = (name, version);
        Err(McpError::internal(
            "Plugin installation not yet implemented".to_string(),
        ))
    }

    /// Get plugin info from registry
    pub async fn info(&self, name: &str) -> McpResult<PluginInfo> {
        // TODO: Implement plugin info fetching
        let _ = name;
        Err(McpError::internal(
            "Plugin info fetching not yet implemented".to_string(),
        ))
    }
}

/// Plugin information from registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub repository: Option<String>,
    pub license: Option<String>,
}

// Helper to get cache directory
fn dirs_cache_dir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
    }

    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Caches"))
    }

    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

mod dirs {
    use super::*;
    pub fn cache_dir() -> Option<PathBuf> {
        dirs_cache_dir()
    }
}

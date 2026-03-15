//! Native dynamic library plugin loading (.so, .dylib, .dll)

use crate::error::{McpError, McpResult};
use crate::plugin::McpPlugin;
use std::ffi::OsStr;

/// Function signature for plugin constructor
///
/// Every plugin dynamic library must export a function with this signature:
/// ```rust,ignore
/// #[no_mangle]
/// pub extern "C" fn _mcp_plugin_create() -> *mut dyn McpPlugin {
///     Box::into_raw(Box::new(MyPlugin::new()))
/// }
/// ```
#[allow(improper_ctypes_definitions)]
pub type PluginCreate = unsafe extern "C" fn() -> *mut dyn McpPlugin;

/// Load a plugin from a dynamic library file
pub fn load_plugin<P: AsRef<OsStr>>(path: P) -> McpResult<Box<dyn McpPlugin>> {
    unsafe {
        let lib = libloading::Library::new(path.as_ref())
            .map_err(|e| McpError::internal(format!("Failed to load library: {}", e)))?;

        let constructor: libloading::Symbol<PluginCreate> =
            lib.get(b"_mcp_plugin_create\0").map_err(|e| {
                McpError::internal(format!(
                    "Plugin does not export _mcp_plugin_create function: {}",
                    e
                ))
            })?;

        let plugin_ptr = constructor();

        if plugin_ptr.is_null() {
            return Err(McpError::internal(
                "Plugin constructor returned null".to_string(),
            ));
        }

        // Take ownership of the plugin
        let plugin = Box::from_raw(plugin_ptr);

        // Forget the library to keep it loaded
        std::mem::forget(lib);

        Ok(plugin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_nonexistent_plugin() {
        let result = load_plugin("/nonexistent/plugin.so");
        assert!(result.is_err());
    }
}

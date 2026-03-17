//! Example: WASM plugin demonstrating WebAssembly plugin support
//!
//! This example shows how to:
//! 1. Create WASM modules with different parameter types
//! 2. Load WASM plugins into an MCP server
//! 3. Execute WASM functions with type safety
//! 4. Handle string parameters with memory operations
//!
//! Run with: cargo run --example wasm_plugin --features plugin,plugin-wasm

use mcp_kit::prelude::*;
use std::fs;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("wasm_plugin=info,mcp_kit=info")
        .init();

    println!("🦀 MCP Kit - WASM Plugin Example");
    println!("==================================\n");

    // Create WASM modules demonstrating different capabilities
    create_wasm_modules().await?;

    // Create server builder
    let builder = McpServer::builder()
        .name("wasm-plugin-demo")
        .version("1.0.0")
        .instructions("Demonstration of WASM plugin capabilities with multiple parameter types");

    // Load WASM plugins and add their tools
    let builder = load_wasm_plugins(builder).await?;

    // Build and start server
    let server = builder.build();

    println!("🚀 Starting server with WASM plugins...");
    println!("💡 Try these commands:");
    println!("   - Call add tool: {{\"method\": \"tools/call\", \"params\": {{\"name\": \"add\", \"arguments\": {{\"param0\": 15, \"param1\": 27}}}}}}");
    println!("   - Call multiply tool: {{\"method\": \"tools/call\", \"params\": {{\"name\": \"multiply\", \"arguments\": {{\"param0\": 3.5, \"param1\": 2.0}}}}}}");
    println!("   - Call calculate tool: {{\"method\": \"tools/call\", \"params\": {{\"name\": \"calculate\", \"arguments\": {{\"param0\": 5, \"param1\": 2.5, \"param2\": 1.5}}}}}}");
    println!("   - Call strlen tool: {{\"method\": \"tools/call\", \"params\": {{\"name\": \"strlen\", \"arguments\": {{\"param0\": \"hello world\"}}}}}}");
    println!();

    // Start server
    server.serve_stdio().await?;

    Ok(())
}

async fn create_wasm_modules() -> anyhow::Result<()> {
    println!("📦 Creating WASM modules...");

    // Ensure examples directory exists
    fs::create_dir_all("examples/wasm")?;

    // 1. Integer arithmetic (i32 + i32 -> i32)
    let add_wat = r#"
(module
  (func (export "add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add))
"#;

    // 2. Float arithmetic (f32 * f32 -> f32)
    let multiply_wat = r#"
(module
  (func (export "multiply") (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.mul))
"#;

    // 3. Mixed types (i32 * f32 * f64 -> f64)
    let calculate_wat = r#"
(module
  (func (export "calculate") (param i32 f32 f64) (result f64)
    local.get 0     ;; i32 count
    f64.convert_i32_s  ;; convert to f64
    local.get 1     ;; f32 rate  
    f64.promote_f32 ;; convert to f64
    f64.mul         ;; count * rate
    local.get 2     ;; f64 precision
    f64.mul))       ;; (count * rate) * precision
"#;

    // 4. String length function (uses memory)
    let strlen_wat = r#"
(module
  (memory (export "memory") 1)
  (func (export "strlen") (param i32) (result i32)
    (local i32)  ;; counter
    local.get 0  ;; get string pointer
    i32.const 0
    local.set 1  ;; counter = 0
    
    loop
      local.get 0
      i32.load8_u  ;; load byte at pointer
      i32.eqz      ;; check if null terminator
      if
        local.get 1
        return     ;; return counter
      end
      local.get 0  
      i32.const 1
      i32.add      ;; pointer++
      local.set 0
      local.get 1
      i32.const 1  
      i32.add      ;; counter++
      local.set 1
      br 0         ;; continue loop
    end
    local.get 1    ;; return counter
  ))
"#;

    // Compile WAT to WASM bytes
    let add_wasm = wat::parse_str(add_wat)?;
    let multiply_wasm = wat::parse_str(multiply_wat)?;
    let calculate_wasm = wat::parse_str(calculate_wat)?;
    let strlen_wasm = wat::parse_str(strlen_wat)?;

    // Save WASM files
    fs::write("examples/wasm/add.wasm", add_wasm)?;
    fs::write("examples/wasm/multiply.wasm", multiply_wasm)?;
    fs::write("examples/wasm/calculate.wasm", calculate_wasm)?;
    fs::write("examples/wasm/strlen.wasm", strlen_wasm)?;

    println!("✅ Created 4 WASM modules:");
    println!("   - add.wasm: i32 addition");
    println!("   - multiply.wasm: f32 multiplication");
    println!("   - calculate.wasm: mixed types calculation");
    println!("   - strlen.wasm: string length with memory");
    println!();

    Ok(())
}

async fn load_wasm_plugins(
    builder: mcp_kit::server::McpServerBuilder,
) -> anyhow::Result<mcp_kit::server::McpServerBuilder> {
    use mcp_kit::plugin::wasm::load_plugin;

    println!("🔧 Loading WASM plugins...");

    let mut builder = builder;

    // Load each WASM module as a plugin
    let wasm_files = [
        ("examples/wasm/add.wasm", "Integer addition (i32 + i32)"),
        (
            "examples/wasm/multiply.wasm",
            "Float multiplication (f32 * f32)",
        ),
        (
            "examples/wasm/calculate.wasm",
            "Mixed types calculation (i32 * f32 * f64)",
        ),
        (
            "examples/wasm/strlen.wasm",
            "String length with memory operations",
        ),
    ];

    for (file_path, description) in wasm_files {
        match fs::read(file_path) {
            Ok(wasm_bytes) => {
                match load_plugin(&wasm_bytes) {
                    Ok(plugin) => {
                        let tools = plugin.register_tools();
                        println!(
                            "✅ Loaded {}: {} tool(s) - {}",
                            file_path,
                            tools.len(),
                            description
                        );

                        // Add all tools from this plugin to the builder
                        for tool_def in tools {
                            let handler = tool_def.handler;
                            builder = builder.tool(tool_def.tool, move |req| {
                                let h = handler.clone();
                                async move { h(req).await }
                            });
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to load {}: {}", file_path, e);
                    }
                }
            }
            Err(e) => {
                println!("❌ Failed to read {}: {}", file_path, e);
            }
        }
    }

    println!();
    Ok(builder)
}

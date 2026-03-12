extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    parse_macro_input, punctuated::Punctuated, spanned::Spanned, Attribute, FnArg, ItemFn,
    Lit, Meta, MetaNameValue, Pat, PatType, Token, Type,
};

// ─── #[tool] ─────────────────────────────────────────────────────────────────

/// Marks an async function as an MCP tool and generates a companion
/// `{fn_name}_tool_def()` function that returns a `mcp::ToolDef`.
///
/// # Attributes
/// - `description = "..."` — human-readable description (required)
/// - `name = "..."` — tool name (defaults to the function name)
///
/// # Example
/// ```rust
/// use mcp::prelude::*;
///
/// /// Add two numbers together.
/// #[tool(description = "Add two numbers")]
/// async fn add(a: f64, b: f64) -> String {
///     format!("{}", a + b)
/// }
///
/// // Register with the builder:
/// McpServer::builder().tool_def(add_tool_def()).build();
/// ```
///
/// Each function parameter must implement `serde::Deserialize` and will be
/// extracted from the tool call's `arguments` JSON object by field name.
#[proc_macro_attribute]
pub fn tool(args: TokenStream, input: TokenStream) -> TokenStream {
    let attr_args = parse_macro_input!(args with Punctuated::<Meta, Token![,]>::parse_terminated);
    let func = parse_macro_input!(input as ItemFn);

    match tool_impl(attr_args, func) {
        Ok(ts) => ts.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

fn tool_impl(
    attr_args: Punctuated<Meta, Token![,]>,
    func: ItemFn,
) -> syn::Result<TokenStream2> {
    // ── Parse attribute options ───────────────────────────────────────────────
    let mut description: Option<String> = None;
    let mut tool_name: Option<String> = None;

    for meta in &attr_args {
        match meta {
            Meta::NameValue(MetaNameValue { path, value, .. }) => {
                let key = path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                if let syn::Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) = value {
                    match key.as_str() {
                        "description" => description = Some(s.value()),
                        "name" => tool_name = Some(s.value()),
                        other => {
                            return Err(syn::Error::new(path.span(), format!("Unknown attribute: {other}")));
                        }
                    }
                }
            }
            other => {
                return Err(syn::Error::new(other.span(), "Expected key = \"value\" pairs"));
            }
        }
    }

    // Fall back to doc comment for description
    if description.is_none() {
        description = extract_doc_comment(&func.attrs);
    }

    let description = description
        .ok_or_else(|| syn::Error::new(Span::call_site(), "#[tool] requires `description = \"...\"`"))?;

    let fn_ident = &func.sig.ident;
    let fn_name_str = tool_name.unwrap_or_else(|| fn_ident.to_string().replace('_', "-"));
    let def_fn_ident = syn::Ident::new(&format!("{fn_ident}_tool_def"), fn_ident.span());

    // ── Parse function parameters ─────────────────────────────────────────────
    struct Param {
        name: String,
        ty: Type,
        doc: String,
    }

    let mut params: Vec<Param> = Vec::new();
    for arg in &func.sig.inputs {
        match arg {
            FnArg::Typed(PatType { pat, ty, attrs, .. }) => {
                let name = match pat.as_ref() {
                    Pat::Ident(id) => id.ident.to_string(),
                    _ => {
                        return Err(syn::Error::new(pat.span(), "Only simple identifiers supported"));
                    }
                };
                let doc = extract_doc_comment(attrs).unwrap_or_default();
                params.push(Param {
                    name,
                    ty: *ty.clone(),
                    doc,
                });
            }
            FnArg::Receiver(r) => {
                return Err(syn::Error::new(r.span(), "#[tool] functions must not take `self`"));
            }
        }
    }

    // ── Build JSON Schema for input parameters ────────────────────────────────
    let prop_entries: Vec<TokenStream2> = params
        .iter()
        .map(|p| {
            let name = &p.name;
            let doc = &p.doc;
            let ty = &p.ty;
            quote! {
                #name: {
                    let mut schema = ::mcp::__private::schemars::schema_for!(#ty).schema;
                    // Inline the schema as JSON
                    let schema_val = ::mcp::__private::serde_json::to_value(&schema)
                        .expect("schema serialization failed");
                    if !#doc.is_empty() {
                        // Wrap with description
                        let mut obj = match schema_val {
                            ::mcp::__private::serde_json::Value::Object(m) => m,
                            other => {
                                let mut m = ::mcp::__private::serde_json::Map::new();
                                m.insert("type".to_string(), other);
                                m
                            }
                        };
                        obj.insert("description".to_string(), ::mcp::__private::serde_json::Value::String(#doc.to_string()));
                        ::mcp::__private::serde_json::Value::Object(obj)
                    } else {
                        schema_val
                    }
                }
            }
        })
        .collect();

    let required_entries: Vec<String> = params.iter().map(|p| p.name.clone()).collect();

    let param_extracts: Vec<TokenStream2> = params
        .iter()
        .map(|p| {
            let name_str = &p.name;
            let name_ident = syn::Ident::new(name_str, Span::call_site());
            let ty = &p.ty;
            quote! {
                let #name_ident: #ty = ::mcp::__private::serde_json::from_value(
                    args.get(#name_str)
                        .cloned()
                        .unwrap_or(::mcp::__private::serde_json::Value::Null)
                ).map_err(|e| ::mcp::McpError::InvalidParams(
                    format!("param `{}`: {}", #name_str, e)
                ))?;
            }
        })
        .collect();

    let param_names: Vec<syn::Ident> = params
        .iter()
        .map(|p| syn::Ident::new(&p.name, Span::call_site()))
        .collect();

    let fn_vis = &func.vis;

    let expanded = quote! {
        // Keep the original function unchanged
        #func

        /// Auto-generated tool definition (from `#[tool]` macro).
        #fn_vis fn #def_fn_ident() -> ::mcp::ToolDef {
            use ::mcp::__private::serde_json;

            // Build the input schema
            let mut properties = serde_json::Map::new();
            #(
                properties.insert(
                    #prop_entries
                );
            )*

            let input_schema = serde_json::json!({
                "type": "object",
                "properties": properties,
                "required": [ #(#required_entries),* ],
            });

            let tool = ::mcp::Tool::new(
                #fn_name_str,
                #description,
                input_schema,
            );

            let handler = ::std::sync::Arc::new(move |req: ::mcp::__private::CallToolRequest| {
                Box::pin(async move {
                    let args = match req.arguments {
                        serde_json::Value::Object(m) => m,
                        serde_json::Value::Null => serde_json::Map::new(),
                        other => {
                            return Err(::mcp::McpError::InvalidParams(
                                format!("expected object, got: {other}")
                            ));
                        }
                    };
                    #(#param_extracts)*
                    let result = #fn_ident(#(#param_names),*).await;
                    Ok(::mcp::__private::IntoToolResult::into_tool_result(result))
                }) as ::mcp::__private::BoxFuture<'static, ::mcp::__private::McpResult<::mcp::CallToolResult>>
            });

            ::mcp::ToolDef::new(tool, handler)
        }
    };

    Ok(expanded)
}

// ─── #[resource] ─────────────────────────────────────────────────────────────

/// Marks an async function as an MCP resource handler.
///
/// # Example
/// ```rust
/// #[resource(uri = "config://app", name = "App Config", description = "Application configuration")]
/// async fn app_config(_req: ReadResourceRequest) -> McpResult<ReadResourceResult> {
///     Ok(ReadResourceResult::text("config://app", r#"{"version": "1.0"}"#))
/// }
/// ```
#[proc_macro_attribute]
pub fn resource(args: TokenStream, input: TokenStream) -> TokenStream {
    let input_clone = input.clone();
    // For now, pass through unchanged (full implementation would generate ResourceDef)
    input_clone
}

// ─── #[prompt] ───────────────────────────────────────────────────────────────

/// Marks an async function as an MCP prompt handler.
#[proc_macro_attribute]
pub fn prompt(args: TokenStream, input: TokenStream) -> TokenStream {
    let input_clone = input.clone();
    // For now, pass through unchanged
    input_clone
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn extract_doc_comment(attrs: &[Attribute]) -> Option<String> {
    let lines: Vec<String> = attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("doc") {
                return None;
            }
            if let Meta::NameValue(MetaNameValue { value: syn::Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }), .. }) = &attr.meta {
                Some(s.value().trim().to_owned())
            } else {
                None
            }
        })
        .collect();

    if lines.is_empty() {
        None
    } else {
        Some(lines.join(" "))
    }
}

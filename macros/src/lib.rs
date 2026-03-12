extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    parse_macro_input, punctuated::Punctuated, spanned::Spanned, Attribute, FnArg, ItemFn, Lit,
    Meta, MetaNameValue, Pat, PatType, Token, Type,
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
/// use rust_mcp::prelude::*;
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

fn tool_impl(attr_args: Punctuated<Meta, Token![,]>, func: ItemFn) -> syn::Result<TokenStream2> {
    // ── Parse attribute options ───────────────────────────────────────────────
    let mut description: Option<String> = None;
    let mut tool_name: Option<String> = None;

    for meta in &attr_args {
        match meta {
            Meta::NameValue(MetaNameValue { path, value, .. }) => {
                let key = path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: Lit::Str(s), ..
                }) = value
                {
                    match key.as_str() {
                        "description" => description = Some(s.value()),
                        "name" => tool_name = Some(s.value()),
                        other => {
                            return Err(syn::Error::new(
                                path.span(),
                                format!("Unknown attribute: {other}"),
                            ));
                        }
                    }
                }
            }
            other => {
                return Err(syn::Error::new(
                    other.span(),
                    "Expected key = \"value\" pairs",
                ));
            }
        }
    }

    // Fall back to doc comment for description
    if description.is_none() {
        description = extract_doc_comment(&func.attrs);
    }

    let description = description.ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            "#[tool] requires `description = \"...\"`",
        )
    })?;

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
                        return Err(syn::Error::new(
                            pat.span(),
                            "Only simple identifiers supported",
                        ));
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
                return Err(syn::Error::new(
                    r.span(),
                    "#[tool] functions must not take `self`",
                ));
            }
        }
    }

    // ── Build JSON Schema for input parameters ────────────────────────────────
    let prop_inserts: Vec<TokenStream2> = params
        .iter()
        .map(|p| {
            let name = &p.name;
            let doc = &p.doc;
            let ty = &p.ty;
            quote! {
                {
                    let mut schema = ::rust_mcp::__private::schemars::schema_for!(#ty).schema;
                    // Inline the schema as JSON
                    let schema_val = ::rust_mcp::__private::serde_json::to_value(&schema)
                        .expect("schema serialization failed");
                    let final_val = if !#doc.is_empty() {
                        // Wrap with description
                        let mut obj = match schema_val {
                            ::rust_mcp::__private::serde_json::Value::Object(m) => m,
                            other => {
                                let mut m = ::rust_mcp::__private::serde_json::Map::new();
                                m.insert("type".to_string(), other);
                                m
                            }
                        };
                        obj.insert("description".to_string(), ::rust_mcp::__private::serde_json::Value::String(#doc.to_string()));
                        ::rust_mcp::__private::serde_json::Value::Object(obj)
                    } else {
                        schema_val
                    };
                    properties.insert(#name.to_string(), final_val);
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
                let #name_ident: #ty = ::rust_mcp::__private::serde_json::from_value(
                    args.get(#name_str)
                        .cloned()
                        .unwrap_or(::rust_mcp::__private::serde_json::Value::Null)
                ).map_err(|e| ::rust_mcp::McpError::InvalidParams(
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
        #fn_vis fn #def_fn_ident() -> ::rust_mcp::ToolDef {
            use ::rust_mcp::__private::serde_json;

            // Build the input schema
            let mut properties = serde_json::Map::new();
            #(#prop_inserts)*

            let input_schema = serde_json::json!({
                "type": "object",
                "properties": properties,
                "required": [ #(#required_entries),* ],
            });

            let tool = ::rust_mcp::Tool::new(
                #fn_name_str,
                #description,
                input_schema,
            );

            let handler = ::std::sync::Arc::new(move |req: ::rust_mcp::__private::CallToolRequest| {
                Box::pin(async move {
                    let args = match req.arguments {
                        serde_json::Value::Object(m) => m,
                        serde_json::Value::Null => serde_json::Map::new(),
                        other => {
                            return Err(::rust_mcp::McpError::InvalidParams(
                                format!("expected object, got: {other}")
                            ));
                        }
                    };
                    #(#param_extracts)*
                    let result = #fn_ident(#(#param_names),*).await;
                    Ok(::rust_mcp::__private::IntoToolResult::into_tool_result(result))
                }) as ::rust_mcp::__private::BoxFuture<'static, ::rust_mcp::__private::McpResult<::rust_mcp::CallToolResult>>
            });

            ::rust_mcp::ToolDef::new(tool, handler)
        }
    };

    Ok(expanded)
}

// ─── #[resource] ─────────────────────────────────────────────────────────────

/// Marks an async function as an MCP resource handler and generates a companion
/// `{fn_name}_resource_def()` function that returns a `mcp::ResourceDef`.
///
/// # Attributes
/// - `uri = "..."` — Resource URI (required). Use `{variable}` for templates.
/// - `name = "..."` — Human-readable name (required)
/// - `description = "..."` — Optional description
/// - `mime_type = "..."` — Optional MIME type (e.g., "application/json")
///
/// # Examples
///
/// Static resource:
/// ```rust
/// #[resource(uri = "config://app", name = "App Config", description = "Application configuration")]
/// async fn app_config(_req: ReadResourceRequest) -> McpResult<ReadResourceResult> {
///     Ok(ReadResourceResult::text("config://app", r#"{"version": "1.0"}"#))
/// }
/// ```
///
/// Template resource:
/// ```rust
/// #[resource(uri = "file://{path}", name = "File System")]
/// async fn read_file(req: ReadResourceRequest) -> McpResult<ReadResourceResult> {
///     let path = req.uri.trim_start_matches("file://");
///     let content = tokio::fs::read_to_string(path).await?;
///     Ok(ReadResourceResult::text(req.uri.clone(), content))
/// }
/// ```
#[proc_macro_attribute]
pub fn resource(args: TokenStream, input: TokenStream) -> TokenStream {
    let attr_args = parse_macro_input!(args with Punctuated::<Meta, Token![,]>::parse_terminated);
    let func = parse_macro_input!(input as ItemFn);

    match resource_impl(attr_args, func) {
        Ok(ts) => ts.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

fn resource_impl(
    attr_args: Punctuated<Meta, Token![,]>,
    func: ItemFn,
) -> syn::Result<TokenStream2> {
    // ── Parse attribute options ───────────────────────────────────────────────
    let mut uri: Option<String> = None;
    let mut name: Option<String> = None;
    let mut description: Option<String> = None;
    let mut mime_type: Option<String> = None;

    for meta in &attr_args {
        match meta {
            Meta::NameValue(MetaNameValue { path, value, .. }) => {
                let key = path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: Lit::Str(s), ..
                }) = value
                {
                    match key.as_str() {
                        "uri" => uri = Some(s.value()),
                        "name" => name = Some(s.value()),
                        "description" => description = Some(s.value()),
                        "mime_type" => mime_type = Some(s.value()),
                        other => {
                            return Err(syn::Error::new(
                                path.span(),
                                format!("Unknown attribute: {other}"),
                            ));
                        }
                    }
                }
            }
            other => {
                return Err(syn::Error::new(
                    other.span(),
                    "Expected key = \"value\" pairs",
                ));
            }
        }
    }

    let uri = uri.ok_or_else(|| {
        syn::Error::new(Span::call_site(), "#[resource] requires `uri = \"...\"`")
    })?;
    let name = name.ok_or_else(|| {
        syn::Error::new(Span::call_site(), "#[resource] requires `name = \"...\"`")
    })?;

    let fn_ident = &func.sig.ident;
    let def_fn_ident = syn::Ident::new(&format!("{fn_ident}_resource_def"), fn_ident.span());
    let fn_vis = &func.vis;

    // Check if URI is a template (contains {variable})
    let is_template = uri.contains('{');

    // Generate optional method calls
    let with_description = description.as_ref().map(|desc| {
        quote! { .with_description(#desc) }
    });
    let with_mime_type = mime_type.as_ref().map(|mime| {
        quote! { .with_mime_type(#mime) }
    });

    let expanded = if is_template {
        // Generate ResourceDef::Template
        quote! {
            // Keep the original function unchanged
            #func

            /// Auto-generated resource definition (from `#[resource]` macro).
            #fn_vis fn #def_fn_ident() -> ::rust_mcp::__private::ResourceDef {
                let template = ::rust_mcp::__private::ResourceTemplate::new(#uri, #name)
                    #with_description
                    #with_mime_type;

                let handler = ::std::sync::Arc::new(move |req: ::rust_mcp::__private::ReadResourceRequest| {
                    Box::pin(async move {
                        #fn_ident(req).await
                    }) as ::rust_mcp::__private::BoxFuture<'static, ::rust_mcp::__private::McpResult<::rust_mcp::__private::ReadResourceResult>>
                });

                ::rust_mcp::__private::ResourceDef::new_template(template, handler)
            }
        }
    } else {
        // Generate ResourceDef::Static
        quote! {
            // Keep the original function unchanged
            #func

            /// Auto-generated resource definition (from `#[resource]` macro).
            #fn_vis fn #def_fn_ident() -> ::rust_mcp::__private::ResourceDef {
                let resource = ::rust_mcp::__private::Resource::new(#uri, #name)
                    #with_description
                    #with_mime_type;

                let handler = ::std::sync::Arc::new(move |req: ::rust_mcp::__private::ReadResourceRequest| {
                    Box::pin(async move {
                        #fn_ident(req).await
                    }) as ::rust_mcp::__private::BoxFuture<'static, ::rust_mcp::__private::McpResult<::rust_mcp::__private::ReadResourceResult>>
                });

                ::rust_mcp::__private::ResourceDef::new_static(resource, handler)
            }
        }
    };

    Ok(expanded)
}

// ─── #[prompt] ───────────────────────────────────────────────────────────────

/// Marks an async function as an MCP prompt handler and generates a companion
/// `{fn_name}_prompt_def()` function that returns a `mcp::PromptDef`.
///
/// # Attributes
/// - `name = "..."` — Prompt name (defaults to function name with `-` instead of `_`)
/// - `description = "..."` — Optional description
/// - `arguments = ["arg1", "arg2:required", "arg3:optional"]` — Optional argument list
///
/// # Examples
///
/// Basic prompt:
/// ```rust
/// #[prompt(name = "greeting", description = "Generate a greeting message")]
/// async fn greeting(_req: GetPromptRequest) -> McpResult<GetPromptResult> {
///     Ok(GetPromptResult::new(vec![
///         PromptMessage::user_text("Hello! How can I help you today?")
///     ]))
/// }
/// ```
///
/// Prompt with arguments:
/// ```rust
/// #[prompt(
///     name = "code-review",
///     description = "Generate a code review",
///     arguments = ["code:required", "language:optional"]
/// )]
/// async fn code_review(req: GetPromptRequest) -> McpResult<GetPromptResult> {
///     let code = req.arguments.get("code").cloned().unwrap_or_default();
///     let lang = req.arguments.get("language").cloned().unwrap_or_else(|| "unknown".into());
///     
///     Ok(GetPromptResult::new(vec![
///         PromptMessage::user_text(format!("Review this {lang} code:\n\n```{lang}\n{code}\n```"))
///     ]))
/// }
/// ```
#[proc_macro_attribute]
pub fn prompt(args: TokenStream, input: TokenStream) -> TokenStream {
    let attr_args = parse_macro_input!(args with Punctuated::<Meta, Token![,]>::parse_terminated);
    let func = parse_macro_input!(input as ItemFn);

    match prompt_impl(attr_args, func) {
        Ok(ts) => ts.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

fn prompt_impl(attr_args: Punctuated<Meta, Token![,]>, func: ItemFn) -> syn::Result<TokenStream2> {
    // ── Parse attribute options ───────────────────────────────────────────────
    let mut prompt_name: Option<String> = None;
    let mut description: Option<String> = None;
    let mut arguments: Vec<(String, bool)> = Vec::new(); // (name, required)

    for meta in &attr_args {
        match meta {
            Meta::NameValue(MetaNameValue { path, value, .. }) => {
                let key = path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                match key.as_str() {
                    "name" => {
                        if let syn::Expr::Lit(syn::ExprLit {
                            lit: Lit::Str(s), ..
                        }) = value
                        {
                            prompt_name = Some(s.value());
                        }
                    }
                    "description" => {
                        if let syn::Expr::Lit(syn::ExprLit {
                            lit: Lit::Str(s), ..
                        }) = value
                        {
                            description = Some(s.value());
                        }
                    }
                    "arguments" => {
                        // Parse array of argument strings: ["arg1", "arg2:required", "arg3:optional"]
                        if let syn::Expr::Array(syn::ExprArray { elems, .. }) = value {
                            for elem in elems {
                                if let syn::Expr::Lit(syn::ExprLit {
                                    lit: Lit::Str(s), ..
                                }) = elem
                                {
                                    let arg_str = s.value();
                                    let (name, required) = if arg_str.contains(':') {
                                        let parts: Vec<&str> = arg_str.split(':').collect();
                                        let name = parts[0].to_string();
                                        let required =
                                            parts.get(1).map_or(true, |&r| r == "required");
                                        (name, required)
                                    } else {
                                        (arg_str, true) // default to required
                                    };
                                    arguments.push((name, required));
                                }
                            }
                        }
                    }
                    other => {
                        return Err(syn::Error::new(
                            path.span(),
                            format!("Unknown attribute: {other}"),
                        ));
                    }
                }
            }
            other => {
                return Err(syn::Error::new(
                    other.span(),
                    "Expected key = \"value\" pairs",
                ));
            }
        }
    }

    let fn_ident = &func.sig.ident;
    let prompt_name = prompt_name.unwrap_or_else(|| fn_ident.to_string().replace('_', "-"));
    let def_fn_ident = syn::Ident::new(&format!("{fn_ident}_prompt_def"), fn_ident.span());
    let fn_vis = &func.vis;

    // Generate argument definitions
    let arg_definitions: Vec<TokenStream2> = arguments
        .iter()
        .map(|(name, required)| {
            if *required {
                quote! {
                    ::rust_mcp::__private::PromptArgument::required(#name)
                }
            } else {
                quote! {
                    ::rust_mcp::__private::PromptArgument::optional(#name)
                }
            }
        })
        .collect();

    let with_args = if !arguments.is_empty() {
        quote! {
            .with_arguments(vec![#(#arg_definitions),*])
        }
    } else {
        quote! {}
    };

    let with_desc = if let Some(desc) = &description {
        quote! {
            .with_description(#desc)
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        // Keep the original function unchanged
        #func

        /// Auto-generated prompt definition (from `#[prompt]` macro).
        #fn_vis fn #def_fn_ident() -> ::rust_mcp::__private::PromptDef {
            let prompt = ::rust_mcp::__private::Prompt::new(#prompt_name)
                #with_desc
                #with_args;

            let handler = ::std::sync::Arc::new(move |req: ::rust_mcp::__private::GetPromptRequest| {
                Box::pin(async move {
                    #fn_ident(req).await
                }) as ::rust_mcp::__private::BoxFuture<'static, ::rust_mcp::__private::McpResult<::rust_mcp::__private::GetPromptResult>>
            });

            ::rust_mcp::__private::PromptDef::new(prompt, handler)
        }
    };

    Ok(expanded)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn extract_doc_comment(attrs: &[Attribute]) -> Option<String> {
    let lines: Vec<String> = attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("doc") {
                return None;
            }
            if let Meta::NameValue(MetaNameValue {
                value:
                    syn::Expr::Lit(syn::ExprLit {
                        lit: Lit::Str(s), ..
                    }),
                ..
            }) = &attr.meta
            {
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

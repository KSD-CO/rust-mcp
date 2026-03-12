use std::{collections::HashMap, sync::Arc};

use mcp_core::{
    error::McpResult,
    types::{
        messages::{
            CallToolRequest, GetPromptRequest, ListPromptsResult, ListResourcesResult,
            ListToolsResult, ReadResourceRequest,
        },
        prompt::{GetPromptResult, Prompt},
        resource::{ReadResourceResult, Resource, ResourceTemplate},
        tool::{CallToolResult, Tool},
    },
};

use crate::handler::{PromptHandlerFn, ResourceHandlerFn, ToolHandlerFn};

// ─── Tool route ───────────────────────────────────────────────────────────────

pub struct ToolRoute {
    pub tool: Tool,
    pub handler: ToolHandlerFn,
}

// ─── Resource route ───────────────────────────────────────────────────────────

pub struct ResourceRoute {
    pub resource: Resource,
    pub handler: ResourceHandlerFn,
}

pub struct ResourceTemplateRoute {
    pub template: ResourceTemplate,
    pub handler: ResourceHandlerFn,
}

// ─── Prompt route ─────────────────────────────────────────────────────────────

pub struct PromptRoute {
    pub prompt: Prompt,
    pub handler: PromptHandlerFn,
}

// ─── Router ───────────────────────────────────────────────────────────────────

/// Central routing table — maps method names to handlers.
#[derive(Default)]
pub struct Router {
    tools: HashMap<String, ToolRoute>,
    resources: HashMap<String, ResourceRoute>,
    resource_templates: Vec<ResourceTemplateRoute>,
    prompts: HashMap<String, PromptRoute>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    // ── Tool registration ─────────────────────────────────────────────────────

    pub fn add_tool(&mut self, tool: Tool, handler: ToolHandlerFn) {
        self.tools.insert(
            tool.name.clone(),
            ToolRoute { tool, handler },
        );
    }

    pub fn list_tools(&self, _cursor: Option<&str>) -> ListToolsResult {
        let tools: Vec<Tool> = self.tools.values().map(|r| r.tool.clone()).collect();
        ListToolsResult {
            tools,
            next_cursor: None,
        }
    }

    pub async fn call_tool(&self, req: CallToolRequest) -> McpResult<CallToolResult> {
        let route = self
            .tools
            .get(&req.name)
            .ok_or_else(|| mcp_core::error::McpError::ToolNotFound(req.name.clone()))?;
        (route.handler)(req).await
    }

    // ── Resource registration ─────────────────────────────────────────────────

    pub fn add_resource(&mut self, resource: Resource, handler: ResourceHandlerFn) {
        self.resources.insert(
            resource.uri.clone(),
            ResourceRoute { resource, handler },
        );
    }

    pub fn add_resource_template(&mut self, template: ResourceTemplate, handler: ResourceHandlerFn) {
        self.resource_templates
            .push(ResourceTemplateRoute { template, handler });
    }

    pub fn list_resources(&self, _cursor: Option<&str>) -> ListResourcesResult {
        let resources: Vec<Resource> = self.resources.values().map(|r| r.resource.clone()).collect();
        ListResourcesResult {
            resources,
            next_cursor: None,
        }
    }

    pub async fn read_resource(&self, req: ReadResourceRequest) -> McpResult<ReadResourceResult> {
        // Exact match first
        if let Some(route) = self.resources.get(&req.uri) {
            return (route.handler)(req).await;
        }

        // Template match
        for tpl in &self.resource_templates {
            if uri_matches_template(&req.uri, &tpl.template.uri_template) {
                return (tpl.handler)(req).await;
            }
        }

        Err(mcp_core::error::McpError::ResourceNotFound(req.uri))
    }

    // ── Prompt registration ───────────────────────────────────────────────────

    pub fn add_prompt(&mut self, prompt: Prompt, handler: PromptHandlerFn) {
        self.prompts.insert(
            prompt.name.clone(),
            PromptRoute { prompt, handler },
        );
    }

    pub fn list_prompts(&self, _cursor: Option<&str>) -> ListPromptsResult {
        let prompts: Vec<Prompt> = self.prompts.values().map(|r| r.prompt.clone()).collect();
        ListPromptsResult {
            prompts,
            next_cursor: None,
        }
    }

    pub async fn get_prompt(&self, req: GetPromptRequest) -> McpResult<GetPromptResult> {
        let route = self
            .prompts
            .get(&req.name)
            .ok_or_else(|| mcp_core::error::McpError::PromptNotFound(req.name.clone()))?;
        (route.handler)(req).await
    }

    // ── Capability introspection ──────────────────────────────────────────────

    pub fn has_tools(&self) -> bool {
        !self.tools.is_empty()
    }

    pub fn has_resources(&self) -> bool {
        !self.resources.is_empty() || !self.resource_templates.is_empty()
    }

    pub fn has_prompts(&self) -> bool {
        !self.prompts.is_empty()
    }
}

/// Naive URI template matcher (supports {variable} placeholders)
fn uri_matches_template(uri: &str, template: &str) -> bool {
    let re = template_to_regex(template);
    regex_match(uri, &re)
}

fn template_to_regex(template: &str) -> String {
    let mut re = String::from("^");
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            // consume until '}'
            while let Some(inner) = chars.next() {
                if inner == '}' {
                    break;
                }
            }
            re.push_str("[^/]+");
        } else {
            // Escape regex special chars
            match c {
                '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '^' | '$' | '|' | '\\' => {
                    re.push('\\');
                    re.push(c);
                }
                _ => re.push(c),
            }
        }
    }
    re.push('$');
    re
}

fn regex_match(uri: &str, pattern: &str) -> bool {
    // Minimal regex matching without pulling in `regex` crate
    // Handles the pattern produced by template_to_regex
    pattern_match(uri, pattern.trim_start_matches('^').trim_end_matches('$'))
}

fn pattern_match(text: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return text.is_empty();
    }
    if pattern.starts_with("[^/]+") {
        let rest_pattern = &pattern["[^/]+".len()..];
        // Match one or more non-'/' characters
        let slash_pos = text.find('/').unwrap_or(text.len());
        if slash_pos == 0 {
            return false;
        }
        for end in 1..=slash_pos {
            if pattern_match(&text[end..], rest_pattern) {
                return true;
            }
        }
        false
    } else {
        // Literal character (possibly escaped)
        let (pat_char, rest_pat) = if pattern.starts_with('\\') && pattern.len() >= 2 {
            (pattern.chars().nth(1).unwrap(), &pattern[2..])
        } else {
            (pattern.chars().next().unwrap(), &pattern[pattern.chars().next().unwrap().len_utf8()..])
        };
        if text.starts_with(pat_char) {
            pattern_match(&text[pat_char.len_utf8()..], rest_pat)
        } else {
            false
        }
    }
}

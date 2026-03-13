//! Global completion handler — fallback for unhandled completion requests.

use mcp_kit::{
    error::McpResult,
    types::messages::{CompleteRequest, CompleteResult, CompletionReference},
};

/// Global completion handler for resources and prompts without specific handlers.
pub async fn global_completion_handler(req: CompleteRequest) -> McpResult<CompleteResult> {
    match &req.reference {
        CompletionReference::Resource { uri } => handle_resource_completion(uri, &req),
        CompletionReference::Prompt { name } => handle_prompt_completion(name, &req),
    }
}

fn handle_resource_completion(uri: &str, req: &CompleteRequest) -> McpResult<CompleteResult> {
    // Auto-complete for resource URI arguments
    let values: Vec<String> = if uri.starts_with("user://") {
        // Suggest some user IDs
        vec!["1", "2", "3", "admin", "guest"]
            .iter()
            .filter(|id| id.starts_with(&req.argument.value))
            .map(|s| s.to_string())
            .collect()
    } else if uri.starts_with("doc://") {
        // Suggest some document IDs
        vec!["readme", "guide", "api", "changelog"]
            .iter()
            .filter(|id| id.starts_with(&req.argument.value))
            .map(|s| s.to_string())
            .collect()
    } else {
        vec![]
    };

    Ok(CompleteResult::new(values))
}

fn handle_prompt_completion(name: &str, req: &CompleteRequest) -> McpResult<CompleteResult> {
    // Fallback completions for prompts without specific handlers
    let values: Vec<String> = match (name, req.argument.name.as_str()) {
        ("summarize", "max_sentences") => vec!["1", "3", "5", "10"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ("translate", "target_language") => {
            let languages = [
                "Spanish",
                "French",
                "German",
                "Italian",
                "Portuguese",
                "Japanese",
                "Chinese",
                "Korean",
                "Russian",
                "Arabic",
            ];
            languages
                .iter()
                .filter(|l| {
                    req.argument.value.is_empty()
                        || l.to_lowercase()
                            .starts_with(&req.argument.value.to_lowercase())
                })
                .map(|s| s.to_string())
                .collect()
        }
        _ => vec![],
    };

    Ok(CompleteResult::new(values))
}

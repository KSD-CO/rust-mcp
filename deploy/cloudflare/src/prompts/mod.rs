//! Prompt handlers: code-review, summarize, translate.

use mcp_kit::{
    error::McpResult,
    types::{
        messages::{CompleteRequest, CompleteResult, GetPromptRequest},
        prompt::{GetPromptResult, Prompt, PromptArgument, PromptMessage},
    },
};

// ─── Code Review Prompt ───────────────────────────────────────────────────────

pub fn code_review_prompt() -> Prompt {
    Prompt::new("code-review")
        .with_description("Review code for bugs and improvements")
        .with_arguments(vec![
            PromptArgument::required("code").with_description("The code to review"),
            PromptArgument::optional("language").with_description("Programming language"),
        ])
}

pub async fn code_review_handler(req: GetPromptRequest) -> McpResult<GetPromptResult> {
    let code = req.arguments.get("code").map(|s| s.as_str()).unwrap_or("");
    let lang = req
        .arguments
        .get("language")
        .map(|s| s.as_str())
        .unwrap_or("unknown");

    Ok(GetPromptResult::new(vec![PromptMessage::user_text(format!(
        "Please review the following {} code for:\n\
         1. Potential bugs and errors\n\
         2. Performance improvements\n\
         3. Code style and best practices\n\
         4. Security vulnerabilities\n\n\
         ```{}\n{}\n```",
        lang, lang, code
    ))])
    .with_description(format!("Code review for {} code", lang)))
}

/// Completion handler for code-review prompt arguments.
pub async fn code_review_completion(req: CompleteRequest) -> McpResult<CompleteResult> {
    let values: Vec<String> = match req.argument.name.as_str() {
        "language" => {
            // Filter languages based on current input
            let languages = [
                "rust",
                "python",
                "javascript",
                "typescript",
                "go",
                "java",
                "c",
                "cpp",
                "csharp",
                "ruby",
                "php",
                "swift",
                "kotlin",
            ];
            languages
                .iter()
                .filter(|l| {
                    req.argument.value.is_empty()
                        || l.starts_with(&req.argument.value.to_lowercase())
                })
                .map(|s| s.to_string())
                .collect()
        }
        _ => vec![],
    };

    Ok(CompleteResult::new(values))
}

// ─── Summarize Prompt ─────────────────────────────────────────────────────────

pub fn summarize_prompt() -> Prompt {
    Prompt::new("summarize")
        .with_description("Summarize text content")
        .with_arguments(vec![
            PromptArgument::required("text").with_description("The text to summarize"),
            PromptArgument::optional("max_sentences")
                .with_description("Maximum sentences in summary"),
        ])
}

pub async fn summarize_handler(req: GetPromptRequest) -> McpResult<GetPromptResult> {
    let text = req.arguments.get("text").map(|s| s.as_str()).unwrap_or("");
    let max = req
        .arguments
        .get("max_sentences")
        .map(|s| s.as_str())
        .unwrap_or("3");

    Ok(
        GetPromptResult::new(vec![PromptMessage::user_text(format!(
            "Please summarize the following text in {} sentences or fewer:\n\n{}",
            max, text
        ))])
        .with_description("Text summarization"),
    )
}

// ─── Translate Prompt ─────────────────────────────────────────────────────────

pub fn translate_prompt() -> Prompt {
    Prompt::new("translate")
        .with_description("Translate text to another language")
        .with_arguments(vec![
            PromptArgument::required("text").with_description("The text to translate"),
            PromptArgument::required("target_language")
                .with_description("Target language (e.g., Spanish)"),
        ])
}

pub async fn translate_handler(req: GetPromptRequest) -> McpResult<GetPromptResult> {
    let text = req.arguments.get("text").map(|s| s.as_str()).unwrap_or("");
    let target = req
        .arguments
        .get("target_language")
        .map(|s| s.as_str())
        .unwrap_or("English");

    Ok(GetPromptResult::new(vec![PromptMessage::user_text(format!(
        "Please translate the following text to {}:\n\n{}",
        target, text
    ))])
    .with_description(format!("Translation to {}", target)))
}

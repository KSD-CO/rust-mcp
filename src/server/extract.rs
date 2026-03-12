/// Extractors for tool/resource/prompt handlers — inspired by axum's extractor pattern.
use crate::error::{McpError, McpResult};
use serde::de::DeserializeOwned;

// ─── Json<T> extractor ────────────────────────────────────────────────────────

/// Deserialize the full arguments object as type `T`.
pub struct Json<T>(pub T);

impl<T: DeserializeOwned> Json<T> {
    pub fn from_value(v: serde_json::Value) -> McpResult<Self> {
        serde_json::from_value(v)
            .map(Json)
            .map_err(|e| McpError::InvalidParams(e.to_string()))
    }
}

impl<T> std::ops::Deref for Json<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// ─── State<S> extractor ───────────────────────────────────────────────────────

/// Share arbitrary state across handlers.
#[derive(Clone)]
pub struct State<S>(pub S);

impl<S> std::ops::Deref for State<S> {
    type Target = S;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// ─── Extension<T> extractor ───────────────────────────────────────────────────

/// Type-map extension data attached to a session.
#[derive(Clone)]
pub struct Extension<T>(pub T);

impl<T> std::ops::Deref for Extension<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

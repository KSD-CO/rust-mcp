/// Extractors for tool/resource/prompt handlers — inspired by axum's extractor pattern.
use mcp_core::error::{McpError, McpResult};
use serde::de::DeserializeOwned;

// ─── Json<T> extractor ────────────────────────────────────────────────────────

/// Deserialize the full arguments object as type `T`.
///
/// # Example
/// ```rust
/// #[derive(serde::Deserialize)]
/// struct Params { a: f64, b: f64 }
///
/// async fn add(Json(p): Json<Params>) -> String {
///     format!("{}", p.a + p.b)
/// }
/// ```
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
///
/// Register state with `McpServerBuilder::state(my_state)` and extract it in handlers:
///
/// # Example
/// ```rust
/// async fn my_tool(State(db): State<Arc<Database>>) -> String {
///     db.query().await
/// }
/// ```
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

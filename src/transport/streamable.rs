//! Streamable HTTP transport for MCP (2025-03-26 spec).
//!
//! This transport uses a single HTTP endpoint that can return either:
//! - JSON response for simple requests
//! - SSE stream for long-running operations
//!
//! # Example
//!
//! ```rust,ignore
//! use mcp_kit::prelude::*;
//!
//! let server = McpServer::builder()
//!     .name("my-server")
//!     .version("1.0.0")
//!     .build();
//!
//! // Serve on a single endpoint
//! server.serve_streamable(([127, 0, 0, 1], 3000)).await?;
//! ```
//!
//! # Protocol
//!
//! ```text
//! POST /mcp
//! Content-Type: application/json
//! Mcp-Session-Id: <optional session id>
//!
//! {"jsonrpc":"2.0","method":"...","id":1}
//!
//! Response (JSON for simple requests):
//! 200 OK
//! Content-Type: application/json
//! Mcp-Session-Id: <session id>
//!
//! {"jsonrpc":"2.0","result":{...},"id":1}
//!
//! Response (SSE for streaming):
//! 200 OK
//! Content-Type: text/event-stream
//! Mcp-Session-Id: <session id>
//!
//! data: {"jsonrpc":"2.0","method":"notifications/progress",...}
//! data: {"jsonrpc":"2.0","result":{...},"id":1}
//! ```

use std::{convert::Infallible, sync::Arc, time::Duration};

use crate::server::{core::McpServer, session::Session};
use crate::{error::McpResult, protocol::JsonRpcMessage};
use axum::{
    extract::State as AxumState,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{sse::Event, IntoResponse, Response, Sse},
    routing::{delete, get, post},
    Json as AxumJson, Router as AxumRouter,
};
use dashmap::DashMap;
use futures_util::stream;
use std::future::Future;
use tokio::sync::mpsc;
use tracing::info;
use uuid::Uuid;

#[cfg(feature = "auth")]
use crate::auth::{AuthenticatedIdentity, DynAuthProvider};
#[cfg(feature = "auth")]
use crate::transport::auth_layer::{auth_middleware, AuthMiddlewareState};

// ─── Constants ────────────────────────────────────────────────────────────────

const MCP_SESSION_ID_HEADER: &str = "Mcp-Session-Id";
const SESSION_TIMEOUT_SECS: u64 = 3600; // 1 hour

// ─── Shared State ─────────────────────────────────────────────────────────────

type NotificationTx = mpsc::Sender<JsonRpcMessage>;

pub(crate) struct SessionEntry {
    session: Arc<tokio::sync::Mutex<Session>>,
    notification_tx: Option<NotificationTx>,
    last_active: std::time::Instant,
}

#[derive(Clone)]
pub struct StreamableState {
    pub(crate) server: Arc<McpServer>,
    pub(crate) sessions: Arc<DashMap<String, SessionEntry>>,
    #[cfg(feature = "auth")]
    #[allow(dead_code)]
    pub(crate) auth: Option<AuthMiddlewareState>,
}

impl StreamableState {
    fn get_or_create_session(
        &self,
        session_id: Option<&str>,
    ) -> (String, Arc<tokio::sync::Mutex<Session>>) {
        if let Some(sid) = session_id {
            if let Some(mut entry) = self.sessions.get_mut(sid) {
                entry.last_active = std::time::Instant::now();
                return (sid.to_string(), entry.session.clone());
            }
        }

        // Create new session
        let session_id = Uuid::new_v4().to_string();
        let session = Arc::new(tokio::sync::Mutex::new(Session::new()));
        self.sessions.insert(
            session_id.clone(),
            SessionEntry {
                session: session.clone(),
                notification_tx: None,
                last_active: std::time::Instant::now(),
            },
        );
        info!(session_id = %session_id, "Created new session");
        (session_id, session)
    }

    fn set_notification_channel(&self, session_id: &str, tx: NotificationTx) {
        if let Some(mut entry) = self.sessions.get_mut(session_id) {
            entry.notification_tx = Some(tx);
        }
    }

    fn remove_notification_channel(&self, session_id: &str) {
        if let Some(mut entry) = self.sessions.get_mut(session_id) {
            entry.notification_tx = None;
        }
    }

    fn remove_session(&self, session_id: &str) {
        self.sessions.remove(session_id);
        info!(session_id = %session_id, "Session terminated");
    }

    fn cleanup_expired_sessions(&self) {
        let timeout = Duration::from_secs(SESSION_TIMEOUT_SECS);
        let now = std::time::Instant::now();

        self.sessions.retain(|sid, entry| {
            let expired = now.duration_since(entry.last_active) > timeout;
            if expired {
                info!(session_id = %sid, "Session expired");
            }
            !expired
        });
    }
}

// ─── StreamableTransport ──────────────────────────────────────────────────────

/// Streamable HTTP transport (MCP 2025-03-26 spec).
pub struct StreamableTransport {
    server: McpServer,
    addr: std::net::SocketAddr,
    endpoint: String,
    #[cfg(feature = "auth")]
    auth: Option<AuthMiddlewareState>,
}

impl StreamableTransport {
    /// Create a new Streamable HTTP transport.
    pub fn new(server: McpServer, addr: impl Into<std::net::SocketAddr>) -> Self {
        Self {
            server,
            addr: addr.into(),
            endpoint: "/mcp".to_string(),
            #[cfg(feature = "auth")]
            auth: None,
        }
    }

    /// Set custom endpoint path (default: "/mcp").
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Require authentication on all requests.
    #[cfg(feature = "auth")]
    pub fn with_auth(mut self, provider: DynAuthProvider) -> Self {
        self.auth = Some(AuthMiddlewareState {
            provider,
            require_auth: true,
        });
        self
    }

    /// Accept optional authentication.
    #[cfg(feature = "auth")]
    pub fn with_optional_auth(mut self, provider: DynAuthProvider) -> Self {
        self.auth = Some(AuthMiddlewareState {
            provider,
            require_auth: false,
        });
        self
    }

    /// Build the Axum router for this transport.
    pub fn build_router(self) -> (AxumRouter, StreamableState) {
        let state = StreamableState {
            server: Arc::new(self.server),
            sessions: Arc::new(DashMap::new()),
            #[cfg(feature = "auth")]
            auth: self.auth.clone(),
        };

        let routes = AxumRouter::new()
            .route(&self.endpoint, post(handle_post))
            .route(&self.endpoint, get(handle_get_sse))
            .route(&self.endpoint, delete(handle_delete));

        #[cfg(feature = "auth")]
        let routes = if let Some(auth_state) = self.auth {
            routes.route_layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ))
        } else {
            routes
        };

        (routes.with_state(state.clone()), state)
    }

    /// Start serving on the configured address.
    pub async fn serve(self) -> McpResult<()> {
        let addr = self.addr;
        let (router, state) = self.build_router();

        // Spawn session cleanup task
        let cleanup_state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 minutes
            loop {
                interval.tick().await;
                cleanup_state.cleanup_expired_sessions();
            }
        });

        info!(addr = %addr, "Streamable HTTP transport listening");

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(crate::error::McpError::Io)?;

        axum::serve(listener, router)
            .await
            .map_err(|e| crate::error::McpError::Transport(e.to_string()))?;

        Ok(())
    }
}

// ─── POST Handler ─────────────────────────────────────────────────────────────

async fn handle_post(
    AxumState(state): AxumState<StreamableState>,
    headers: HeaderMap,
    #[cfg(feature = "auth")] identity: Option<axum::Extension<Arc<AuthenticatedIdentity>>>,
    AxumJson(msg): AxumJson<JsonRpcMessage>,
) -> Response {
    // Get or create session
    let session_id_header = headers
        .get(MCP_SESSION_ID_HEADER)
        .and_then(|v| v.to_str().ok());

    let (session_id, session_arc) = state.get_or_create_session(session_id_header);

    let mut session = session_arc.lock().await;

    // Set identity if authenticated
    #[cfg(feature = "auth")]
    if let Some(axum::Extension(id)) = identity {
        session.identity = Some((*id).clone());
    }

    // Check if this is a method that might need streaming
    let needs_streaming = matches!(&msg, JsonRpcMessage::Request(req) if
        req.method == "tools/call" ||
        req.method == "sampling/createMessage"
    );

    let server = state.server.clone();

    // Handle the message
    match server.handle_message(msg, &mut session).await {
        Some(response) => {
            if needs_streaming {
                // For potentially long-running operations, use SSE
                stream_response(session_id, response)
            } else {
                // Simple JSON response
                json_response(session_id, response)
            }
        }
        None => {
            // Notification - no response needed
            (StatusCode::ACCEPTED, [(MCP_SESSION_ID_HEADER, session_id)]).into_response()
        }
    }
}

fn json_response(session_id: String, response: JsonRpcMessage) -> Response {
    let mut resp = AxumJson(response).into_response();
    resp.headers_mut().insert(
        MCP_SESSION_ID_HEADER,
        HeaderValue::from_str(&session_id).unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    resp
}

fn stream_response(session_id: String, response: JsonRpcMessage) -> Response {
    let event = Event::default()
        .event("message")
        .data(serde_json::to_string(&response).unwrap_or_default());

    let stream = stream::once(async move { Ok::<_, Infallible>(event) });

    let mut resp = Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::default())
        .into_response();

    resp.headers_mut().insert(
        MCP_SESSION_ID_HEADER,
        HeaderValue::from_str(&session_id).unwrap_or_else(|_| HeaderValue::from_static("")),
    );

    resp
}

// ─── GET Handler (SSE stream for server-initiated messages) ───────────────────

async fn handle_get_sse(
    AxumState(state): AxumState<StreamableState>,
    headers: HeaderMap,
    #[cfg(feature = "auth")] _identity: Option<axum::Extension<Arc<AuthenticatedIdentity>>>,
) -> Response {
    // Require existing session for SSE
    let Some(session_id) = headers
        .get(MCP_SESSION_ID_HEADER)
        .and_then(|v| v.to_str().ok())
    else {
        return (
            StatusCode::BAD_REQUEST,
            "Mcp-Session-Id header required for SSE",
        )
            .into_response();
    };

    // Verify session exists
    if !state.sessions.contains_key(session_id) {
        return (StatusCode::NOT_FOUND, "Session not found").into_response();
    }

    let session_id = session_id.to_string();

    // Create notification channel
    let (tx, rx) = mpsc::channel::<JsonRpcMessage>(64);
    state.set_notification_channel(&session_id, tx);

    info!(session_id = %session_id, "SSE stream opened");

    let cleanup_state = state.clone();
    let session_id_for_cleanup = session_id.clone();

    let stream = stream::unfold(rx, move |mut rx| async move {
        match rx.recv().await {
            Some(msg) => {
                let data = serde_json::to_string(&msg).unwrap_or_default();
                let event = Event::default().event("message").data(data);
                Some((Ok::<_, Infallible>(event), rx))
            }
            None => None,
        }
    });

    // Spawn cleanup task for when client disconnects
    tokio::spawn(async move {
        // This task monitors connection - cleanup happens when stream ends
        tokio::time::sleep(Duration::from_secs(SESSION_TIMEOUT_SECS)).await;
        cleanup_state.remove_notification_channel(&session_id_for_cleanup);
        info!(session_id = %session_id_for_cleanup, "SSE stream timeout cleanup");
    });

    let mut resp = Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::default())
        .into_response();

    resp.headers_mut().insert(
        MCP_SESSION_ID_HEADER,
        HeaderValue::from_str(&session_id).unwrap_or_else(|_| HeaderValue::from_static("")),
    );

    resp
}

// ─── DELETE Handler (terminate session) ───────────────────────────────────────

async fn handle_delete(
    AxumState(state): AxumState<StreamableState>,
    headers: HeaderMap,
) -> Response {
    let Some(session_id) = headers
        .get(MCP_SESSION_ID_HEADER)
        .and_then(|v| v.to_str().ok())
    else {
        return (StatusCode::BAD_REQUEST, "Mcp-Session-Id header required").into_response();
    };

    if state.sessions.contains_key(session_id) {
        state.remove_session(session_id);
        StatusCode::NO_CONTENT.into_response()
    } else {
        (StatusCode::NOT_FOUND, "Session not found").into_response()
    }
}

// ─── Extension Trait ──────────────────────────────────────────────────────────

/// Extension trait that adds `.serve_streamable()` to `McpServer`.
pub trait ServeStreamableExt {
    /// Serve using the Streamable HTTP transport.
    fn serve_streamable(
        self,
        addr: impl Into<std::net::SocketAddr>,
    ) -> impl Future<Output = McpResult<()>> + Send;
}

impl ServeStreamableExt for McpServer {
    fn serve_streamable(
        self,
        addr: impl Into<std::net::SocketAddr>,
    ) -> impl Future<Output = McpResult<()>> + Send {
        #[cfg(feature = "auth")]
        {
            let transport = StreamableTransport::new(self.clone(), addr);
            let transport = match (self.auth_provider, self.require_auth) {
                (Some(provider), true) => transport.with_auth(provider),
                (Some(provider), false) => transport.with_optional_auth(provider),
                (None, _) => transport,
            };
            transport.serve()
        }
        #[cfg(not(feature = "auth"))]
        StreamableTransport::new(self, addr).serve()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let state = StreamableState {
            server: Arc::new(McpServer::builder().name("test").version("1.0").build()),
            sessions: Arc::new(DashMap::new()),
            #[cfg(feature = "auth")]
            auth: None,
        };

        // Create new session
        let (sid1, _) = state.get_or_create_session(None);
        assert!(!sid1.is_empty());
        assert!(state.sessions.contains_key(&sid1));

        // Get existing session
        let (sid2, _) = state.get_or_create_session(Some(&sid1));
        assert_eq!(sid1, sid2);
    }
}

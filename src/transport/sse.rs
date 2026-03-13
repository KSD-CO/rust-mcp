/// SSE (Server-Sent Events) + HTTP POST transport for MCP.
///
///   GET  /sse          → opens SSE stream, client receives messages from server
///   POST /message      → client sends JSON-RPC messages to server
use std::{convert::Infallible, sync::Arc};

use crate::server::{core::McpServer, session::Session};
use crate::{error::McpResult, protocol::JsonRpcMessage};
use axum::{
    extract::{Query, State as AxumState},
    http::StatusCode,
    response::{sse::Event, IntoResponse, Response, Sse},
    routing::{get, post},
    Json as AxumJson, Router as AxumRouter,
};
use dashmap::DashMap;
use futures_util::stream;
use std::future::Future;
use tokio::sync::mpsc;
use tracing::{error, info};
use uuid::Uuid;

#[cfg(feature = "auth")]
use crate::auth::{AuthenticatedIdentity, DynAuthProvider};
#[cfg(feature = "auth")]
use crate::transport::auth_layer::{auth_middleware, AuthMiddlewareState};

// ─── Shared SSE state ─────────────────────────────────────────────────────────

type SessionTx = mpsc::Sender<JsonRpcMessage>;
type SessionData = (SessionTx, Arc<tokio::sync::Mutex<Session>>);

#[derive(Clone)]
pub struct SseState {
    pub server: Arc<McpServer>,
    pub sessions: Arc<DashMap<String, SessionData>>,
    #[cfg(feature = "auth")]
    pub auth: Option<AuthMiddlewareState>,
}

// ─── SseTransport ─────────────────────────────────────────────────────────────

pub struct SseTransport {
    server: McpServer,
    addr: std::net::SocketAddr,
    #[cfg(feature = "auth")]
    auth: Option<AuthMiddlewareState>,
}

impl SseTransport {
    pub fn new(server: McpServer, addr: impl Into<std::net::SocketAddr>) -> Self {
        Self {
            server,
            addr: addr.into(),
            #[cfg(feature = "auth")]
            auth: None,
        }
    }

    /// Require authentication on all requests using the given provider.
    /// Requests with no or invalid credentials receive HTTP 401.
    #[cfg(feature = "auth")]
    pub fn with_auth(mut self, provider: DynAuthProvider) -> Self {
        self.auth = Some(AuthMiddlewareState {
            provider,
            require_auth: true,
        });
        self
    }

    /// Accept an auth provider but allow unauthenticated requests through.
    /// Authenticated requests will have an identity; unauthenticated ones will not.
    #[cfg(feature = "auth")]
    pub fn with_optional_auth(mut self, provider: DynAuthProvider) -> Self {
        self.auth = Some(AuthMiddlewareState {
            provider,
            require_auth: false,
        });
        self
    }

    pub async fn serve(self) -> McpResult<()> {
        let state = SseState {
            server: Arc::new(self.server),
            sessions: Arc::new(DashMap::new()),
            #[cfg(feature = "auth")]
            auth: self.auth,
        };

        let app = build_router(state);

        info!(addr = %self.addr, "SSE transport listening");

        let listener = tokio::net::TcpListener::bind(self.addr)
            .await
            .map_err(crate::error::McpError::Io)?;

        axum::serve(listener, app)
            .await
            .map_err(|e| crate::error::McpError::Transport(e.to_string()))?;

        Ok(())
    }
}

pub(crate) fn build_router(state: SseState) -> AxumRouter {
    let routes = AxumRouter::new()
        .route("/sse", get(sse_handler))
        .route("/message", post(message_handler));

    #[cfg(feature = "auth")]
    if let Some(auth_state) = state.auth.clone() {
        return routes
            .route_layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ))
            .with_state(state);
    }

    routes.with_state(state)
}

// ─── GET /sse ─────────────────────────────────────────────────────────────────

async fn sse_handler(
    AxumState(state): AxumState<SseState>,
    #[cfg(feature = "auth")] identity: Option<axum::Extension<Arc<AuthenticatedIdentity>>>,
) -> Response {
    let session_id = Uuid::new_v4().to_string();
    let (tx, rx) = mpsc::channel::<JsonRpcMessage>(64);
    let session = Arc::new(tokio::sync::Mutex::new(Session::new()));

    #[cfg(feature = "auth")]
    if let Some(axum::Extension(id)) = identity {
        session.lock().await.identity = Some((*id).clone());
    }

    state
        .sessions
        .insert(session_id.clone(), (tx, session.clone()));
    info!(session_id = %session_id, "New SSE connection");

    let sid = session_id.clone();
    let init_event = Event::default()
        .event("endpoint")
        .data(format!("/message?sessionId={sid}"));

    let stream = stream::unfold(
        (rx, session_id.clone(), state.clone()),
        |(rx, sid, state)| async move {
            let mut rx = rx;
            match rx.recv().await {
                Some(msg) => {
                    let data = serde_json::to_string(&msg).unwrap_or_default();
                    let event = Event::default().event("message").data(data);
                    Some((Ok::<_, Infallible>(event), (rx, sid, state)))
                }
                None => {
                    state.sessions.remove(&sid);
                    None
                }
            }
        },
    );

    let combined = futures_util::StreamExt::chain(
        stream::once(async move { Ok::<_, Infallible>(init_event) }),
        stream,
    );

    Sse::new(combined)
        .keep_alive(axum::response::sse::KeepAlive::default())
        .into_response()
}

// ─── POST /message ────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct MessageQuery {
    #[serde(rename = "sessionId")]
    session_id: String,
}

async fn message_handler(
    AxumState(state): AxumState<SseState>,
    Query(query): Query<MessageQuery>,
    #[cfg(feature = "auth")] identity: Option<axum::Extension<Arc<AuthenticatedIdentity>>>,
    AxumJson(msg): AxumJson<JsonRpcMessage>,
) -> impl IntoResponse {
    let entry = state.sessions.get(&query.session_id);
    let Some(entry) = entry else {
        return (StatusCode::NOT_FOUND, "Session not found").into_response();
    };

    let (tx, session_arc) = entry.value().clone();
    drop(entry);

    let mut session = session_arc.lock().await;

    // Refresh the identity on every POST so it reflects the current request's auth.
    #[cfg(feature = "auth")]
    if let Some(axum::Extension(id)) = identity {
        session.identity = Some((*id).clone());
    }

    let server = state.server.clone();

    match server.handle_message(msg, &mut session).await {
        Some(response) => {
            if tx.send(response).await.is_err() {
                error!(session_id = %query.session_id, "Failed to send SSE response");
            }
            StatusCode::OK.into_response()
        }
        None => StatusCode::ACCEPTED.into_response(),
    }
}

/// Extension trait that adds `.serve_sse()` to `McpServer`.
pub trait ServeSseExt {
    fn serve_sse(
        self,
        addr: impl Into<std::net::SocketAddr>,
    ) -> impl Future<Output = McpResult<()>> + Send;
}

impl ServeSseExt for McpServer {
    fn serve_sse(
        self,
        addr: impl Into<std::net::SocketAddr>,
    ) -> impl Future<Output = McpResult<()>> + Send {
        #[cfg(feature = "auth")]
        {
            let transport = SseTransport::new(self.clone(), addr);
            let transport = match (self.auth_provider, self.require_auth) {
                (Some(provider), true) => transport.with_auth(provider),
                (Some(provider), false) => transport.with_optional_auth(provider),
                (None, _) => transport,
            };
            transport.serve()
        }
        #[cfg(not(feature = "auth"))]
        SseTransport::new(self, addr).serve()
    }
}

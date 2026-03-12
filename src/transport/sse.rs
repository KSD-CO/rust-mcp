/// SSE (Server-Sent Events) + HTTP POST transport for MCP.
///
///   GET  /sse          → opens SSE stream, client receives messages from server
///   POST /message      → client sends JSON-RPC messages to server
use std::{convert::Infallible, sync::Arc};

use axum::{
    extract::{Query, State as AxumState},
    http::StatusCode,
    response::{sse::Event, IntoResponse, Response, Sse},
    routing::{get, post},
    Json as AxumJson,
    Router as AxumRouter,
};
use dashmap::DashMap;
use futures_util::stream;
use crate::{error::McpResult, protocol::JsonRpcMessage};
use crate::server::{server::McpServer, session::Session};
use tokio::sync::mpsc;
use tracing::{error, info};
use uuid::Uuid;

// ─── Shared SSE state ─────────────────────────────────────────────────────────

type SessionTx = mpsc::Sender<JsonRpcMessage>;

#[derive(Clone)]
pub struct SseState {
    pub server: Arc<McpServer>,
    pub sessions: Arc<DashMap<String, (SessionTx, Arc<tokio::sync::Mutex<Session>>)>>,
}

// ─── SseTransport ─────────────────────────────────────────────────────────────

pub struct SseTransport {
    server: McpServer,
    addr: std::net::SocketAddr,
}

impl SseTransport {
    pub fn new(server: McpServer, addr: impl Into<std::net::SocketAddr>) -> Self {
        Self { server, addr: addr.into() }
    }

    pub async fn serve(self) -> McpResult<()> {
        let state = SseState {
            server: Arc::new(self.server),
            sessions: Arc::new(DashMap::new()),
        };

        let app = AxumRouter::new()
            .route("/sse", get(sse_handler))
            .route("/message", post(message_handler))
            .with_state(state);

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

// ─── GET /sse ─────────────────────────────────────────────────────────────────

async fn sse_handler(AxumState(state): AxumState<SseState>) -> Response {
    let session_id = Uuid::new_v4().to_string();
    let (tx, rx) = mpsc::channel::<JsonRpcMessage>(64);
    let session = Arc::new(tokio::sync::Mutex::new(Session::new()));

    state.sessions.insert(session_id.clone(), (tx, session.clone()));
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
    AxumJson(msg): AxumJson<JsonRpcMessage>,
) -> impl IntoResponse {
    let entry = state.sessions.get(&query.session_id);
    let Some(entry) = entry else {
        return (StatusCode::NOT_FOUND, "Session not found").into_response();
    };

    let (tx, session_arc) = entry.value().clone();
    drop(entry);

    let mut session = session_arc.lock().await;
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
    async fn serve_sse(self, addr: impl Into<std::net::SocketAddr>) -> McpResult<()>;
}

impl ServeSseExt for McpServer {
    async fn serve_sse(self, addr: impl Into<std::net::SocketAddr>) -> McpResult<()> {
        SseTransport::new(self, addr).serve().await
    }
}

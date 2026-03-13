//! Streamable HTTP transport - connects to MCP server via Streamable HTTP (2025-03-26 spec).
//!
//! This transport uses a single HTTP endpoint that can return either:
//! - JSON response for simple requests
//! - SSE stream for long-running operations

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::StreamExt;
use mcp_kit::protocol::{JsonRpcMessage, JsonRpcNotification, JsonRpcResponse, RequestId};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tracing::{debug, error, trace, warn};

use super::Transport;
use crate::error::{ClientError, ClientResult};
use crate::server_request::ServerRequestHandler;

const MCP_SESSION_ID_HEADER: &str = "Mcp-Session-Id";

/// Streamable HTTP transport for MCP communication.
///
/// Uses a single endpoint that can return JSON or SSE responses.
pub struct StreamableHttpTransport {
    /// Base URL of the MCP server
    base_url: String,
    /// MCP endpoint path (default: /mcp)
    endpoint: String,
    /// HTTP client for requests
    http_client: Client,
    /// Session ID from server
    session_id: Arc<RwLock<Option<String>>>,
    /// Receiver for notifications from the server
    notification_rx: Mutex<mpsc::Receiver<JsonRpcNotification>>,
    /// Sender for notifications (to pass to SSE listener)
    notification_tx: mpsc::Sender<JsonRpcNotification>,
    /// Pending requests waiting for responses
    pending: Arc<Mutex<HashMap<RequestId, oneshot::Sender<JsonRpcResponse>>>>,
    /// Connection state
    connected: Arc<AtomicBool>,
    /// Whether SSE stream is active for server notifications
    sse_active: Arc<AtomicBool>,
    /// Handler for server-initiated requests (sampling, elicitation)
    server_request_handler: Option<ServerRequestHandler>,
}

impl StreamableHttpTransport {
    /// Connect to a Streamable HTTP MCP server.
    ///
    /// # Arguments
    /// * `base_url` - Base URL of the MCP server (e.g., "http://localhost:3000")
    pub async fn connect(base_url: &str) -> ClientResult<Self> {
        Self::connect_with_endpoint(base_url, "/mcp").await
    }

    /// Connect to a Streamable HTTP MCP server with custom endpoint.
    ///
    /// # Arguments
    /// * `base_url` - Base URL of the MCP server
    /// * `endpoint` - MCP endpoint path (e.g., "/mcp")
    pub async fn connect_with_endpoint(base_url: &str, endpoint: &str) -> ClientResult<Self> {
        Self::connect_with_options(base_url, endpoint, None).await
    }

    /// Connect to a Streamable HTTP MCP server with full options.
    ///
    /// # Arguments
    /// * `base_url` - Base URL of the MCP server
    /// * `endpoint` - MCP endpoint path (e.g., "/mcp")
    /// * `server_request_handler` - Optional handler for server-initiated requests
    pub async fn connect_with_options(
        base_url: &str,
        endpoint: &str,
        server_request_handler: Option<ServerRequestHandler>,
    ) -> ClientResult<Self> {
        let http_client = Client::new();
        let base_url = base_url.trim_end_matches('/').to_string();
        let endpoint = endpoint.to_string();

        let (notification_tx, notification_rx) = mpsc::channel::<JsonRpcNotification>(32);
        let pending: Arc<Mutex<HashMap<RequestId, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let connected = Arc::new(AtomicBool::new(true));
        let session_id: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));

        Ok(Self {
            base_url,
            endpoint,
            http_client,
            session_id,
            notification_rx: Mutex::new(notification_rx),
            notification_tx,
            pending,
            connected,
            sse_active: Arc::new(AtomicBool::new(false)),
            server_request_handler,
        })
    }

    /// Get the full endpoint URL.
    fn endpoint_url(&self) -> String {
        format!("{}{}", self.base_url, self.endpoint)
    }

    /// Build headers for requests.
    async fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        if let Some(session_id) = self.session_id.read().await.as_ref() {
            if let Ok(value) = HeaderValue::from_str(session_id) {
                headers.insert(MCP_SESSION_ID_HEADER, value);
            }
        }

        headers
    }

    /// Start SSE stream for server-initiated notifications.
    ///
    /// Call this after initialization to receive notifications from the server.
    pub async fn start_notification_stream(&self) -> ClientResult<()> {
        if self.sse_active.load(Ordering::SeqCst) {
            return Ok(()); // Already active
        }

        let session_id = self.session_id.read().await.clone();
        let Some(session_id) = session_id else {
            return Err(ClientError::Transport(
                "No session ID - initialize first".into(),
            ));
        };

        let url = self.endpoint_url();
        let mut headers = HeaderMap::new();
        if let Ok(value) = HeaderValue::from_str(&session_id) {
            headers.insert(MCP_SESSION_ID_HEADER, value);
        }

        let client = self.http_client.clone();
        let request = client.get(&url).headers(headers);
        let mut es =
            EventSource::new(request).map_err(|e| ClientError::Transport(e.to_string()))?;

        let notification_tx = self.notification_tx.clone();
        let pending = self.pending.clone();
        let connected = self.connected.clone();
        let sse_active = self.sse_active.clone();
        let http_client_clone = self.http_client.clone();
        let endpoint_url = self.endpoint_url();
        let handler = self.server_request_handler.clone();
        let session_id_for_handler = session_id.clone();

        sse_active.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            while let Some(event) = es.next().await {
                match event {
                    Ok(Event::Open) => {
                        debug!("Streamable HTTP SSE stream opened");
                    }
                    Ok(Event::Message(msg)) => {
                        trace!("SSE event: {} - {}", msg.event, msg.data);

                        match serde_json::from_str::<JsonRpcMessage>(&msg.data) {
                            Ok(JsonRpcMessage::Response(resp)) => {
                                let mut pending = pending.lock().await;
                                if let Some(tx) = pending.remove(&resp.id) {
                                    let _ = tx.send(resp);
                                } else {
                                    warn!("Received response for unknown request: {:?}", resp.id);
                                }
                            }
                            Ok(JsonRpcMessage::Error(err)) => {
                                let mut pending = pending.lock().await;
                                if let Some(tx) = pending.remove(&err.id) {
                                    let resp = JsonRpcResponse {
                                        jsonrpc: err.jsonrpc,
                                        id: err.id,
                                        result: serde_json::json!({
                                            "error": {
                                                "code": err.error.code,
                                                "message": err.error.message
                                            }
                                        }),
                                    };
                                    let _ = tx.send(resp);
                                }
                            }
                            Ok(JsonRpcMessage::Notification(notif)) => {
                                if notification_tx.send(notif).await.is_err() {
                                    break;
                                }
                            }
                            Ok(JsonRpcMessage::Request(req)) => {
                                debug!("Received server request: {}", req.method);
                                if let Some(ref h) = handler {
                                    if let Some(response) = h.handle(req).await {
                                        // Send response via HTTP POST
                                        let mut headers = HeaderMap::new();
                                        headers.insert(
                                            CONTENT_TYPE,
                                            HeaderValue::from_static("application/json"),
                                        );
                                        if let Ok(value) =
                                            HeaderValue::from_str(&session_id_for_handler)
                                        {
                                            headers.insert(MCP_SESSION_ID_HEADER, value);
                                        }

                                        if let Ok(json) = serde_json::to_string(
                                            &JsonRpcMessage::Response(response),
                                        ) {
                                            let _ = http_client_clone
                                                .post(&endpoint_url)
                                                .headers(headers)
                                                .body(json)
                                                .send()
                                                .await;
                                        }
                                    }
                                } else {
                                    warn!("No handler for server request: {}", req.method);
                                }
                            }
                            Err(e) => {
                                trace!("Non-JSON-RPC event: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("SSE stream error: {}", e);
                        break;
                    }
                }
            }
            connected.store(false, Ordering::SeqCst);
            sse_active.store(false, Ordering::SeqCst);
            debug!("Streamable HTTP SSE stream closed");
        });

        Ok(())
    }

    /// Terminate the session.
    pub async fn terminate_session(&self) -> ClientResult<()> {
        let headers = self.build_headers().await;

        self.http_client
            .delete(self.endpoint_url())
            .headers(headers)
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        *self.session_id.write().await = None;
        self.connected.store(false, Ordering::SeqCst);

        Ok(())
    }
}

#[async_trait]
impl Transport for StreamableHttpTransport {
    async fn request(&self, message: JsonRpcMessage) -> ClientResult<JsonRpcMessage> {
        if !self.connected.load(Ordering::SeqCst) {
            return Err(ClientError::Closed);
        }

        let request_id = match &message {
            JsonRpcMessage::Request(req) => Some(req.id.clone()),
            _ => None,
        };

        let headers = self.build_headers().await;
        let body = serde_json::to_string(&message).map_err(ClientError::Serialization)?;

        let response = self
            .http_client
            .post(self.endpoint_url())
            .headers(headers)
            .body(body)
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        // Extract session ID from response headers
        if let Some(session_id) = response.headers().get(MCP_SESSION_ID_HEADER) {
            if let Ok(sid) = session_id.to_str() {
                *self.session_id.write().await = Some(sid.to_string());
            }
        }

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if content_type.contains("text/event-stream") {
            // SSE streaming response
            let (_tx, _rx) = oneshot::channel();
            if let Some(ref id) = request_id {
                self.pending.lock().await.insert(id.clone(), _tx);
            }

            // Parse SSE stream for the response
            let text = response
                .text()
                .await
                .map_err(|e| ClientError::Transport(e.to_string()))?;

            // Parse last SSE data line as the response
            for line in text.lines().rev() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if let Ok(msg) = serde_json::from_str::<JsonRpcMessage>(data) {
                        return Ok(msg);
                    }
                }
            }

            // Wait for response via pending if not found in body
            if let Some(ref id) = request_id {
                if let Some(tx) = self.pending.lock().await.remove(id) {
                    drop(tx);
                }
                return Err(ClientError::Transport("No response in SSE stream".into()));
            }

            Err(ClientError::Transport("Empty SSE response".into()))
        } else {
            // JSON response
            let text = response
                .text()
                .await
                .map_err(|e| ClientError::Transport(e.to_string()))?;

            if text.is_empty() {
                // 202 Accepted - no response (for notifications)
                return Ok(JsonRpcMessage::Notification(JsonRpcNotification {
                    jsonrpc: "2.0".into(),
                    method: "_accepted".into(),
                    params: None,
                }));
            }

            serde_json::from_str(&text).map_err(ClientError::Serialization)
        }
    }

    async fn notify(&self, notification: JsonRpcNotification) -> ClientResult<()> {
        if !self.connected.load(Ordering::SeqCst) {
            return Err(ClientError::Closed);
        }

        let message = JsonRpcMessage::Notification(notification);
        let headers = self.build_headers().await;
        let body = serde_json::to_string(&message).map_err(ClientError::Serialization)?;

        let response = self
            .http_client
            .post(self.endpoint_url())
            .headers(headers)
            .body(body)
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        // Update session ID if provided
        if let Some(session_id) = response.headers().get(MCP_SESSION_ID_HEADER) {
            if let Ok(sid) = session_id.to_str() {
                *self.session_id.write().await = Some(sid.to_string());
            }
        }

        Ok(())
    }

    async fn recv_notification(&self) -> ClientResult<JsonRpcNotification> {
        let mut rx = self.notification_rx.lock().await;
        rx.recv()
            .await
            .ok_or(ClientError::Transport("Notification channel closed".into()))
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    async fn close(&self) -> ClientResult<()> {
        self.terminate_session().await
    }
}

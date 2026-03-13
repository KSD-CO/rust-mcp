//! SSE transport - connects to MCP server via HTTP Server-Sent Events.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::StreamExt;
use mcp_kit::protocol::{JsonRpcMessage, JsonRpcNotification, JsonRpcResponse, RequestId};
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::{debug, error, trace, warn};

use super::Transport;
use crate::error::{ClientError, ClientResult};
use crate::server_request::ServerRequestHandler;

/// SSE transport for HTTP Server-Sent Events communication.
pub struct SseTransport {
    /// Base URL of the MCP server
    base_url: String,
    /// HTTP client for POST requests
    http_client: Client,
    /// Receiver for notifications from the server
    notification_rx: Mutex<mpsc::Receiver<JsonRpcNotification>>,
    /// Pending requests waiting for responses
    pending: Arc<Mutex<HashMap<RequestId, oneshot::Sender<JsonRpcResponse>>>>,
    /// Connection state
    connected: Arc<AtomicBool>,
    /// Session endpoint for sending messages
    session_endpoint: Arc<Mutex<Option<String>>>,
    /// Server request handler
    #[allow(dead_code)]
    server_request_handler: Option<ServerRequestHandler>,
}

impl SseTransport {
    /// Connect to an SSE MCP server.
    ///
    /// # Arguments
    /// * `base_url` - Base URL of the MCP server (e.g., "http://localhost:3000")
    pub async fn connect(base_url: &str) -> ClientResult<Self> {
        Self::connect_with_handler(base_url, None).await
    }

    /// Connect to an SSE MCP server with a server request handler.
    pub async fn connect_with_handler(
        base_url: &str,
        server_request_handler: Option<ServerRequestHandler>,
    ) -> ClientResult<Self> {
        let http_client = Client::new();
        let base_url = base_url.trim_end_matches('/').to_string();

        let (notification_tx, notification_rx) = mpsc::channel::<JsonRpcNotification>(32);
        let pending: Arc<Mutex<HashMap<RequestId, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let connected = Arc::new(AtomicBool::new(true));
        let session_endpoint: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        // Start SSE event stream
        let sse_url = format!("{}/sse", base_url);
        let mut es = EventSource::get(&sse_url);

        let pending_clone = pending.clone();
        let connected_clone = connected.clone();
        let session_endpoint_clone = session_endpoint.clone();
        let http_client_clone = http_client.clone();
        let handler_clone = server_request_handler.clone();

        tokio::spawn(async move {
            while let Some(event) = es.next().await {
                match event {
                    Ok(Event::Open) => {
                        debug!("SSE connection opened");
                    }
                    Ok(Event::Message(msg)) => {
                        trace!("SSE event: {} - {}", msg.event, msg.data);

                        // Check for endpoint event (session URL)
                        if msg.event == "endpoint" {
                            let mut endpoint = session_endpoint_clone.lock().await;
                            *endpoint = Some(msg.data.clone());
                            debug!("SSE session endpoint: {}", msg.data);
                            continue;
                        }

                        // Parse JSON-RPC message
                        match serde_json::from_str::<JsonRpcMessage>(&msg.data) {
                            Ok(JsonRpcMessage::Response(resp)) => {
                                let mut pending = pending_clone.lock().await;
                                if let Some(tx) = pending.remove(&resp.id) {
                                    let _ = tx.send(resp);
                                } else {
                                    warn!("Received response for unknown request: {:?}", resp.id);
                                }
                            }
                            Ok(JsonRpcMessage::Error(err)) => {
                                let mut pending = pending_clone.lock().await;
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
                                if let Some(ref handler) = handler_clone {
                                    if let Some(response) = handler.handle(req).await {
                                        // Send response via HTTP POST to session endpoint
                                        let endpoint = session_endpoint_clone.lock().await;
                                        if let Some(ref url) = *endpoint {
                                            if let Ok(json) =
                                                serde_json::to_string(&JsonRpcMessage::Response(response))
                                            {
                                                let _ = http_client_clone
                                                    .post(url)
                                                    .header("Content-Type", "application/json")
                                                    .body(json)
                                                    .send()
                                                    .await;
                                            }
                                        }
                                    }
                                } else {
                                    warn!("No handler for server request: {}", req.method);
                                }
                            }
                            Err(e) => {
                                // Not all SSE events are JSON-RPC messages
                                trace!("Non-JSON-RPC event: {} - {}", msg.event, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("SSE error: {}", e);
                        break;
                    }
                }
            }
            connected_clone.store(false, Ordering::SeqCst);
        });

        // Wait for the endpoint to be received
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Ok(Self {
            base_url,
            http_client,
            notification_rx: Mutex::new(notification_rx),
            pending,
            connected,
            session_endpoint,
            server_request_handler,
        })
    }

    /// Get the message endpoint URL.
    async fn get_endpoint(&self) -> ClientResult<String> {
        let endpoint = self.session_endpoint.lock().await;
        if let Some(ep) = endpoint.as_ref() {
            // If it's a relative URL, combine with base
            if ep.starts_with('/') {
                Ok(format!("{}{}", self.base_url, ep))
            } else {
                Ok(ep.clone())
            }
        } else {
            // Fallback to /message endpoint
            Ok(format!("{}/message", self.base_url))
        }
    }
}

#[async_trait]
impl Transport for SseTransport {
    async fn request(&self, message: JsonRpcMessage) -> ClientResult<JsonRpcMessage> {
        let request = match &message {
            JsonRpcMessage::Request(req) => req,
            _ => return Err(ClientError::Protocol("Expected request message".into())),
        };

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(request.id.clone(), tx);
        }

        let endpoint = self.get_endpoint().await?;
        let json = serde_json::to_string(&message)?;

        trace!("SSE POST to {}: {}", endpoint, json);

        let resp = self
            .http_client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await
            .map_err(|e| ClientError::Transport(format!("HTTP request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(ClientError::Transport(format!(
                "HTTP error: {}",
                resp.status()
            )));
        }

        // Wait for response via SSE
        let response = rx.await.map_err(|_| ClientError::Closed)?;
        Ok(JsonRpcMessage::Response(response))
    }

    async fn notify(&self, notification: JsonRpcNotification) -> ClientResult<()> {
        let endpoint = self.get_endpoint().await?;
        let json = serde_json::to_string(&JsonRpcMessage::Notification(notification))?;

        self.http_client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await
            .map_err(|e| ClientError::Transport(format!("HTTP request failed: {}", e)))?;

        Ok(())
    }

    async fn recv_notification(&self) -> ClientResult<JsonRpcNotification> {
        let mut rx = self.notification_rx.lock().await;
        rx.recv().await.ok_or(ClientError::Closed)
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    async fn close(&self) -> ClientResult<()> {
        self.connected.store(false, Ordering::SeqCst);
        Ok(())
    }
}

//! WebSocket transport - connects to MCP server via WebSocket.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use mcp_kit::protocol::{JsonRpcMessage, JsonRpcNotification, JsonRpcResponse, RequestId};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, trace, warn};

use super::Transport;
use crate::error::{ClientError, ClientResult};
use crate::server_request::ServerRequestHandler;

/// WebSocket transport for MCP communication.
pub struct WebSocketTransport {
    /// Sender to write messages to the WebSocket
    writer_tx: mpsc::Sender<String>,
    /// Receiver for notifications from the server
    notification_rx: Mutex<mpsc::Receiver<JsonRpcNotification>>,
    /// Pending requests waiting for responses
    pending: Arc<Mutex<HashMap<RequestId, oneshot::Sender<JsonRpcResponse>>>>,
    /// Connection state
    connected: Arc<AtomicBool>,
    /// Server request handler
    #[allow(dead_code)]
    server_request_handler: Option<ServerRequestHandler>,
}

impl WebSocketTransport {
    /// Connect to a WebSocket MCP server.
    pub async fn connect(url: &str) -> ClientResult<Self> {
        Self::connect_with_handler(url, None).await
    }

    /// Connect to a WebSocket MCP server with a server request handler.
    pub async fn connect_with_handler(
        url: &str,
        server_request_handler: Option<ServerRequestHandler>,
    ) -> ClientResult<Self> {
        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| ClientError::Transport(format!("WebSocket connection failed: {}", e)))?;

        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        let (writer_tx, mut writer_rx) = mpsc::channel::<String>(32);
        let (notification_tx, notification_rx) = mpsc::channel::<JsonRpcNotification>(32);
        let pending: Arc<Mutex<HashMap<RequestId, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let connected = Arc::new(AtomicBool::new(true));

        // Writer task
        let connected_clone = connected.clone();
        tokio::spawn(async move {
            while let Some(msg) = writer_rx.recv().await {
                trace!("WS Sending: {}", msg);
                if ws_tx.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
            let _ = ws_tx.close().await;
            connected_clone.store(false, Ordering::SeqCst);
        });

        // Reader task
        let pending_clone = pending.clone();
        let connected_clone = connected.clone();
        let writer_tx_clone = writer_tx.clone();
        let handler_clone = server_request_handler.clone();
        tokio::spawn(async move {
            while let Some(msg) = ws_rx.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        trace!("WS Received: {}", text);
                        match serde_json::from_str::<JsonRpcMessage>(&text) {
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
                                        if let Ok(json) = serde_json::to_string(&JsonRpcMessage::Response(response)) {
                                            let _ = writer_tx_clone.send(json).await;
                                        }
                                    }
                                } else {
                                    warn!("No handler for server request: {}", req.method);
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse message: {} - {}", e, text);
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        debug!("WebSocket closed by server");
                        break;
                    }
                    Ok(Message::Ping(_)) => {
                        trace!("WS Ping received");
                        // Pong is handled automatically by tungstenite
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
            connected_clone.store(false, Ordering::SeqCst);
        });

        Ok(Self {
            writer_tx,
            notification_rx: Mutex::new(notification_rx),
            pending,
            connected,
            server_request_handler,
        })
    }
}

#[async_trait]
impl Transport for WebSocketTransport {
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

        let json = serde_json::to_string(&message)?;
        self.writer_tx
            .send(json)
            .await
            .map_err(|_| ClientError::Closed)?;

        let response = rx.await.map_err(|_| ClientError::Closed)?;
        Ok(JsonRpcMessage::Response(response))
    }

    async fn notify(&self, notification: JsonRpcNotification) -> ClientResult<()> {
        let json = serde_json::to_string(&JsonRpcMessage::Notification(notification))?;
        self.writer_tx
            .send(json)
            .await
            .map_err(|_| ClientError::Closed)?;
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
        // Drop the writer channel to close the connection
        Ok(())
    }
}

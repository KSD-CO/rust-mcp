//! Stdio transport - connects to MCP server via subprocess stdin/stdout.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use mcp_kit::protocol::{JsonRpcMessage, JsonRpcNotification, JsonRpcResponse, RequestId};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::{debug, error, trace, warn};

use super::Transport;
use crate::error::{ClientError, ClientResult};
use crate::server_request::ServerRequestHandler;

/// Stdio transport for subprocess communication.
pub struct StdioTransport {
    /// Sender to write messages to the subprocess
    writer_tx: mpsc::Sender<String>,
    /// Receiver for notifications from the subprocess
    notification_rx: Mutex<mpsc::Receiver<JsonRpcNotification>>,
    /// Pending requests waiting for responses
    pending: Arc<Mutex<HashMap<RequestId, oneshot::Sender<JsonRpcResponse>>>>,
    /// Connection state
    connected: Arc<AtomicBool>,
    /// Child process handle
    _child: Arc<Mutex<Child>>,
    /// Server request handler (used via clone in spawn, false positive warning)
    #[allow(dead_code)]
    server_request_handler: Option<ServerRequestHandler>,
}

impl StdioTransport {
    /// Spawn a subprocess and connect to it via stdio.
    pub async fn spawn(
        program: impl AsRef<Path>,
        args: &[&str],
        env: &[(&str, &str)],
    ) -> ClientResult<Self> {
        Self::spawn_with_handler(program, args, env, None).await
    }

    /// Spawn a subprocess with a server request handler.
    pub async fn spawn_with_handler(
        program: impl AsRef<Path>,
        args: &[&str],
        env: &[(&str, &str)],
        server_request_handler: Option<ServerRequestHandler>,
    ) -> ClientResult<Self> {
        let mut cmd = Command::new(program.as_ref());
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        for (key, value) in env {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| ClientError::Transport("Failed to open stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ClientError::Transport("Failed to open stdout".into()))?;

        let (writer_tx, mut writer_rx) = mpsc::channel::<String>(32);
        let (notification_tx, notification_rx) = mpsc::channel::<JsonRpcNotification>(32);
        let pending: Arc<Mutex<HashMap<RequestId, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let connected = Arc::new(AtomicBool::new(true));

        // Writer task
        let connected_clone = connected.clone();
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(msg) = writer_rx.recv().await {
                trace!("Sending: {}", msg);
                if stdin.write_all(msg.as_bytes()).await.is_err() {
                    break;
                }
                if stdin.write_all(b"\n").await.is_err() {
                    break;
                }
                if stdin.flush().await.is_err() {
                    break;
                }
            }
            connected_clone.store(false, Ordering::SeqCst);
        });

        // Reader task
        let pending_clone = pending.clone();
        let connected_clone = connected.clone();
        let writer_tx_clone = writer_tx.clone();
        let handler_clone = server_request_handler.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        debug!("Subprocess stdout closed");
                        break;
                    }
                    Ok(_) => {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        trace!("Received: {}", line);

                        match serde_json::from_str::<JsonRpcMessage>(line) {
                            Ok(JsonRpcMessage::Response(resp)) => {
                                let mut pending = pending_clone.lock().await;
                                if let Some(tx) = pending.remove(&resp.id) {
                                    let _ = tx.send(resp);
                                } else {
                                    warn!("Received response for unknown request: {:?}", resp.id);
                                }
                            }
                            Ok(JsonRpcMessage::Error(err)) => {
                                // Handle error response - find pending request and send error
                                let mut pending = pending_clone.lock().await;
                                if let Some(tx) = pending.remove(&err.id) {
                                    // Convert error to a response with error result
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
                                // Server-initiated request (e.g., sampling, elicitation)
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
                                warn!("Failed to parse message: {} - {}", e, line);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error reading from subprocess: {}", e);
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
            _child: Arc::new(Mutex::new(child)),
            server_request_handler,
        })
    }
}

#[async_trait]
impl Transport for StdioTransport {
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
        let mut child = self._child.lock().await;
        let _ = child.kill().await;
        Ok(())
    }
}

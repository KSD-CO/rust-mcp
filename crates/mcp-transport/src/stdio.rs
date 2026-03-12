/// stdio transport — reads from stdin, writes to stdout (newline-delimited JSON).
///
/// This is the most common transport for MCP servers launched as subprocesses by clients.
use futures_util::{SinkExt, StreamExt};
use mcp_core::{error::McpResult, protocol::JsonRpcMessage};
use mcp_server::{server::McpServer, session::Session};
use tokio::io::{stdin, stdout};
use tokio_util::codec::{FramedRead, FramedWrite};
use tracing::{debug, error};

use crate::codec::NdJsonCodec;

/// Serves `server` over stdin/stdout until the stream closes.
pub struct StdioTransport {
    server: McpServer,
}

impl StdioTransport {
    pub fn new(server: McpServer) -> Self {
        Self { server }
    }

    /// Start the stdio event loop.
    pub async fn serve(self) -> McpResult<()> {
        let stdin = stdin();
        let stdout = stdout();

        let mut reader = FramedRead::new(stdin, NdJsonCodec);
        let mut writer = FramedWrite::new(stdout, NdJsonCodec);

        let mut session = Session::new();

        tracing::info!(server = %self.server.info().name, "stdio transport started");

        while let Some(msg) = reader.next().await {
            match msg {
                Ok(msg) => {
                    debug!(?msg, "Received message");
                    if let Some(response) = self.server.handle_message(msg, &mut session).await {
                        debug!(?response, "Sending response");
                        if let Err(e) = writer.send(response).await {
                            error!(error = %e, "Failed to write response");
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!(error = %e, "Failed to decode message");
                }
            }
        }

        tracing::info!("stdio transport closed");
        Ok(())
    }
}

/// Extension trait that adds `.serve_stdio()` to `McpServer`.
pub trait ServeStdioExt {
    async fn serve_stdio(self) -> McpResult<()>;
}

impl ServeStdioExt for McpServer {
    async fn serve_stdio(self) -> McpResult<()> {
        StdioTransport::new(self).serve().await
    }
}

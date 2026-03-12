use crate::error::McpResult;
use crate::server::{core::McpServer, session::Session};
/// stdio transport — reads from stdin, writes to stdout (newline-delimited JSON).
use futures_util::{SinkExt, StreamExt};
use std::future::Future;
use tokio::io::{stdin, stdout};
use tokio_util::codec::{FramedRead, FramedWrite};
use tracing::{debug, error};

use super::codec::NdJsonCodec;

/// Serves `server` over stdin/stdout until the stream closes.
pub struct StdioTransport {
    server: McpServer,
}

impl StdioTransport {
    pub fn new(server: McpServer) -> Self {
        Self { server }
    }

    pub async fn serve(self) -> McpResult<()> {
        let mut reader = FramedRead::new(stdin(), NdJsonCodec);
        let mut writer = FramedWrite::new(stdout(), NdJsonCodec);
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
    fn serve_stdio(self) -> impl Future<Output = McpResult<()>> + Send;
}

impl ServeStdioExt for McpServer {
    fn serve_stdio(self) -> impl Future<Output = McpResult<()>> + Send {
        StdioTransport::new(self).serve()
    }
}

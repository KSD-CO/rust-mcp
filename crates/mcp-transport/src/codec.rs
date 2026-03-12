/// Newline-delimited JSON codec for use with tokio-util's `Framed`.
use bytes::{Buf, BufMut, BytesMut};
use mcp_core::{error::McpError, protocol::JsonRpcMessage};
use tokio_util::codec::{Decoder, Encoder};

pub struct NdJsonCodec;

impl Encoder<JsonRpcMessage> for NdJsonCodec {
    type Error = McpError;

    fn encode(&mut self, item: JsonRpcMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let json = serde_json::to_string(&item)?;
        dst.reserve(json.len() + 1);
        dst.put_slice(json.as_bytes());
        dst.put_u8(b'\n');
        Ok(())
    }
}

impl Decoder for NdJsonCodec {
    type Item = JsonRpcMessage;
    type Error = McpError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if let Some(newline) = src.iter().position(|&b| b == b'\n') {
            let line = src.split_to(newline + 1);
            let trimmed = std::str::from_utf8(&line[..line.len() - 1])
                .map_err(|e| McpError::ParseError(e.to_string()))?
                .trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            let msg: JsonRpcMessage = serde_json::from_str(trimmed)
                .map_err(|e| McpError::ParseError(e.to_string()))?;
            Ok(Some(msg))
        } else {
            Ok(None)
        }
    }
}

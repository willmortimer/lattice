//! Minimal Connect JSON codec helpers (unary + server-stream).

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{CellClientError, Result};

pub const CONNECT_PROTOCOL_VERSION: &str = "1";

pub const CELL_APPLY: &str = "/cell.v1.CellService/ApplyCell";
pub const CELL_START: &str = "/cell.v1.CellService/StartCell";
pub const GUEST_INVOKE: &str = "/cell.v1.GuestSessionService/Invoke";

/// Unary Connect JSON POST body + expected response decode.
pub fn encode_unary_json<T: Serialize>(message: &T) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec(message)?)
}

pub fn decode_unary_json<T: DeserializeOwned>(body: &[u8]) -> Result<T> {
    Ok(serde_json::from_slice(body)?)
}

/// Decode a Connect server-stream (`application/connect+json`) body into messages.
///
/// Envelope: 1 flag byte + 4-byte big-endian length + payload.
/// Flag bit1 (0x02) marks end-stream (error or trailer JSON).
pub fn decode_connect_stream(body: &[u8]) -> Result<Vec<serde_json::Value>> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while offset < body.len() {
        if body.len() - offset < 5 {
            return Err(CellClientError::Connect(format!(
                "truncated envelope truncated at offset {offset}"
            )));
        }
        let flags = body[offset];
        let len = u32::from_be_bytes([
            body[offset + 1],
            body[offset + 2],
            body[offset + 3],
            body[offset + 4],
        ]) as usize;
        offset += 5;
        if body.len() - offset < len {
            return Err(CellClientError::Connect(format!(
                "truncated message truncated: need {len} bytes"
            )));
        }
        let payload = &body[offset..offset + len];
        offset += len;

        let end_stream = flags & 0x02 != 0;
        if end_stream {
            if payload.is_empty() || payload == b"{}" {
                break;
            }
            let trailer: serde_json::Value = serde_json::from_slice(payload)?;
            if let Some(err) = trailer.get("error") {
                return Err(CellClientError::Connect(format!("stream error: {err}")));
            }
            break;
        }
        if flags & 0x01 != 0 {
            return Err(CellClientError::Connect(
                "compressed connect frames are not supported".into(),
            ));
        }
        if !payload.is_empty() {
            out.push(serde_json::from_slice(payload)?);
        }
    }
    Ok(out)
}

/// Encode a single Connect stream envelope (uncompressed message).
#[cfg(test)]
pub fn encode_connect_message(message: &serde_json::Value) -> Result<Vec<u8>> {
    let payload = serde_json::to_vec(message)?;
    let mut out = Vec::with_capacity(5 + payload.len());
    out.push(0);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&payload);
    // end-stream empty trailer
    out.push(0x02);
    out.extend_from_slice(&0u32.to_be_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn roundtrip_stream_envelope() {
        let encoded = encode_connect_message(&json!({"done": true, "payload": "e30="})).unwrap();
        let msgs = decode_connect_stream(&encoded).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["done"], true);
    }
}

//! Unix-domain socket [`LatticeClient`] for daemon mode.
//!
//! Connection flow:
//! 1. Connect to the socket path.
//! 2. Exchange a length-delimited handshake (auth token + protocol version).
//! 3. Send/receive framed [`lattice_protocol::Envelope`] messages.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use bytes::BytesMut;
use lattice_protocol::{
    encode_frame, envelope, request_envelope, FrameDecoder, Request, Response, PROTOCOL_VERSION,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::Mutex;

use crate::client::LatticeClient;
use crate::error::ClientError;
use crate::events::{EventFilter, EventStream};
use crate::handshake::{
    decode_handshake_frame, encode_handshake_frame, HandshakeRequest, HandshakeResponse,
};

/// Client connected to a private Unix-domain daemon socket.
pub struct DaemonClient {
    socket_path: PathBuf,
    instance_id: String,
    connection: Mutex<DaemonConnection>,
    next_request_id: AtomicU64,
}

struct DaemonConnection {
    stream: UnixStream,
    read_buf: BytesMut,
    decoder: FrameDecoder,
}

impl DaemonClient {
    /// Connect to `socket_path`, authenticate with `auth_token`, and verify protocol version.
    pub async fn connect(
        socket_path: impl AsRef<Path>,
        auth_token: impl Into<String>,
    ) -> Result<Self, ClientError> {
        let socket_path = socket_path.as_ref().to_path_buf();
        let mut stream = UnixStream::connect(&socket_path).await?;
        let instance_id = perform_handshake(&mut stream, auth_token.into()).await?;
        Ok(Self {
            socket_path,
            instance_id,
            connection: Mutex::new(DaemonConnection {
                stream,
                read_buf: BytesMut::new(),
                decoder: FrameDecoder::new(),
            }),
            next_request_id: AtomicU64::new(1),
        })
    }

    /// Socket path used for this connection.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Daemon instance id returned by the handshake.
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn alloc_request_id(&self) -> String {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        format!("req-{id}")
    }

    async fn write_frame(stream: &mut UnixStream, frame: &[u8]) -> Result<(), ClientError> {
        stream.write_all(frame).await?;
        stream.flush().await?;
        Ok(())
    }

    async fn read_envelope(
        conn: &mut DaemonConnection,
    ) -> Result<lattice_protocol::Envelope, ClientError> {
        loop {
            if let Some(envelope) = conn.decoder.decode(&mut conn.read_buf)? {
                return Ok(envelope);
            }
            let mut tmp = [0u8; 8192];
            let n = conn.stream.read(&mut tmp).await?;
            if n == 0 {
                return Err(ClientError::Transport(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "daemon closed connection while waiting for envelope",
                )));
            }
            conn.read_buf.extend_from_slice(&tmp[..n]);
        }
    }
}

async fn perform_handshake(
    stream: &mut UnixStream,
    auth_token: String,
) -> Result<String, ClientError> {
    let request = HandshakeRequest::new(auth_token);
    let frame = encode_handshake_frame(&request)?;
    stream.write_all(&frame).await?;
    stream.flush().await?;

    let response = read_handshake_response(stream).await?;
    if response.protocol_version != PROTOCOL_VERSION {
        return Err(ClientError::ProtocolVersionMismatch {
            client_version: PROTOCOL_VERSION,
            peer_version: response.protocol_version,
        });
    }
    if !response.accepted {
        return Err(ClientError::HandshakeRejected {
            message: if response.message.is_empty() {
                "authentication failed".into()
            } else {
                response.message
            },
        });
    }
    Ok(response.instance_id)
}

async fn read_handshake_response(
    stream: &mut UnixStream,
) -> Result<HandshakeResponse, ClientError> {
    let mut buf = BytesMut::new();
    let mut tmp = [0u8; 4096];
    loop {
        match try_decode_handshake(&buf) {
            Ok(Some((response, consumed))) => {
                let _ = buf.split_to(consumed);
                if !buf.is_empty() {
                    return Err(ClientError::UnexpectedResponse(
                        "trailing bytes after handshake response".into(),
                    ));
                }
                return Ok(response);
            }
            Ok(None) => {}
            Err(err) => return Err(err),
        }

        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            return Err(ClientError::Transport(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "daemon closed connection during handshake",
            )));
        }
        buf.extend_from_slice(&tmp[..n]);
    }
}

fn try_decode_handshake(buf: &BytesMut) -> Result<Option<(HandshakeResponse, usize)>, ClientError> {
    if buf.len() < 4 {
        return Ok(None);
    }
    let declared = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if declared > lattice_protocol::MAX_FRAME_LENGTH {
        return Err(ClientError::Protocol(
            lattice_protocol::ProtocolError::FrameTooLarge {
                max_frame_length: lattice_protocol::MAX_FRAME_LENGTH,
                declared_length: declared,
            },
        ));
    }
    let frame_len = 4usize.saturating_add(declared);
    if buf.len() < frame_len {
        return Ok(None);
    }
    let response = decode_handshake_frame(&buf[..frame_len])?;
    Ok(Some((response, frame_len)))
}

#[async_trait]
impl LatticeClient for DaemonClient {
    async fn request(&self, request: Request) -> Result<Response, ClientError> {
        let request_id = self.alloc_request_id();
        let envelope = request_envelope(request_id.clone(), request);
        let framed = encode_frame(&envelope)?;

        let mut conn = self.connection.lock().await;
        Self::write_frame(&mut conn.stream, &framed).await?;

        loop {
            let reply = Self::read_envelope(&mut conn).await?;
            if reply.request_id != request_id {
                // D0 is strictly request/response on one connection; skip
                // unmatched frames (for example late events) rather than failing.
                continue;
            }
            match reply.payload {
                Some(envelope::Payload::Response(response)) => return Ok(response),
                Some(envelope::Payload::Error(error)) => {
                    return Err(ClientError::from_wire(error));
                }
                Some(envelope::Payload::Event(_)) => continue,
                Some(envelope::Payload::Request(_)) => {
                    return Err(ClientError::UnexpectedResponse(
                        "daemon sent a request payload for a client request".into(),
                    ));
                }
                None => {
                    return Err(ClientError::UnexpectedResponse(
                        "daemon envelope missing payload".into(),
                    ));
                }
            }
        }
    }

    async fn subscribe(&self, _filter: EventFilter) -> Result<EventStream, ClientError> {
        Err(ClientError::Unimplemented(
            "daemon event subscription lands with the latticed event bus",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handshake::HandshakeRequest;

    #[test]
    fn handshake_request_uses_protocol_version() {
        let req = HandshakeRequest::new("tok");
        assert_eq!(req.protocol_version, PROTOCOL_VERSION);
        assert_eq!(req.auth_token, "tok");
    }
}

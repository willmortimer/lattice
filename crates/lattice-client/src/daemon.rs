//! Unix-domain socket [`LatticeClient`] for daemon mode.
//!
//! Connection flow:
//! 1. Connect to the socket path.
//! 2. Exchange a length-delimited handshake (auth token + protocol version).
//! 3. Spawn a reader task that demultiplexes responses and push events.
//! 4. Send/receive framed [`lattice_protocol::Envelope`] messages.
//!
//! The daemon pushes sequenced events on the same connection after handshake
//! (no separate Subscribe RPC). [`DaemonClient::subscribe`] yields those events
//! from an in-process broadcast fed by the reader task.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use bytes::BytesMut;
use lattice_protocol::{
    encode_frame, envelope, request_envelope, Event, FrameDecoder, Request, Response,
    PROTOCOL_VERSION,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};

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
    writer: Mutex<OwnedWriteHalf>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Result<Response, ClientError>>>>>,
    event_tx: broadcast::Sender<Event>,
    next_request_id: AtomicU64,
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
        let (reader, writer) = stream.into_split();

        let pending: Arc<Mutex<HashMap<String, oneshot::Sender<Result<Response, ClientError>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (event_tx, _) = broadcast::channel(64);
        spawn_reader(reader, Arc::clone(&pending), event_tx.clone());

        Ok(Self {
            socket_path,
            instance_id,
            writer: Mutex::new(writer),
            pending,
            event_tx,
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
}

fn spawn_reader(
    mut reader: OwnedReadHalf,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Result<Response, ClientError>>>>>,
    event_tx: broadcast::Sender<Event>,
) {
    tokio::spawn(async move {
        let mut read_buf = BytesMut::new();
        let mut decoder = FrameDecoder::new();
        loop {
            let envelope = match read_envelope(&mut reader, &mut read_buf, &mut decoder).await {
                Ok(envelope) => envelope,
                Err(_) => {
                    let mut guard = pending.lock().await;
                    for (_, tx) in guard.drain() {
                        let _ = tx.send(Err(ClientError::Transport(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "daemon connection closed",
                        ))));
                    }
                    break;
                }
            };

            match envelope.payload {
                Some(envelope::Payload::Response(response)) => {
                    let mut guard = pending.lock().await;
                    if let Some(tx) = guard.remove(&envelope.request_id) {
                        let _ = tx.send(Ok(response));
                    }
                }
                Some(envelope::Payload::Error(error)) => {
                    let mut guard = pending.lock().await;
                    if let Some(tx) = guard.remove(&envelope.request_id) {
                        let _ = tx.send(Err(ClientError::from_wire(error)));
                    }
                }
                Some(envelope::Payload::Event(event)) => {
                    let _ = event_tx.send(event);
                }
                Some(envelope::Payload::Request(_)) | None => {}
            }
        }
    });
}

async fn read_envelope(
    reader: &mut OwnedReadHalf,
    read_buf: &mut BytesMut,
    decoder: &mut FrameDecoder,
) -> Result<lattice_protocol::Envelope, ClientError> {
    loop {
        if let Some(envelope) = decoder.decode(read_buf)? {
            return Ok(envelope);
        }
        let mut tmp = [0u8; 8192];
        let n = reader.read(&mut tmp).await?;
        if n == 0 {
            return Err(ClientError::Transport(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "daemon closed connection while waiting for envelope",
            )));
        }
        read_buf.extend_from_slice(&tmp[..n]);
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

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(request_id, tx);
        }

        {
            let mut writer = self.writer.lock().await;
            writer.write_all(&framed).await?;
            writer.flush().await?;
        }

        match rx.await {
            Ok(result) => result,
            Err(_) => Err(ClientError::Transport(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "daemon response channel closed",
            ))),
        }
    }

    async fn subscribe(&self, filter: EventFilter) -> Result<EventStream, ClientError> {
        let mut event_rx = self.event_tx.subscribe();
        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        if let Some(workspace_id) = filter.workspace_id.as_ref() {
                            if &event.workspace_id != workspace_id {
                                continue;
                            }
                        }
                        if tx.send(Ok(event)).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Gap: surface as a transport error so callers can resync.
                        let _ = tx
                            .send(Err(ClientError::UnexpectedResponse(
                                "event subscription lagged; resubscribe from last sequence".into(),
                            )))
                            .await;
                        break;
                    }
                }
            }
        });
        Ok(EventStream::new(rx))
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

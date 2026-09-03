//! Daemon-mode [`LatticeClient`] over a transport-neutral IPC stream.
//!
//! Connection flow:
//! 1. Connect to the platform endpoint (Unix UDS or Windows named pipe).
//! 2. Exchange a length-delimited handshake (auth token + protocol version).
//! 3. Spawn a reader task that demultiplexes responses and push events.
//! 4. Send/receive framed [`lattice_protocol::Envelope`] messages.
//!
//! The daemon pushes sequenced events on the same connection after handshake
//! (no separate Subscribe RPC). [`DaemonClient::subscribe`] yields those events
//! from in-process broadcasts fed by the reader task.
//!
//! Agent events are demuxed onto a dedicated bus so IndexProgress floods cannot
//! Lagged-drop mid-run `tool-output-*` chunks the UI awaits.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use bytes::BytesMut;
use lattice_protocol::{
    encode_frame, envelope, event, request_envelope, Event, FrameDecoder, Request, Response,
    PROTOCOL_VERSION,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::client::LatticeClient;
use crate::error::ClientError;
use crate::events::{event_matches_filter, EventFilter, EventStream};
use crate::handshake::{
    decode_handshake_frame, encode_handshake_frame, HandshakeRequest, HandshakeResponse,
};
use crate::transport::{self, DaemonStream};

#[cfg(unix)]
type IpcReader = tokio::net::unix::OwnedReadHalf;
#[cfg(unix)]
type IpcWriter = tokio::net::unix::OwnedWriteHalf;
#[cfg(windows)]
type IpcReader = tokio::io::ReadHalf<DaemonStream>;
#[cfg(windows)]
type IpcWriter = tokio::io::WriteHalf<DaemonStream>;

/// Client connected to a private daemon IPC endpoint.
pub struct DaemonClient {
    socket_path: PathBuf,
    instance_id: String,
    writer: Mutex<IpcWriter>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Result<Response, ClientError>>>>>,
    event_tx: broadcast::Sender<Event>,
    agent_event_tx: broadcast::Sender<Event>,
    next_request_id: AtomicU64,
    reader_task: JoinHandle<()>,
}

impl DaemonClient {
    /// Connect to `socket_path`, authenticate with `auth_token`, and verify protocol version.
    ///
    /// `socket_path` is a UDS filesystem path on Unix, or a named-pipe path
    /// (`\\.\pipe\…`) on Windows.
    pub async fn connect(
        socket_path: impl AsRef<Path>,
        auth_token: impl Into<String>,
    ) -> Result<Self, ClientError> {
        let socket_path = socket_path.as_ref().to_path_buf();
        let mut stream = transport::connect(&socket_path).await?;
        let instance_id = perform_handshake(&mut stream, auth_token.into()).await?;
        let (reader, writer) = split_daemon_stream(stream);

        let pending: Arc<Mutex<HashMap<String, oneshot::Sender<Result<Response, ClientError>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        // General bus absorbs IndexProgress / resource chatter.
        let (event_tx, _) = broadcast::channel(8192);
        // Agent bus stays quiet so tool-output chunks are not Lagged away.
        let (agent_event_tx, _) = broadcast::channel(1024);
        let reader_task = spawn_reader(
            reader,
            Arc::clone(&pending),
            event_tx.clone(),
            agent_event_tx.clone(),
        );

        Ok(Self {
            socket_path,
            instance_id,
            writer: Mutex::new(writer),
            pending,
            event_tx,
            agent_event_tx,
            next_request_id: AtomicU64::new(1),
            reader_task,
        })
    }

    /// Endpoint path used for this connection (UDS or named pipe).
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

impl Drop for DaemonClient {
    fn drop(&mut self) {
        // Abort the reader so both IPC halves can drop. On Unix, OwnedWriteHalf
        // already shuts down the socket; aborting avoids a leaked task.
        self.reader_task.abort();
    }
}

/// Split the connected stream so dropping the write half is visible to latticed.
///
/// `tokio::io::split` keeps the underlying socket alive until **both** halves
/// drop. The reader task would then leak the connection after `DaemonClient`
/// is dropped, so idle shutdown never saw "last client disconnected".
fn split_daemon_stream(stream: DaemonStream) -> (IpcReader, IpcWriter) {
    #[cfg(unix)]
    {
        stream.into_split()
    }
    #[cfg(windows)]
    {
        tokio::io::split(stream)
    }
}

fn spawn_reader(
    mut reader: IpcReader,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Result<Response, ClientError>>>>>,
    event_tx: broadcast::Sender<Event>,
    agent_event_tx: broadcast::Sender<Event>,
) -> JoinHandle<()> {
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
                    let is_agent = matches!(event.body, Some(event::Body::AgentEvent(_)));
                    if is_agent {
                        let _ = agent_event_tx.send(event.clone());
                    }
                    let _ = event_tx.send(event);
                }
                Some(envelope::Payload::Request(_)) | None => {}
            }
        }
    })
}

async fn read_envelope<R: AsyncRead + Unpin>(
    reader: &mut R,
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

async fn perform_handshake<S>(stream: &mut S, auth_token: String) -> Result<String, ClientError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
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

async fn read_handshake_response<S>(stream: &mut S) -> Result<HandshakeResponse, ClientError>
where
    S: AsyncRead + Unpin,
{
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
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(request_id.clone(), tx);
        }

        let envelope = request_envelope(request_id.clone(), request);
        let framed = encode_frame(&envelope)?;
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
        // Agent-only subscribers listen on the quiet demux bus so IndexProgress
        // cannot Lagged-drop tool-output chunks mid-run.
        let mut event_rx = if filter.agent_events_only {
            self.agent_event_tx.subscribe()
        } else {
            self.event_tx.subscribe()
        };
        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        if !event_matches_filter(&event, &filter) {
                            continue;
                        }
                        if tx.send(Ok(event)).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Quiet agent bus should rarely lag; still fail closed so
                        // the UI surfaces an error instead of an infinite TOOL wait.
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

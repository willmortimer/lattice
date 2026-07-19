//! In-test Unix-domain fake daemon for DaemonClient contract coverage.

use std::path::PathBuf;
use std::sync::Arc;

use bytes::BytesMut;
use lattice_client::{
    decode_handshake_frame, encode_handshake_frame, HandshakeRequest, HandshakeResponse,
    PROTOCOL_VERSION,
};
use lattice_protocol::{
    encode_frame, envelope, error_envelope, request, response, response_envelope, Error,
    FrameDecoder, HealthRequest, HealthResponse, PingRequest, PingResponse, Response,
};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct FakeDaemonConfig {
    pub auth_token: String,
    pub instance_id: String,
}

pub struct FakeDaemonGuard {
    _dir: TempDir,
    shutdown: Option<oneshot::Sender<()>>,
    join: Option<JoinHandle<()>>,
}

impl Drop for FakeDaemonGuard {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.join.take() {
            join.abort();
        }
    }
}

/// Bind a temporary socket and accept connections until the guard is dropped.
pub async fn spawn_fake_daemon(config: FakeDaemonConfig) -> (PathBuf, FakeDaemonGuard) {
    let dir = TempDir::new().expect("tempdir");
    let socket_path = dir.path().join("latticed.sock");
    let listener = UnixListener::bind(&socket_path).expect("bind");
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
    let config = Arc::new(config);

    let join = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                accepted = listener.accept() => {
                    match accepted {
                        Ok((stream, _)) => {
                            let config = Arc::clone(&config);
                            tokio::spawn(async move {
                                let _ = serve_connection(stream, config).await;
                            });
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    });

    (
        socket_path,
        FakeDaemonGuard {
            _dir: dir,
            shutdown: Some(shutdown_tx),
            join: Some(join),
        },
    )
}

async fn serve_connection(
    mut stream: UnixStream,
    config: Arc<FakeDaemonConfig>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let handshake = read_handshake(&mut stream).await?;
    let accepted =
        handshake.auth_token == config.auth_token && handshake.protocol_version == PROTOCOL_VERSION;
    let response = HandshakeResponse {
        accepted,
        protocol_version: PROTOCOL_VERSION,
        instance_id: config.instance_id.clone(),
        message: if accepted {
            String::new()
        } else {
            "invalid auth token or protocol version".into()
        },
    };
    let frame = encode_handshake_frame(&response)?;
    stream.write_all(&frame).await?;
    stream.flush().await?;
    if !accepted {
        return Ok(());
    }

    let mut read_buf = BytesMut::new();
    let mut decoder = FrameDecoder::new();
    loop {
        let envelope = match read_envelope(&mut stream, &mut read_buf, &mut decoder).await {
            Ok(envelope) => envelope,
            Err(err)
                if err
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::UnexpectedEof) =>
            {
                return Ok(());
            }
            Err(err) => return Err(err),
        };

        let request_id = envelope.request_id.clone();
        let reply = match envelope.payload {
            Some(envelope::Payload::Request(req)) => match req.body {
                Some(request::Body::Health(HealthRequest {})) => response_envelope(
                    request_id,
                    Response {
                        body: Some(response::Body::Health(HealthResponse {
                            status: "ok".into(),
                            protocol_version: PROTOCOL_VERSION,
                            instance_id: config.instance_id.clone(),
                        })),
                    },
                ),
                Some(request::Body::Ping(PingRequest { nonce })) => response_envelope(
                    request_id,
                    Response {
                        body: Some(response::Body::Ping(PingResponse { nonce })),
                    },
                ),
                _ => error_envelope(
                    request_id,
                    Error {
                        code: "unimplemented".into(),
                        message: "fake daemon only supports health and ping".into(),
                        details: None,
                    },
                ),
            },
            _ => error_envelope(
                request_id,
                Error {
                    code: "invalid_payload".into(),
                    message: "expected request envelope".into(),
                    details: None,
                },
            ),
        };
        let framed = encode_frame(&reply)?;
        stream.write_all(&framed).await?;
        stream.flush().await?;
    }
}

async fn read_handshake(
    stream: &mut UnixStream,
) -> Result<HandshakeRequest, Box<dyn std::error::Error + Send + Sync>> {
    let mut buf = BytesMut::new();
    let mut tmp = [0u8; 4096];
    loop {
        if buf.len() >= 4 {
            let declared = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
            let frame_len = 4usize.saturating_add(declared);
            if buf.len() >= frame_len {
                let request = decode_handshake_frame::<HandshakeRequest>(&buf[..frame_len])?;
                return Ok(request);
            }
        }
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "client closed during handshake",
            )
            .into());
        }
        buf.extend_from_slice(&tmp[..n]);
    }
}

async fn read_envelope(
    stream: &mut UnixStream,
    read_buf: &mut BytesMut,
    decoder: &mut FrameDecoder,
) -> Result<lattice_protocol::Envelope, Box<dyn std::error::Error + Send + Sync>> {
    loop {
        if let Some(envelope) = decoder.decode(read_buf)? {
            return Ok(envelope);
        }
        let mut tmp = [0u8; 8192];
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "client closed connection",
            )
            .into());
        }
        read_buf.extend_from_slice(&tmp[..n]);
    }
}

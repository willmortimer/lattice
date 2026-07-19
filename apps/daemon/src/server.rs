//! Unix-domain socket server for framed control-plane envelopes.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bytes::BytesMut;
use lattice_client::{
    decode_handshake_frame, encode_handshake_frame, HandshakeRequest, HandshakeResponse,
};
use lattice_protocol::{
    encode_frame, envelope, error_envelope, event, event_envelope, request, response,
    response_envelope, ApplyPageUpdateRequest, ApplyPageUpdateResponse, Error as WireError, Event,
    FrameDecoder, HealthRequest, HealthResponse, IndexProgress, OpenWorkspaceRequest,
    OpenWorkspaceResponse, PingRequest, PingResponse, Request, ResourceChanged, Response,
    SearchRequest, SearchResponse, WorkspaceLeaseChanged, PROTOCOL_VERSION,
};
use lattice_runtime::{
    IdempotentOutcome, LatticeRuntime, RuntimeEvent, RuntimeIndexProgress, RuntimeResourceChanged,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::OwnedReadHalf;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, oneshot, Mutex};
use tracing::{debug, info, warn};

use crate::config::DaemonConfig;
use crate::error::{Error, Result};
use crate::lease::{daemon_lease_claim, lease_to_wire, require_workspace_lease};

/// Shared daemon state for accepted connections.
#[derive(Clone)]
pub struct DaemonState {
    pub config: Arc<DaemonConfig>,
    pub runtime: Arc<LatticeRuntime>,
    event_tx: broadcast::Sender<Event>,
    next_event_seq: Arc<AtomicU64>,
}

impl DaemonState {
    pub fn new(config: DaemonConfig, runtime: Arc<LatticeRuntime>) -> Self {
        let (event_tx, _) = broadcast::channel(64);
        let state = Self {
            config: Arc::new(config),
            runtime,
            event_tx,
            next_event_seq: Arc::new(AtomicU64::new(1)),
        };
        state.spawn_event_bridge();
        state
    }

    fn next_sequence(&self) -> u64 {
        self.next_event_seq.fetch_add(1, Ordering::Relaxed)
    }

    fn publish_event(&self, workspace_id: String, body: event::Body) {
        let event = Event {
            sequence: self.next_sequence(),
            workspace_id,
            body: Some(body),
        };
        let _ = self.event_tx.send(event);
    }

    /// Bridge synchronous [`lattice_runtime::EventBus`] signals into sequenced
    /// wire events fan-out to connected clients.
    fn spawn_event_bridge(&self) {
        let runtime = Arc::clone(&self.runtime);
        let state = self.clone();
        std::thread::Builder::new()
            .name("latticed-event-bridge".into())
            .spawn(move || {
                let rx = runtime.events().subscribe();
                while let Ok(evt) = rx.recv() {
                    match evt {
                        RuntimeEvent::SessionOpened { workspace_id, .. } => {
                            debug!(%workspace_id, "runtime session opened");
                        }
                        RuntimeEvent::SessionClosed { workspace_id, .. } => {
                            debug!(%workspace_id, "runtime session closed");
                        }
                        RuntimeEvent::ResourceChanged(changed) => {
                            let workspace_id = changed.workspace_id.clone();
                            state.publish_event(
                                workspace_id,
                                event::Body::ResourceChanged(resource_changed_to_wire(changed)),
                            );
                        }
                        RuntimeEvent::IndexProgress(progress) => {
                            let workspace_id = progress.workspace_id.clone();
                            state.publish_event(
                                workspace_id,
                                event::Body::IndexProgress(index_progress_to_wire(progress)),
                            );
                        }
                    }
                }
            })
            .ok();
    }
}

/// Bind the configured socket and serve until `shutdown` fires.
pub async fn serve_with_shutdown(
    config: DaemonConfig,
    runtime: Arc<LatticeRuntime>,
    shutdown: oneshot::Receiver<()>,
) -> Result<()> {
    let socket_path = config.socket_path.clone();
    prepare_socket_path(&socket_path)?;
    let listener = UnixListener::bind(&socket_path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        let _ = std::fs::set_permissions(&socket_path, perms);
    }
    info!(path = %socket_path.display(), "latticed listening");

    let state = DaemonState::new(config, runtime);
    let mut shutdown = shutdown;
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                info!("latticed shutting down");
                break;
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _)) => {
                        let state = state.clone();
                        tokio::spawn(async move {
                            if let Err(err) = serve_connection(stream, state).await {
                                warn!(error = %err, "connection closed with error");
                            }
                        });
                    }
                    Err(err) => {
                        warn!(error = %err, "accept failed");
                        break;
                    }
                }
            }
        }
    }

    let _ = std::fs::remove_file(&socket_path);
    Ok(())
}

/// Bind and serve until SIGINT or SIGTERM.
pub async fn serve(config: DaemonConfig, runtime: Arc<LatticeRuntime>) -> Result<()> {
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        if let Err(err) = wait_for_shutdown_signal().await {
            warn!(error = %err, "signal handler failed");
        }
        let _ = tx.send(());
    });
    serve_with_shutdown(config, runtime, rx).await
}

async fn wait_for_shutdown_signal() -> std::io::Result<()> {
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    tokio::select! {
        _ = sigterm.recv() => {}
        _ = sigint.recv() => {}
    }
    Ok(())
}

fn prepare_socket_path(path: &PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

async fn serve_connection(stream: UnixStream, state: DaemonState) -> Result<()> {
    let (mut reader, mut writer) = stream.into_split();
    let handshake = read_handshake(&mut reader).await?;
    let accepted = handshake.auth_token == state.config.auth_token
        && handshake.protocol_version == PROTOCOL_VERSION;
    let response = HandshakeResponse {
        accepted,
        protocol_version: PROTOCOL_VERSION,
        instance_id: state.config.instance_id.clone(),
        message: if accepted {
            String::new()
        } else {
            "invalid auth token or protocol version".into()
        },
    };
    let frame = encode_handshake_frame(&response)?;
    writer.write_all(&frame).await?;
    writer.flush().await?;
    if !accepted {
        return Err(Error::HandshakeRejected);
    }

    let writer = Arc::new(Mutex::new(writer));
    let mut event_rx = state.event_tx.subscribe();
    let events_writer = Arc::clone(&writer);
    let event_pump = tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    let envelope = event_envelope(format!("evt-{}", event.sequence), event);
                    match encode_frame(&envelope) {
                        Ok(framed) => {
                            let mut guard = events_writer.lock().await;
                            if guard.write_all(&framed).await.is_err() {
                                break;
                            }
                            if guard.flush().await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    });

    let mut read_buf = BytesMut::new();
    let mut decoder = FrameDecoder::new();
    let result = async {
        loop {
            let envelope = match read_envelope(&mut reader, &mut read_buf, &mut decoder).await {
                Ok(envelope) => envelope,
                Err(err) if is_eof(&err) => return Ok(()),
                Err(err) => return Err(err),
            };

            let request_id = envelope.request_id.clone();
            let reply = match envelope.payload {
                Some(envelope::Payload::Request(req)) => match handle_request(&state, req) {
                    Ok((response, lease_event)) => {
                        if let Some((workspace_id, lease_body)) = lease_event {
                            state.publish_event(
                                workspace_id,
                                event::Body::LeaseChanged(WorkspaceLeaseChanged {
                                    lease: Some(lease_body),
                                }),
                            );
                        }
                        response_envelope(request_id, response)
                    }
                    Err(wire) => error_envelope(request_id, wire),
                },
                _ => error_envelope(
                    request_id,
                    WireError {
                        code: "invalid_payload".into(),
                        message: "expected request envelope".into(),
                        details: None,
                    },
                ),
            };

            let framed = encode_frame(&reply)?;
            {
                let mut guard = writer.lock().await;
                guard.write_all(&framed).await?;
                guard.flush().await?;
            }
        }
    }
    .await;

    event_pump.abort();
    result
}

fn handle_request(
    state: &DaemonState,
    req: Request,
) -> std::result::Result<(Response, Option<(String, lattice_protocol::WorkspaceLease)>), WireError>
{
    let idempotency_key = req.idempotency_key.clone();
    match req.body {
        Some(request::Body::Health(HealthRequest {})) => Ok((
            Response {
                body: Some(response::Body::Health(HealthResponse {
                    status: "ok".into(),
                    protocol_version: PROTOCOL_VERSION,
                    instance_id: state.config.instance_id.clone(),
                })),
            },
            None,
        )),
        Some(request::Body::Ping(PingRequest { nonce })) => Ok((
            Response {
                body: Some(response::Body::Ping(PingResponse { nonce })),
            },
            None,
        )),
        Some(request::Body::OpenWorkspace(OpenWorkspaceRequest { path })) => {
            handle_open_workspace(state, path)
        }
        Some(request::Body::Search(SearchRequest {
            workspace_id,
            query,
        })) => handle_search(state, workspace_id, query),
        Some(request::Body::ApplyPageUpdate(ApplyPageUpdateRequest {
            workspace_id,
            path,
            content,
            expected_revision,
        })) => handle_apply_page_update(
            state,
            workspace_id,
            path,
            content,
            expected_revision,
            idempotency_key,
        ),
        None => Err(WireError {
            code: "invalid_request".into(),
            message: "request body is required".into(),
            details: None,
        }),
    }
}

fn handle_open_workspace(
    state: &DaemonState,
    path: String,
) -> std::result::Result<(Response, Option<(String, lattice_protocol::WorkspaceLease)>), WireError>
{
    let claim = daemon_lease_claim(&state.config);
    let (session, lease_file) = state
        .runtime
        .open_workspace_session_for_write(path.as_str(), &claim)
        .map_err(runtime_error_to_wire)?;

    let wire_lease = lease_to_wire(&lease_file);
    let workspace_id = session.workspace_id().to_string();
    Ok((
        Response {
            body: Some(response::Body::OpenWorkspace(OpenWorkspaceResponse {
                workspace_id: workspace_id.clone(),
                lease: Some(wire_lease.clone()),
            })),
        },
        Some((workspace_id, wire_lease)),
    ))
}

fn handle_search(
    state: &DaemonState,
    workspace_id: String,
    query: String,
) -> std::result::Result<(Response, Option<(String, lattice_protocol::WorkspaceLease)>), WireError>
{
    let session = state
        .runtime
        .get_session_by_id(&workspace_id)
        .ok_or_else(|| WireError {
            code: "workspace_not_found".into(),
            message: format!("workspace session not found for id {workspace_id}"),
            details: None,
        })?;
    // Exercise the warm index; SearchResponse has no hit payload yet (D0/D2).
    let _hits = session.search(&query, 10).map_err(|err| WireError {
        code: "search_failed".into(),
        message: err.to_string(),
        details: None,
    })?;
    Ok((
        Response {
            body: Some(response::Body::Search(SearchResponse {})),
        },
        None,
    ))
}

fn handle_apply_page_update(
    state: &DaemonState,
    workspace_id: String,
    path: String,
    content: String,
    expected_revision: String,
    idempotency_key: Option<String>,
) -> std::result::Result<(Response, Option<(String, lattice_protocol::WorkspaceLease)>), WireError>
{
    let session = state
        .runtime
        .get_session_by_id(&workspace_id)
        .ok_or_else(|| WireError {
            code: "workspace_not_found".into(),
            message: format!("workspace session not found for id {workspace_id}"),
            details: None,
        })?;

    let claim = session
        .write_lease_claim()
        .unwrap_or_else(|| daemon_lease_claim(&state.config));
    require_workspace_lease(session.root(), &claim).map_err(runtime_error_to_wire)?;

    if let Some(key) = idempotency_key.as_ref() {
        if let Some(cached) = session.idempotency().get(key) {
            return Ok((
                Response {
                    body: Some(response::Body::ApplyPageUpdate(ApplyPageUpdateResponse {
                        revision: cached.revision,
                    })),
                },
                None,
            ));
        }
    }

    let revision = lattice_handlers::apply_page_update(
        session.root().to_string_lossy().into_owned(),
        path,
        content,
        expected_revision,
    )
    .map_err(|message| WireError {
        code: "apply_page_update_failed".into(),
        message,
        details: None,
    })?;

    if let Some(key) = idempotency_key {
        session.idempotency().insert(
            key,
            IdempotentOutcome {
                revision: revision.clone(),
            },
        );
    }

    Ok((
        Response {
            body: Some(response::Body::ApplyPageUpdate(ApplyPageUpdateResponse {
                revision,
            })),
        },
        None,
    ))
}

fn runtime_error_to_wire(err: lattice_runtime::Error) -> WireError {
    match &err {
        lattice_runtime::Error::LeaseHeld { .. } => WireError {
            code: "lease_held".into(),
            message: err.to_string(),
            details: None,
        },
        lattice_runtime::Error::LeaseNotHeld { .. } => WireError {
            code: "lease_not_held".into(),
            message: err.to_string(),
            details: None,
        },
        _ => WireError {
            code: "runtime_error".into(),
            message: err.to_string(),
            details: None,
        },
    }
}

async fn read_handshake(reader: &mut OwnedReadHalf) -> Result<HandshakeRequest> {
    let mut buf = BytesMut::new();
    let mut tmp = [0u8; 4096];
    loop {
        if buf.len() >= 4 {
            let declared = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
            if declared > lattice_protocol::MAX_FRAME_LENGTH {
                return Err(Error::Protocol(
                    lattice_protocol::ProtocolError::FrameTooLarge {
                        max_frame_length: lattice_protocol::MAX_FRAME_LENGTH,
                        declared_length: declared,
                    },
                ));
            }
            let frame_len = 4usize.saturating_add(declared);
            if buf.len() >= frame_len {
                return Ok(decode_handshake_frame::<HandshakeRequest>(&buf[..frame_len])?);
            }
        }
        let n = reader.read(&mut tmp).await?;
        if n == 0 {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "client closed during handshake",
            )));
        }
        buf.extend_from_slice(&tmp[..n]);
    }
}

async fn read_envelope(
    reader: &mut OwnedReadHalf,
    read_buf: &mut BytesMut,
    decoder: &mut FrameDecoder,
) -> Result<lattice_protocol::Envelope> {
    loop {
        if let Some(envelope) = decoder.decode(read_buf)? {
            return Ok(envelope);
        }
        let mut tmp = [0u8; 8192];
        let n = reader.read(&mut tmp).await?;
        if n == 0 {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "client closed connection",
            )));
        }
        read_buf.extend_from_slice(&tmp[..n]);
    }
}

fn is_eof(err: &Error) -> bool {
    matches!(
        err,
        Error::Io(e) if e.kind() == std::io::ErrorKind::UnexpectedEof
    )
}

fn resource_changed_to_wire(changed: RuntimeResourceChanged) -> ResourceChanged {
    ResourceChanged {
        path: path_string(&changed.path),
        change: changed.kind.as_str().to_string(),
        revision: changed.revision,
        from_path: changed.from_path.as_ref().map(|p| path_string(p)),
    }
}

fn index_progress_to_wire(progress: RuntimeIndexProgress) -> IndexProgress {
    IndexProgress {
        phase: progress.phase.as_str().to_string(),
        path: progress.path.as_ref().map(|p| path_string(p)),
        detail: progress.detail,
    }
}

fn path_string(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

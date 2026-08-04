//! Platform IPC server for framed control-plane envelopes.
//!
//! Unix: Unix-domain socket. Windows: named pipe (`ServerOptions` multi-instance).

use std::path::PathBuf;
#[cfg(windows)]
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bytes::BytesMut;
use lattice_client::{
    decode_handshake_frame, encode_handshake_frame, HandshakeRequest, HandshakeResponse,
};
use lattice_protocol::{
    encode_frame, envelope, error_envelope, event, event_envelope, request, response,
    response_envelope, ApplyCollabUpdateRequest, ApplyCollabUpdateResponse, ApplyPageUpdateRequest,
    ApplyPageUpdateResponse, CancelAgentRunRequest, CancelAgentRunResponse, CloseCollabDocRequest,
    CloseCollabDocResponse, DisableSemanticSearchRequest, DisableSemanticSearchResponse,
    EnableSemanticSearchRequest, EnableSemanticSearchResponse, Error as WireError, Event,
    FrameDecoder, GetAgentHealthRequest, GetAgentHealthResponse, GetCollabStateRequest,
    GetCollabStateResponse, GetSemanticStatusRequest, GetSemanticStatusResponse, HealthRequest,
    HealthResponse, IndexProgress, OpenCollabDocRequest, OpenCollabDocResponse,
    OpenWorkspaceRequest, OpenWorkspaceResponse, PingRequest, PingResponse, Request,
    ResourceChanged, Response, SearchRequest, SearchResponse,
    SemanticStatus as WireSemanticStatus, StartAgentRunRequest, StartAgentRunResponse,
    WorkspaceLeaseChanged, PROTOCOL_VERSION,
};
use lattice_runtime::{
    IdempotentOutcome, LatticeRuntime, RuntimeEvent, RuntimeIndexProgress, RuntimeResourceChanged,
    SemanticStatus,
};
use lattice_collab::CollabRegistry;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{broadcast, oneshot, Mutex};
use tracing::{debug, info, warn};

use crate::cloud_relay::{spawn_cloud_relay, CloudRelayConfig};
use crate::config::DaemonConfig;
use crate::error::{Error, Result};
use crate::idle::ConnectionTracker;
use crate::lease::{daemon_lease_claim, lease_to_wire, require_workspace_lease};

/// Shared daemon state for accepted connections.
#[derive(Clone)]
pub struct DaemonState {
    pub config: Arc<DaemonConfig>,
    pub runtime: Arc<LatticeRuntime>,
    pub jobs: Arc<crate::jobs::JobRegistry>,
    pub semantic: Option<Arc<crate::embed_host::SemanticController>>,
    pub voice: Option<Arc<crate::voice_host::VoiceController>>,
    pub agent: Option<Arc<crate::agent::AgentController>>,
    /// Yrs sessions with optional `.lattice/collab/` journal persistence.
    pub collab: Arc<Mutex<CollabRegistry>>,
    connections: Option<Arc<ConnectionTracker>>,
    event_tx: broadcast::Sender<Event>,
    /// Quiet bus for agent run chunks; pumped with priority over `event_tx`.
    agent_event_tx: broadcast::Sender<Event>,
    next_event_seq: Arc<AtomicU64>,
}

impl DaemonState {
    pub fn new(config: DaemonConfig, runtime: Arc<LatticeRuntime>) -> Self {
        Self::new_with_controllers(config, runtime, None, None, None)
    }

    pub fn new_with_semantic(
        config: DaemonConfig,
        runtime: Arc<LatticeRuntime>,
        semantic: Option<Arc<crate::embed_host::SemanticController>>,
    ) -> Self {
        Self::new_with_controllers(config, runtime, semantic, None, None)
    }

    pub fn new_with_controllers(
        config: DaemonConfig,
        runtime: Arc<LatticeRuntime>,
        semantic: Option<Arc<crate::embed_host::SemanticController>>,
        voice: Option<Arc<crate::voice_host::VoiceController>>,
        agent: Option<Arc<crate::agent::AgentController>>,
    ) -> Self {
        // Shared fan-out for workspace index/resource/voice events.
        let (event_tx, _) = broadcast::channel(8192);
        // Quiet bus for agent run chunks so IndexProgress cannot Lagged-drop
        // tool-output / run_completed frames mid-turn.
        let (agent_event_tx, _) = broadcast::channel(1024);
        let next_event_seq = Arc::new(AtomicU64::new(1));
        if let Some(voice) = voice.as_ref() {
            voice.attach_event_fanout(event_tx.clone(), Arc::clone(&next_event_seq));
        }
        if let Some(agent) = agent.as_ref() {
            agent.attach_event_fanout(agent_event_tx.clone(), Arc::clone(&next_event_seq));
        }
        let state = Self {
            config: Arc::new(config),
            runtime,
            jobs: Arc::new(crate::jobs::JobRegistry::new()),
            semantic,
            voice,
            agent,
            collab: Arc::new(Mutex::new(CollabRegistry::new())),
            connections: None,
            event_tx,
            agent_event_tx,
            next_event_seq,
        };
        state.spawn_event_bridge();
        state
    }

    fn with_connections(mut self, connections: Arc<ConnectionTracker>) -> Self {
        self.connections = Some(connections);
        self
    }

    /// Shared connection / idle-shutdown tracker when serving live clients.
    pub fn connections(&self) -> Option<&Arc<ConnectionTracker>> {
        self.connections.as_ref()
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
    serve_with_shutdown_and_controllers(config, runtime, None, None, None, shutdown).await
}

/// Bind and serve with an optional semantic indexing controller.
pub async fn serve_with_shutdown_and_semantic(
    config: DaemonConfig,
    runtime: Arc<LatticeRuntime>,
    semantic: Option<Arc<crate::embed_host::SemanticController>>,
    shutdown: oneshot::Receiver<()>,
) -> Result<()> {
    serve_with_shutdown_and_controllers(config, runtime, semantic, None, None, shutdown).await
}

/// Bind and serve with optional semantic + voice + agent controllers.
pub async fn serve_with_shutdown_and_controllers(
    config: DaemonConfig,
    runtime: Arc<LatticeRuntime>,
    semantic: Option<Arc<crate::embed_host::SemanticController>>,
    voice: Option<Arc<crate::voice_host::VoiceController>>,
    agent: Option<Arc<crate::agent::AgentController>>,
    shutdown: oneshot::Receiver<()>,
) -> Result<()> {
    let socket_path = config.socket_path.clone();

    #[cfg(unix)]
    prepare_socket_path(&socket_path)?;

    #[cfg(unix)]
    let listener = {
        use tokio::net::UnixListener;
        let listener = UnixListener::bind(&socket_path)?;
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        let _ = std::fs::set_permissions(&socket_path, perms);
        listener
    };

    #[cfg(windows)]
    let mut pipe_server = {
        use tokio::net::windows::named_pipe::ServerOptions;
        // first_pipe_instance fails fast if another latticed already owns the pipe.
        ServerOptions::new()
            .first_pipe_instance(true)
            .create(&socket_path)?
    };

    info!(path = %socket_path.display(), "latticed listening");

    let (idle_shutdown_tx, idle_shutdown_rx) = oneshot::channel();
    let connections = ConnectionTracker::new(
        config.keep_services_running,
        config.idle_shutdown_timeout,
        idle_shutdown_tx,
    );
    if let Ok(registry) = crate::workspace_registry::WorkspaceRegistry::load_default() {
        crate::workspace_registry::sync_remote_access_lease(&connections, &registry).await;
    }
    if let Some(relay) = CloudRelayConfig::from_env() {
        info!(
            cloud_url = %relay.cloud_url,
            device_id = %relay.device_id,
            "cloud device relay enabled via environment"
        );
        spawn_cloud_relay(
            Arc::clone(&runtime),
            relay,
            Some(Arc::clone(&connections)),
        );
    }
    let state = DaemonState::new_with_controllers(config, runtime, semantic, voice, agent)
        .with_connections(Arc::clone(&connections));
    let schedule_runner = crate::schedule::spawn_schedule_runner_with_connections(
        Arc::clone(&state.runtime),
        Arc::clone(&state.jobs),
        Some(Arc::clone(&connections)),
        crate::DEFAULT_SCHEDULE_TICK,
    );
    let api_shutdown = state
        .config
        .api_port
        .map(|port| crate::http::spawn_localhost_api(state.clone(), port));
    let mut shutdown = shutdown;
    let mut idle_shutdown = idle_shutdown_rx;

    #[cfg(unix)]
    {
        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    info!("latticed shutting down");
                    break;
                }
                _ = &mut idle_shutdown => {
                    info!("latticed idle shutdown after last client disconnected");
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
    }

    #[cfg(windows)]
    {
        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    info!("latticed shutting down");
                    break;
                }
                _ = &mut idle_shutdown => {
                    info!("latticed idle shutdown after last client disconnected");
                    break;
                }
                accepted = accept_named_pipe_client(&mut pipe_server, &socket_path) => {
                    match accepted {
                        Ok(stream) => {
                            let state = state.clone();
                            tokio::spawn(async move {
                                if let Err(err) = serve_connection(stream, state).await {
                                    warn!(error = %err, "connection closed with error");
                                }
                            });
                        }
                        Err(err) => {
                            warn!(error = %err, "named pipe accept failed");
                            break;
                        }
                    }
                }
            }
        }
    }

    schedule_runner.abort();
    if let Some(tx) = api_shutdown {
        let _ = tx.send(());
    }
    if let Some(semantic) = state.semantic.as_ref() {
        semantic.shutdown();
    }
    if let Some(voice) = state.voice.as_ref() {
        voice.shutdown();
    }
    if let Some(agent) = state.agent.as_ref() {
        agent.shutdown().await;
    }
    state.runtime.shutdown_all_sessions();
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(&socket_path);
    }
    Ok(())
}

/// Bind and serve until a platform shutdown signal.
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
    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        let mut sigint =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
        tokio::select! {
            _ = sigterm.recv() => {}
            _ = sigint.recv() => {}
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        tokio::signal::ctrl_c().await
    }
}

#[cfg(unix)]
fn prepare_socket_path(path: &PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Wait for a client, then replace `server` with the next unbound instance.
#[cfg(windows)]
async fn accept_named_pipe_client(
    server: &mut tokio::net::windows::named_pipe::NamedPipeServer,
    pipe_path: &Path,
) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    use tokio::net::windows::named_pipe::ServerOptions;

    server.connect().await?;
    let connected = std::mem::replace(server, ServerOptions::new().create(pipe_path)?);
    Ok(connected)
}

async fn serve_connection<S>(stream: S, state: DaemonState) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut reader, mut writer) = tokio::io::split(stream);
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

    let _connection_guard = if let Some(tracker) = state.connections.as_ref() {
        tracker.on_connect().await;
        Some(tracker.guard())
    } else {
        None
    };

    let writer = Arc::new(Mutex::new(writer));
    let mut event_rx = state.event_tx.subscribe();
    let mut agent_event_rx = state.agent_event_tx.subscribe();
    let events_writer = Arc::clone(&writer);
    let event_pump = tokio::spawn(async move {
        // Prefer agent chunks over IndexProgress so tool-output frames are not
        // queued behind (or Lagged away by) workspace index floods.
        loop {
            let event = tokio::select! {
                biased;
                result = agent_event_rx.recv() => match result {
                    Ok(event) => event,
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                },
                result = event_rx.recv() => match result {
                    Ok(event) => event,
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                },
            };
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
                Some(envelope::Payload::Request(req)) => match handle_request(&state, req).await {
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

async fn handle_request(
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
                    backend: None,
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
            limit,
            mode,
        })) => handle_search(state, workspace_id, query, limit, mode).await,
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
        Some(request::Body::EnableSemanticSearch(EnableSemanticSearchRequest { workspace_id })) => {
            handle_enable_semantic(state, workspace_id)
        }
        Some(request::Body::DisableSemanticSearch(DisableSemanticSearchRequest {
            workspace_id,
        })) => handle_disable_semantic(state, workspace_id),
        Some(request::Body::GetSemanticStatus(GetSemanticStatusRequest { workspace_id })) => {
            handle_get_semantic_status(state, workspace_id)
        }
        Some(request::Body::StartAgentRun(req)) => handle_start_agent_run(state, req).await,
        Some(request::Body::CancelAgentRun(CancelAgentRunRequest { run_id })) => {
            handle_cancel_agent_run(state, run_id).await
        }
        Some(request::Body::GetAgentHealth(GetAgentHealthRequest {})) => {
            handle_get_agent_health(state).await
        }
        Some(request::Body::OpenCollabDoc(OpenCollabDocRequest {
            workspace_id,
            doc_id,
            path,
        })) => {
            handle_open_collab_doc(state, workspace_id, doc_id, path).await
        }
        Some(request::Body::ApplyCollabUpdate(ApplyCollabUpdateRequest {
            workspace_id,
            doc_id,
            update,
        })) => handle_apply_collab_update(state, workspace_id, doc_id, update).await,
        Some(request::Body::GetCollabState(GetCollabStateRequest {
            workspace_id,
            doc_id,
            state_vector,
        })) => handle_get_collab_state(state, workspace_id, doc_id, state_vector).await,
        Some(request::Body::CloseCollabDoc(CloseCollabDocRequest {
            workspace_id,
            doc_id,
        })) => handle_close_collab_doc(state, workspace_id, doc_id).await,
        Some(
            body @ (request::Body::PrepareModel(_)
            | request::Body::GetVoiceCapabilities(_)
            | request::Body::StartVoiceSession(_)
            | request::Body::PushAudioChunk(_)
            | request::Body::FinishUtterance(_)
            | request::Body::UpdateSessionContext(_)
            | request::Body::CancelVoiceSession(_)
            | request::Body::EndVoiceSession(_)
            | request::Body::VoiceHostStatus(_)
            | request::Body::UnloadVoiceModel(_)),
        ) => {
            let voice = state.voice.as_ref().ok_or_else(|| WireError {
                code: "voice_unavailable".into(),
                message: "voice-host is not configured (set LATTICE_VOICE_FAKE=1 or LATTICE_VOICE_HOST_SOCKET)".into(),
                details: None,
            })?;
            let response = voice
                .handle_request(Request {
                    deadline_unix_ms: req.deadline_unix_ms,
                    idempotency_key,
                    body: Some(body),
                })
                .await?;
            Ok((response, None))
        }
        None => Err(WireError {
            code: "invalid_request".into(),
            message: "request body is required".into(),
            details: None,
        }),
    }
}

fn handle_enable_semantic(
    state: &DaemonState,
    workspace_id: String,
) -> std::result::Result<(Response, Option<(String, lattice_protocol::WorkspaceLease)>), WireError>
{
    let semantic = state.semantic.as_ref().ok_or_else(|| WireError {
        code: "semantic_unavailable".into(),
        message: "semantic controller is not configured".into(),
        details: None,
    })?;
    let session = state
        .runtime
        .get_session_by_id(&workspace_id)
        .ok_or_else(|| WireError {
            code: "workspace_not_found".into(),
            message: format!("workspace session not found for id {workspace_id}"),
            details: None,
        })?;

    // Return immediately so the UI can show live downloading → preparing →
    // indexing → ready transitions via SemanticStatusChanged + GetSemanticStatus.
    let early = SemanticStatus::downloading(0);
    session.set_semantic_prepare_status(Some(early.clone()));
    state.publish_event(
        workspace_id.clone(),
        event::Body::SemanticStatus(lattice_protocol::SemanticStatusChanged {
            status: Some(semantic_status_to_wire(&early, Some(semantic))),
        }),
    );

    let state_bg = state.clone();
    let semantic_bg = Arc::clone(semantic);
    let workspace_bg = workspace_id.clone();
    std::thread::Builder::new()
        .name("latticed-semantic-enable".into())
        .spawn(move || {
            let mut last_wire: Option<String> = None;
            let status =
                match semantic_bg.enable_workspace_with_progress(&workspace_bg, &mut |progress| {
                    let wire = semantic_status_to_wire(progress, Some(semantic_bg.as_ref()));
                    let fingerprint = format!(
                        "{}:{}:{:?}",
                        wire.state,
                        wire.progress_percent.unwrap_or_default(),
                        wire.message
                    );
                    if last_wire.as_ref() == Some(&fingerprint) {
                        return;
                    }
                    last_wire = Some(fingerprint);
                    state_bg.publish_event(
                        workspace_bg.clone(),
                        event::Body::SemanticStatus(lattice_protocol::SemanticStatusChanged {
                            status: Some(wire),
                        }),
                    );
                }) {
                    Ok(status) => status,
                    Err(message) => {
                        let failed = SemanticStatus {
                            state: lattice_runtime::SemanticStatusState::Failed,
                            pending_chunks: None,
                            message: Some(message),
                            progress_percent: None,
                        };
                        if let Some(session) = state_bg.runtime.get_session_by_id(&workspace_bg) {
                            session.set_semantic_prepare_status(Some(failed.clone()));
                        }
                        failed
                    }
                };
            state_bg.publish_event(
                workspace_bg,
                event::Body::SemanticStatus(lattice_protocol::SemanticStatusChanged {
                    status: Some(semantic_status_to_wire(&status, Some(semantic_bg.as_ref()))),
                }),
            );
        })
        .map_err(|err| WireError {
            code: "semantic_enable_failed".into(),
            message: format!("failed to spawn semantic enable job: {err}"),
            details: None,
        })?;

    Ok((
        Response {
            body: Some(response::Body::EnableSemanticSearch(
                EnableSemanticSearchResponse {
                    status: Some(semantic_status_to_wire(&early, Some(semantic))),
                },
            )),
        },
        None,
    ))
}

fn handle_disable_semantic(
    state: &DaemonState,
    workspace_id: String,
) -> std::result::Result<(Response, Option<(String, lattice_protocol::WorkspaceLease)>), WireError>
{
    let semantic = state.semantic.as_ref().ok_or_else(|| WireError {
        code: "semantic_unavailable".into(),
        message: "semantic controller is not configured".into(),
        details: None,
    })?;
    let status = semantic
        .disable_workspace(&workspace_id)
        .map_err(|message| WireError {
            code: "semantic_disable_failed".into(),
            message,
            details: None,
        })?;
    state.publish_event(
        workspace_id,
        event::Body::SemanticStatus(lattice_protocol::SemanticStatusChanged {
            status: Some(semantic_status_to_wire(&status, Some(semantic))),
        }),
    );
    Ok((
        Response {
            body: Some(response::Body::DisableSemanticSearch(
                DisableSemanticSearchResponse {
                    status: Some(semantic_status_to_wire(&status, Some(semantic))),
                },
            )),
        },
        None,
    ))
}

fn handle_get_semantic_status(
    state: &DaemonState,
    workspace_id: String,
) -> std::result::Result<(Response, Option<(String, lattice_protocol::WorkspaceLease)>), WireError>
{
    let semantic = state.semantic.as_ref().ok_or_else(|| WireError {
        code: "semantic_unavailable".into(),
        message: "semantic controller is not configured".into(),
        details: None,
    })?;
    let status = semantic.status_for_workspace(&workspace_id);
    Ok((
        Response {
            body: Some(response::Body::GetSemanticStatus(
                GetSemanticStatusResponse {
                    status: Some(semantic_status_to_wire(&status, Some(semantic))),
                },
            )),
        },
        None,
    ))
}

async fn handle_start_agent_run(
    state: &DaemonState,
    req: StartAgentRunRequest,
) -> std::result::Result<(Response, Option<(String, lattice_protocol::WorkspaceLease)>), WireError>
{
    let agent = state.agent.as_ref().ok_or_else(|| WireError {
        code: "agent_unavailable".into(),
        message: "agent runtime is not configured (set LATTICE_AGENT_FAKE=1 or LATTICE_AGENTD_BIN)"
            .into(),
        details: None,
    })?;

    let provider = if req.provider.is_empty() {
        agent.default_provider()
    } else {
        crate::agent::ProviderKind::parse(&req.provider).ok_or_else(|| WireError {
            code: "agent_invalid_request".into(),
            message: format!(
                "unknown provider {:?}; expected pioneer|openai|local|fake",
                req.provider
            ),
            details: None,
        })?
    };
    let model = if req.model.is_empty() {
        agent.default_model()
    } else {
        req.model
    };
    let messages = match req.messages_json.as_deref() {
        None | Some("") => None,
        Some(raw) => {
            let value: serde_json::Value = serde_json::from_str(raw).map_err(|err| WireError {
                code: "agent_invalid_request".into(),
                message: format!("messages_json is not valid JSON: {err}"),
                details: None,
            })?;
            let arr = value.as_array().ok_or_else(|| WireError {
                code: "agent_invalid_request".into(),
                message: "messages_json must be a JSON array".into(),
                details: None,
            })?;
            Some(arr.clone())
        }
    };

    let workspace_root = state
        .runtime
        .get_session_by_id(&req.workspace_id)
        .map(|session| session.root().display().to_string());

    let start = crate::agent::StartAgentRunRequest {
        thread_id: req.thread_id,
        run_id: crate::agent::AgentRunId::new(req.run_id.unwrap_or_default()),
        provider,
        model,
        messages,
        prompt: req.prompt,
        workspace_id: req.workspace_id,
        workspace_root,
    };
    let handle = agent.start_run(start).await.map_err(agent_error_to_wire)?;
    Ok((
        Response {
            body: Some(response::Body::StartAgentRun(StartAgentRunResponse {
                run_id: handle.run_id.to_string(),
                thread_id: handle.thread_id,
            })),
        },
        None,
    ))
}

async fn handle_cancel_agent_run(
    state: &DaemonState,
    run_id: String,
) -> std::result::Result<(Response, Option<(String, lattice_protocol::WorkspaceLease)>), WireError>
{
    let agent = state.agent.as_ref().ok_or_else(|| WireError {
        code: "agent_unavailable".into(),
        message: "agent runtime is not configured (set LATTICE_AGENT_FAKE=1 or LATTICE_AGENTD_BIN)"
            .into(),
        details: None,
    })?;
    agent
        .cancel_run(crate::agent::AgentRunId::new(run_id))
        .await
        .map_err(agent_error_to_wire)?;
    Ok((
        Response {
            body: Some(response::Body::CancelAgentRun(CancelAgentRunResponse {})),
        },
        None,
    ))
}

async fn handle_get_agent_health(
    state: &DaemonState,
) -> std::result::Result<(Response, Option<(String, lattice_protocol::WorkspaceLease)>), WireError>
{
    let agent = state.agent.as_ref().ok_or_else(|| WireError {
        code: "agent_unavailable".into(),
        message: "agent runtime is not configured (set LATTICE_AGENT_FAKE=1 or LATTICE_AGENTD_BIN)"
            .into(),
        details: None,
    })?;
    let health = agent.health().await.map_err(agent_error_to_wire)?;
    Ok((
        Response {
            body: Some(response::Body::GetAgentHealth(GetAgentHealthResponse {
                ok: health.ok,
                backend: health.backend,
                degraded: health.degraded,
            })),
        },
        None,
    ))
}

fn agent_error_to_wire(err: crate::agent::AgentRuntimeError) -> WireError {
    WireError {
        code: err.wire_code().into(),
        message: err.to_string(),
        details: None,
    }
}

fn semantic_status_to_wire(
    status: &SemanticStatus,
    semantic: Option<&crate::embed_host::SemanticController>,
) -> WireSemanticStatus {
    let identity = semantic.map(|controller| controller.provider_identity());
    WireSemanticStatus {
        state: status.state.as_str().to_string(),
        pending_chunks: status.pending_chunks,
        message: status.message.clone(),
        progress_percent: status.progress_percent,
        provider_id: identity.as_ref().map(|id| id.provider_id.clone()),
        model_id: identity.as_ref().and_then(|id| id.model_id.clone()),
        dimensions: identity.as_ref().and_then(|id| id.dimensions),
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

    // Semantic indexing is user-driven via EnableSemanticSearch (E4), not
    // auto-attached on open.
    crate::jobs::reconcile_or_warn(&state.jobs, session.root());

    let wire_lease = lease_to_wire(&lease_file);
    let workspace_id = session.workspace_id().to_string();
    if let Err(err) = crate::workspace_registry::register_workspace(&workspace_id, session.root())
    {
        warn!(
            %workspace_id,
            root = %session.root().display(),
            "failed to persist workspace registry entry: {err}"
        );
    }
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

async fn handle_search(
    state: &DaemonState,
    workspace_id: String,
    query: String,
    limit: u32,
    mode: Option<String>,
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
    let limit = clamp_search_limit(limit);
    let hits = lattice_handlers::search_workspace_ui_with_session_async(
        &session,
        &query,
        limit,
        mode.as_deref(),
    )
    .await
    .map_err(|message| WireError {
        code: "search_failed".into(),
        message,
        details: None,
    })?;
    Ok((
        Response {
            body: Some(response::Body::Search(SearchResponse {
                hits: hits.into_iter().map(search_hit_ui_to_wire).collect(),
            })),
        },
        None,
    ))
}

fn clamp_search_limit(limit: u32) -> usize {
    let raw = if limit == 0 { 10 } else { limit as usize };
    raw.clamp(1, crate::api::MAX_HIT_LIMIT)
}

fn search_hit_ui_to_wire(hit: lattice_handlers::SearchHitUi) -> lattice_protocol::SearchHit {
    lattice_protocol::SearchHit {
        path: hit.path,
        title: hit.title,
        snippet: hit.snippet,
        rank: hit.rank,
        fused_score: hit.fused_score,
        lexical_rank: hit.lexical_rank,
        semantic_rank: hit.semantic_rank,
        heading_path: hit.heading_path.unwrap_or_default(),
        chunk_id: hit.chunk_id,
        sensitivity: hit.sensitivity,
        export_policy: hit.export_policy,
    }
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

fn require_workspace_session(
    state: &DaemonState,
    workspace_id: &str,
) -> std::result::Result<std::sync::Arc<lattice_runtime::WorkspaceSession>, WireError> {
    state
        .runtime
        .get_session_by_id(workspace_id)
        .ok_or_else(|| WireError {
            code: "workspace_not_found".into(),
            message: format!("workspace session not found for id {workspace_id}"),
            details: None,
        })
}

fn collab_error_to_wire(err: lattice_collab::Error) -> WireError {
    let code = match &err {
        lattice_collab::Error::InvalidDocId { .. } => "invalid_collab_doc_id",
        lattice_collab::Error::SessionNotOpen { .. } => "collab_session_not_open",
        lattice_collab::Error::Yrs { .. } => "collab_yrs_error",
        lattice_collab::Error::ResourceResolve { .. } => "collab_resource_resolve_failed",
        lattice_collab::Error::ResourceIdMismatch { .. } => "collab_resource_id_mismatch",
        lattice_collab::Error::Io { .. } => "collab_journal_io_error",
    };
    WireError {
        code: code.into(),
        message: err.to_string(),
        details: None,
    }
}

async fn handle_open_collab_doc(
    state: &DaemonState,
    workspace_id: String,
    doc_id: String,
    path: Option<String>,
) -> std::result::Result<(Response, Option<(String, lattice_protocol::WorkspaceLease)>), WireError>
{
    let session = require_workspace_session(state, &workspace_id)?;
    let root = session.root().to_path_buf();
    let mut collab = state.collab.lock().await;
    let opened = collab
        .open(
            &doc_id,
            Some(root.as_path()),
            path.as_deref(),
        )
        .map_err(collab_error_to_wire)?;
    Ok((
        Response {
            body: Some(response::Body::OpenCollabDoc(OpenCollabDocResponse {
                doc_id: opened.snapshot.doc_id.to_string(),
                state_vector: opened.snapshot.state_vector,
                update: opened.snapshot.update,
                created: opened.created,
            })),
        },
        None,
    ))
}

async fn handle_apply_collab_update(
    state: &DaemonState,
    workspace_id: String,
    doc_id: String,
    update: Vec<u8>,
) -> std::result::Result<(Response, Option<(String, lattice_protocol::WorkspaceLease)>), WireError>
{
    let session = require_workspace_session(state, &workspace_id)?;
    let root = session.root().to_path_buf();
    let mut collab = state.collab.lock().await;
    let snapshot = collab
        .apply_update(&doc_id, &update, Some(root.as_path()))
        .map_err(collab_error_to_wire)?;
    Ok((
        Response {
            body: Some(response::Body::ApplyCollabUpdate(
                ApplyCollabUpdateResponse {
                    doc_id: snapshot.doc_id.to_string(),
                    state_vector: snapshot.state_vector,
                },
            )),
        },
        None,
    ))
}

async fn handle_get_collab_state(
    state: &DaemonState,
    workspace_id: String,
    doc_id: String,
    state_vector: Vec<u8>,
) -> std::result::Result<(Response, Option<(String, lattice_protocol::WorkspaceLease)>), WireError>
{
    let _session = require_workspace_session(state, &workspace_id)?;
    let collab = state.collab.lock().await;
    let snapshot = collab
        .get_state(&doc_id, &state_vector)
        .map_err(collab_error_to_wire)?;
    Ok((
        Response {
            body: Some(response::Body::GetCollabState(GetCollabStateResponse {
                doc_id: snapshot.doc_id.to_string(),
                state_vector: snapshot.state_vector,
                update: snapshot.update,
            })),
        },
        None,
    ))
}

async fn handle_close_collab_doc(
    state: &DaemonState,
    workspace_id: String,
    doc_id: String,
) -> std::result::Result<(Response, Option<(String, lattice_protocol::WorkspaceLease)>), WireError>
{
    let session = require_workspace_session(state, &workspace_id)?;
    let root = session.root().to_path_buf();
    let mut collab = state.collab.lock().await;
    let closed = collab
        .close(&doc_id, Some(root.as_path()))
        .map_err(collab_error_to_wire)?;
    Ok((
        Response {
            body: Some(response::Body::CloseCollabDoc(CloseCollabDocResponse {
                closed,
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

async fn read_handshake<R>(reader: &mut R) -> Result<HandshakeRequest>
where
    R: AsyncRead + Unpin,
{
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
                return Ok(decode_handshake_frame::<HandshakeRequest>(
                    &buf[..frame_len],
                )?);
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

async fn read_envelope<R>(
    reader: &mut R,
    read_buf: &mut BytesMut,
    decoder: &mut FrameDecoder,
) -> Result<lattice_protocol::Envelope>
where
    R: AsyncRead + Unpin,
{
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

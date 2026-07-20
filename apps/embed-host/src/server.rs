use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use bytes::BytesMut;
use lattice_embedding::{
    EmbedDocumentRequest, EmbedQueryRequest, EmbeddingInstallState, EmbeddingSpecification,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tracing::{info, warn};
use uuid::Uuid;

use crate::backend::{open_backend, BackendKind, LoadedBackend};
use crate::error::EmbedHostError;
use crate::framing::{encode_frame, try_decode_frame};
use crate::install::{install_model, load_manifest};
use crate::spec::embedding_spec_to_proto;
use crate::{
    envelope, error_envelope, request, response_envelope, CancelResponse, EmbedDocumentsResponse,
    EmbedQueryResponse, EmbeddingVector, Envelope, HealthResponse, InstallModelResponse,
    LoadModelResponse, PROTOCOL_VERSION, Request, Response, StatusResponse, UnloadModelResponse,
};

/// Host process configuration.
#[derive(Debug, Clone)]
pub struct HostConfig {
    pub socket_path: PathBuf,
    pub backend: BackendKind,
    pub models_dir: PathBuf,
    pub instance_id: String,
}

impl HostConfig {
    pub fn new(socket_path: PathBuf, backend: BackendKind, models_dir: PathBuf) -> Self {
        Self {
            socket_path,
            backend,
            models_dir,
            instance_id: Uuid::now_v7().to_string(),
        }
    }
}

/// Shared host runtime state.
pub struct HostState {
    pub config: HostConfig,
    /// Arc so embed handlers can release the mutex while inference runs, letting
    /// concurrent connections service Health/Status/Cancel/EmbedQuery.
    backend: Mutex<Option<Arc<LoadedBackend>>>,
    install_state: Mutex<EmbeddingInstallState>,
    loaded_model_id: Mutex<Option<String>>,
    specification: Mutex<Option<EmbeddingSpecification>>,
    queries_completed: AtomicU64,
    documents_completed: AtomicU64,
    cancel_count: AtomicU64,
    active: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl HostState {
    pub fn new(config: HostConfig) -> Arc<Self> {
        Arc::new(Self {
            config,
            backend: Mutex::new(None),
            install_state: Mutex::new(EmbeddingInstallState::NotInstalled),
            loaded_model_id: Mutex::new(None),
            specification: Mutex::new(None),
            queries_completed: AtomicU64::new(0),
            documents_completed: AtomicU64::new(0),
            cancel_count: AtomicU64::new(0),
            active: Mutex::new(HashMap::new()),
        })
    }

    pub fn backend_name(&self) -> &'static str {
        self.config.backend.as_str()
    }
}

/// Serve embed-host RPCs on a Unix-domain socket until the listener fails.
pub async fn run_server(state: Arc<HostState>) -> Result<(), EmbedHostError> {
    if let Some(parent) = state.config.socket_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    if state.config.socket_path.exists() {
        tokio::fs::remove_file(&state.config.socket_path).await?;
    }

    let listener = UnixListener::bind(&state.config.socket_path)?;
    info!(
        socket = %state.config.socket_path.display(),
        backend = state.backend_name(),
        "embed-host listening"
    );

    loop {
        let (stream, _) = listener.accept().await?;
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(error) = handle_connection(state, stream).await {
                warn!(error = %error, "embed-host connection closed with error");
            }
        });
    }
}

async fn handle_connection(
    state: Arc<HostState>,
    mut stream: UnixStream,
) -> Result<(), EmbedHostError> {
    let mut buffer = BytesMut::with_capacity(4096);
    let mut read_buf = [0u8; 8192];

    loop {
        let n = stream.read(&mut read_buf).await?;
        if n == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(&read_buf[..n]);

        while let Some(envelope) = try_decode_frame(&mut buffer)? {
            let reply = dispatch(Arc::clone(&state), envelope).await;
            let framed = encode_frame(&reply)?;
            stream.write_all(&framed).await?;
        }
    }
}

async fn dispatch(state: Arc<HostState>, envelope: Envelope) -> Envelope {
    let request_id = envelope.request_id.clone();
    if envelope.protocol_version != PROTOCOL_VERSION {
        return error_envelope(
            request_id,
            crate::Error {
                code: "protocol_version".into(),
                message: format!(
                    "unsupported protocol version {} (host speaks {})",
                    envelope.protocol_version, PROTOCOL_VERSION
                ),
                details: None,
            },
        );
    }

    let Some(envelope::Payload::Request(request)) = envelope.payload else {
        return error_envelope(
            request_id,
            crate::Error {
                code: "invalid_payload".into(),
                message: "expected request payload".into(),
                details: None,
            },
        );
    };

    match handle_request(state, &request_id, request).await {
        Ok(response) => response_envelope(request_id, response),
        Err(error) => error_envelope(request_id, remote_error(error)),
    }
}

fn remote_error(error: EmbedHostError) -> crate::Error {
    let (code, message) = match &error {
        EmbedHostError::Cancelled => ("cancelled", error.to_string()),
        EmbedHostError::ModelNotLoaded => ("model_not_loaded", error.to_string()),
        EmbedHostError::BackendUnavailable(message) => ("backend_unavailable", message.clone()),
        EmbedHostError::Embedding(inner) => ("embedding", inner.to_string()),
        EmbedHostError::Remote { code, message } => {
            return crate::Error {
                code: code.clone(),
                message: message.clone(),
                details: None,
            };
        }
        other => ("host_error", other.to_string()),
    };
    crate::Error {
        code: code.into(),
        message,
        details: None,
    }
}

async fn handle_request(
    state: Arc<HostState>,
    request_id: &str,
    request: Request,
) -> Result<Response, EmbedHostError> {
    let body = request
        .body
        .ok_or_else(|| EmbedHostError::protocol("request body is required"))?;

    match body {
        request::Body::Health(_) => Ok(Response {
            body: Some(crate::response::Body::Health(HealthResponse {
                status: "ok".into(),
                protocol_version: PROTOCOL_VERSION,
                instance_id: state.config.instance_id.clone(),
                backend: state.backend_name().into(),
            })),
        }),
        request::Body::Status(_) => {
            let install_state = *state.install_state.lock().await;
            let spec = state.specification.lock().await.clone();
            let loaded_model_id = state.loaded_model_id.lock().await.clone();
            let active_requests = state.active.lock().await.len() as u64;
            Ok(Response {
                body: Some(crate::response::Body::Status(StatusResponse {
                    install_state: install_state_wire(install_state),
                    backend: state.backend_name().into(),
                    spec: spec.as_ref().map(embedding_spec_to_proto),
                    queries_completed: state.queries_completed.load(Ordering::Relaxed),
                    documents_completed: state.documents_completed.load(Ordering::Relaxed),
                    active_requests,
                    cancel_count: state.cancel_count.load(Ordering::Relaxed),
                    loaded_model_id,
                })),
            })
        }
        request::Body::LoadModel(load) => handle_load(state, load).await,
        request::Body::UnloadModel(_) => handle_unload(state).await,
        request::Body::EmbedQuery(embed) => {
            handle_embed_query(state, request_id, embed.text).await
        }
        request::Body::EmbedDocuments(embed) => {
            handle_embed_documents(state, request_id, embed.documents).await
        }
        request::Body::Cancel(cancel) => {
            let cancelled = {
                let mut active = state.active.lock().await;
                if let Some(flag) = active.remove(&cancel.target_request_id) {
                    flag.store(true, Ordering::SeqCst);
                    state.cancel_count.fetch_add(1, Ordering::Relaxed);
                    true
                } else {
                    false
                }
            };
            Ok(Response {
                body: Some(crate::response::Body::Cancel(CancelResponse { cancelled })),
            })
        }
        request::Body::InstallModel(install) => {
            let result = tokio::task::spawn_blocking({
                let manifest_path = PathBuf::from(install.manifest_path);
                let artifact_path = PathBuf::from(install.artifact_path);
                let models_dir = if install.models_dir.is_empty() {
                    state.config.models_dir.clone()
                } else {
                    PathBuf::from(install.models_dir)
                };
                move || install_model(&manifest_path, &artifact_path, &models_dir)
            })
            .await
            .map_err(|error| EmbedHostError::protocol(format!("install task failed: {error}")))??;

            *state.install_state.lock().await = EmbeddingInstallState::NotInstalled;
            Ok(Response {
                body: Some(crate::response::Body::InstallModel(InstallModelResponse {
                    model_dir: result.model_dir.display().to_string(),
                    artifact_sha256: result.artifact_sha256,
                    install_state: install_state_wire(result.install_state),
                })),
            })
        }
    }
}

async fn handle_load(
    state: Arc<HostState>,
    load: crate::LoadModelRequest,
) -> Result<Response, EmbedHostError> {
    *state.install_state.lock().await = EmbeddingInstallState::Loading;
    if load.model_dir.is_empty() {
        *state.install_state.lock().await = EmbeddingInstallState::Failed;
        return Err(EmbedHostError::protocol("model_dir is required"));
    }
    let model_dir = PathBuf::from(load.model_dir);

    let opened = tokio::task::spawn_blocking({
        let model_dir = model_dir.clone();
        let backend = state.config.backend;
        let dimensions = load.dimensions;
        move || {
            let (manifest, artifact_path) = load_manifest(&model_dir)?;
            let dims = dimensions.unwrap_or(manifest.default_dimensions);
            let loaded = LoadedBackend::new(open_backend(
                backend,
                &manifest,
                &artifact_path,
                dims,
            )?);
            let model_id = manifest.model_id.clone();
            let spec = loaded.specification().clone();
            Ok::<_, EmbedHostError>((loaded, model_id, spec))
        }
    })
    .await
    .map_err(|error| EmbedHostError::protocol(format!("load task failed: {error}")))?;

    match opened {
        Ok((loaded, model_id, spec)) => {
            *state.backend.lock().await = Some(Arc::new(loaded));
            *state.loaded_model_id.lock().await = Some(model_id);
            *state.specification.lock().await = Some(spec.clone());
            *state.install_state.lock().await = EmbeddingInstallState::Ready;
            Ok(Response {
                body: Some(crate::response::Body::LoadModel(LoadModelResponse {
                    spec: Some(embedding_spec_to_proto(&spec)),
                    install_state: install_state_wire(EmbeddingInstallState::Ready),
                })),
            })
        }
        Err(error) => {
            *state.install_state.lock().await = EmbeddingInstallState::Failed;
            Err(error)
        }
    }
}

async fn handle_unload(state: Arc<HostState>) -> Result<Response, EmbedHostError> {
    *state.backend.lock().await = None;
    *state.loaded_model_id.lock().await = None;
    *state.specification.lock().await = None;
    *state.install_state.lock().await = EmbeddingInstallState::NotInstalled;
    Ok(Response {
        body: Some(crate::response::Body::UnloadModel(UnloadModelResponse {
            install_state: install_state_wire(EmbeddingInstallState::NotInstalled),
        })),
    })
}

async fn handle_embed_query(
    state: Arc<HostState>,
    request_id: &str,
    text: String,
) -> Result<Response, EmbedHostError> {
    let cancel = register_active(&state, request_id).await;
    let result = async {
        ensure_not_cancelled(&cancel)?;
        let backend = {
            let guard = state.backend.lock().await;
            Arc::clone(guard.as_ref().ok_or(EmbedHostError::ModelNotLoaded)?)
        };
        ensure_not_cancelled(&cancel)?;
        let vector = backend
            .embed_query(EmbedQueryRequest { text })
            .await?;
        ensure_not_cancelled(&cancel)?;
        state.queries_completed.fetch_add(1, Ordering::Relaxed);
        Ok(Response {
            body: Some(crate::response::Body::EmbedQuery(EmbedQueryResponse {
                values: vector.values,
            })),
        })
    }
    .await;
    unregister_active(&state, request_id).await;
    result
}

async fn handle_embed_documents(
    state: Arc<HostState>,
    request_id: &str,
    documents: Vec<crate::EmbedDocument>,
) -> Result<Response, EmbedHostError> {
    let cancel = register_active(&state, request_id).await;
    let result = async {
        ensure_not_cancelled(&cancel)?;
        let requests: Vec<EmbedDocumentRequest> = documents
            .into_iter()
            .map(|document| EmbedDocumentRequest {
                chunk_id: document.chunk_id,
                text: document.text,
            })
            .collect();
        let count = requests.len() as u64;
        let backend = {
            let guard = state.backend.lock().await;
            Arc::clone(guard.as_ref().ok_or(EmbedHostError::ModelNotLoaded)?)
        };
        ensure_not_cancelled(&cancel)?;
        let vectors = backend.embed_documents(requests).await?;
        ensure_not_cancelled(&cancel)?;
        state
            .documents_completed
            .fetch_add(count, Ordering::Relaxed);
        Ok(Response {
            body: Some(crate::response::Body::EmbedDocuments(
                EmbedDocumentsResponse {
                    vectors: vectors
                        .into_iter()
                        .map(|vector| EmbeddingVector {
                            values: vector.values,
                        })
                        .collect(),
                },
            )),
        })
    }
    .await;
    unregister_active(&state, request_id).await;
    result
}

async fn register_active(state: &HostState, request_id: &str) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    state
        .active
        .lock()
        .await
        .insert(request_id.to_string(), Arc::clone(&flag));
    flag
}

async fn unregister_active(state: &HostState, request_id: &str) {
    state.active.lock().await.remove(request_id);
}

fn ensure_not_cancelled(flag: &AtomicBool) -> Result<(), EmbedHostError> {
    if flag.load(Ordering::SeqCst) {
        Err(EmbedHostError::Cancelled)
    } else {
        Ok(())
    }
}

fn install_state_wire(state: EmbeddingInstallState) -> String {
    match state {
        EmbeddingInstallState::NotInstalled => "not-installed".into(),
        EmbeddingInstallState::Loading => "loading".into(),
        EmbeddingInstallState::Ready => "ready".into(),
        EmbeddingInstallState::Degraded => "degraded".into(),
        EmbeddingInstallState::Failed => "failed".into(),
    }
}

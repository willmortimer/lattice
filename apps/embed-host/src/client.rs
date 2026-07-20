use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use bytes::BytesMut;
use lattice_embedding::{
    EmbedDocumentRequest, EmbedQueryRequest, EmbeddingProvider, EmbeddingSpecification,
    EmbeddingVector,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::EmbedHostError;
use crate::framing::{encode_frame, try_decode_frame};
use crate::spec::embedding_spec_from_proto;
use crate::{
    envelope, request, request_envelope, CancelRequest, EmbedDocument, EmbedDocumentsRequest,
    EmbedQueryRequest as ProtoEmbedQueryRequest, HealthRequest, InstallModelRequest,
    LoadModelRequest, Request, StatusRequest, UnloadModelRequest, PROTOCOL_VERSION,
};

/// Which dedicated UDS connection an RPC uses.
///
/// Query/control stays free while indexing holds the index connection, so
/// interactive [`EmbedQuery`] / [`Cancel`] are not head-of-line blocked.
#[derive(Debug, Clone, Copy)]
enum ClientLane {
    /// EmbedQuery, Health, Status, Cancel.
    Query,
    /// EmbedDocuments, Load, Unload, Install.
    Index,
}

/// Client that speaks the embed-host UDS protocol.
///
/// Opens two connections to the same socket so document indexing cannot block
/// query/control RPCs (the server already accepts concurrent connections).
pub struct EmbedHostClient {
    query_stream: Mutex<UnixStream>,
    index_stream: Mutex<UnixStream>,
}

impl EmbedHostClient {
    /// Connect to a running embed-host socket.
    pub async fn connect(socket_path: impl AsRef<Path>) -> Result<Self, EmbedHostError> {
        let path = socket_path.as_ref();
        let query_stream = UnixStream::connect(path).await?;
        let index_stream = UnixStream::connect(path).await?;
        Ok(Self {
            query_stream: Mutex::new(query_stream),
            index_stream: Mutex::new(index_stream),
        })
    }

    /// Health check.
    pub async fn health(&self) -> Result<crate::HealthResponse, EmbedHostError> {
        let response = self
            .call(
                ClientLane::Query,
                Request {
                    deadline_unix_ms: None,
                    body: Some(request::Body::Health(HealthRequest {})),
                },
            )
            .await?;
        match response.body {
            Some(crate::response::Body::Health(health)) => Ok(health),
            other => Err(EmbedHostError::protocol(format!(
                "unexpected health response: {other:?}"
            ))),
        }
    }

    /// Host status and metrics.
    pub async fn status(&self) -> Result<crate::StatusResponse, EmbedHostError> {
        let response = self
            .call(
                ClientLane::Query,
                Request {
                    deadline_unix_ms: None,
                    body: Some(request::Body::Status(StatusRequest {})),
                },
            )
            .await?;
        match response.body {
            Some(crate::response::Body::Status(status)) => Ok(status),
            other => Err(EmbedHostError::protocol(format!(
                "unexpected status response: {other:?}"
            ))),
        }
    }

    /// Explicit model install (never runs inside search).
    pub async fn install_model(
        &self,
        manifest_path: impl AsRef<Path>,
        artifact_path: impl AsRef<Path>,
        models_dir: impl AsRef<Path>,
    ) -> Result<crate::InstallModelResponse, EmbedHostError> {
        let response = self
            .call(
                ClientLane::Index,
                Request {
                    deadline_unix_ms: None,
                    body: Some(request::Body::InstallModel(InstallModelRequest {
                        manifest_path: manifest_path.as_ref().display().to_string(),
                        artifact_path: artifact_path.as_ref().display().to_string(),
                        models_dir: models_dir.as_ref().display().to_string(),
                    })),
                },
            )
            .await?;
        match response.body {
            Some(crate::response::Body::InstallModel(install)) => Ok(install),
            other => Err(EmbedHostError::protocol(format!(
                "unexpected install response: {other:?}"
            ))),
        }
    }

    /// Load a verified model directory and return an [`EmbeddingProvider`] session.
    pub async fn load_model(
        self: &Arc<Self>,
        model_dir: impl AsRef<Path>,
        dimensions: Option<u32>,
    ) -> Result<EmbedHostSession, EmbedHostError> {
        let response = self
            .call(
                ClientLane::Index,
                Request {
                    deadline_unix_ms: None,
                    body: Some(request::Body::LoadModel(LoadModelRequest {
                        model_dir: model_dir.as_ref().display().to_string(),
                        dimensions,
                    })),
                },
            )
            .await?;
        match response.body {
            Some(crate::response::Body::LoadModel(load)) => {
                let spec = load
                    .spec
                    .ok_or_else(|| EmbedHostError::protocol("load response missing spec"))?;
                let specification =
                    embedding_spec_from_proto(&spec).map_err(EmbedHostError::protocol)?;
                Ok(EmbedHostSession {
                    client: Arc::clone(self),
                    specification,
                })
            }
            other => Err(EmbedHostError::protocol(format!(
                "unexpected load response: {other:?}"
            ))),
        }
    }

    /// Connect to a host socket and load a model in one step.
    pub async fn connect_and_load(
        socket_path: impl AsRef<Path>,
        model_dir: impl AsRef<Path>,
        dimensions: Option<u32>,
    ) -> Result<EmbedHostSession, EmbedHostError> {
        let client = Arc::new(Self::connect(socket_path).await?);
        client.load_model(model_dir, dimensions).await
    }

    /// Unload the current model.
    pub async fn unload_model(&self) -> Result<(), EmbedHostError> {
        let response = self
            .call(
                ClientLane::Index,
                Request {
                    deadline_unix_ms: None,
                    body: Some(request::Body::UnloadModel(UnloadModelRequest {})),
                },
            )
            .await?;
        match response.body {
            Some(crate::response::Body::UnloadModel(_)) => Ok(()),
            other => Err(EmbedHostError::protocol(format!(
                "unexpected unload response: {other:?}"
            ))),
        }
    }

    /// Cancel an in-flight request by id.
    ///
    /// Uses the query/control connection so cancel is not blocked by indexing.
    pub async fn cancel(
        &self,
        target_request_id: impl Into<String>,
    ) -> Result<bool, EmbedHostError> {
        let response = self
            .call(
                ClientLane::Query,
                Request {
                    deadline_unix_ms: None,
                    body: Some(request::Body::Cancel(CancelRequest {
                        target_request_id: target_request_id.into(),
                    })),
                },
            )
            .await?;
        match response.body {
            Some(crate::response::Body::Cancel(cancel)) => Ok(cancel.cancelled),
            other => Err(EmbedHostError::protocol(format!(
                "unexpected cancel response: {other:?}"
            ))),
        }
    }

    pub(crate) async fn embed_query_rpc(
        &self,
        request: EmbedQueryRequest,
    ) -> Result<EmbeddingVector, EmbedHostError> {
        let response = self
            .call(
                ClientLane::Query,
                Request {
                    deadline_unix_ms: None,
                    body: Some(request::Body::EmbedQuery(ProtoEmbedQueryRequest {
                        text: request.text,
                    })),
                },
            )
            .await?;
        match response.body {
            Some(crate::response::Body::EmbedQuery(embed)) => Ok(EmbeddingVector {
                values: embed.values,
            }),
            other => Err(EmbedHostError::protocol(format!(
                "unexpected embed_query response: {other:?}"
            ))),
        }
    }

    pub(crate) async fn embed_documents_rpc(
        &self,
        requests: Vec<EmbedDocumentRequest>,
    ) -> Result<Vec<EmbeddingVector>, EmbedHostError> {
        let response = self
            .call(
                ClientLane::Index,
                Request {
                    deadline_unix_ms: None,
                    body: Some(request::Body::EmbedDocuments(EmbedDocumentsRequest {
                        documents: requests
                            .into_iter()
                            .map(|request| EmbedDocument {
                                chunk_id: request.chunk_id,
                                text: request.text,
                            })
                            .collect(),
                    })),
                },
            )
            .await?;
        match response.body {
            Some(crate::response::Body::EmbedDocuments(embed)) => Ok(embed
                .vectors
                .into_iter()
                .map(|vector| EmbeddingVector {
                    values: vector.values,
                })
                .collect()),
            other => Err(EmbedHostError::protocol(format!(
                "unexpected embed_documents response: {other:?}"
            ))),
        }
    }

    async fn call(
        &self,
        lane: ClientLane,
        request: Request,
    ) -> Result<crate::Response, EmbedHostError> {
        let request_id = Uuid::now_v7().to_string();
        let envelope = request_envelope(request_id.clone(), request);
        let framed = encode_frame(&envelope)?;

        let stream = match lane {
            ClientLane::Query => &self.query_stream,
            ClientLane::Index => &self.index_stream,
        };
        let mut stream = stream.lock().await;
        stream.write_all(&framed).await?;

        let mut buffer = BytesMut::new();
        let mut read_buf = [0u8; 8192];
        loop {
            let n = stream.read(&mut read_buf).await?;
            if n == 0 {
                return Err(EmbedHostError::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "embed-host connection closed",
                )));
            }
            buffer.extend_from_slice(&read_buf[..n]);
            if let Some(reply) = try_decode_frame(&mut buffer)? {
                return match reply.payload {
                    Some(envelope::Payload::Response(response)) => {
                        if reply.request_id != request_id {
                            return Err(EmbedHostError::protocol(format!(
                                "request id mismatch: sent {}, got {}",
                                request_id, reply.request_id
                            )));
                        }
                        if reply.protocol_version != PROTOCOL_VERSION {
                            return Err(EmbedHostError::protocol(format!(
                                "unexpected protocol version {}",
                                reply.protocol_version
                            )));
                        }
                        Ok(response)
                    }
                    Some(envelope::Payload::Error(error)) => {
                        Err(EmbedHostError::remote(error.code, error.message))
                    }
                    other => Err(EmbedHostError::protocol(format!(
                        "unexpected reply payload: {other:?}"
                    ))),
                };
            }
        }
    }
}

/// Embedding provider bound to a loaded host model.
///
/// Created by [`EmbedHostClient::load_model`]. Holds a specification snapshot
/// from load time; call [`EmbedHostClient::unload_model`] when finished.
pub struct EmbedHostSession {
    client: Arc<EmbedHostClient>,
    specification: EmbeddingSpecification,
}

#[async_trait]
impl EmbeddingProvider for EmbedHostSession {
    fn specification(&self) -> &EmbeddingSpecification {
        &self.specification
    }

    async fn embed_query(
        &self,
        request: EmbedQueryRequest,
    ) -> Result<EmbeddingVector, lattice_embedding::EmbeddingError> {
        self.client
            .embed_query_rpc(request)
            .await
            .map_err(map_to_embedding_error)
    }

    async fn embed_documents(
        &self,
        requests: Vec<EmbedDocumentRequest>,
    ) -> Result<Vec<EmbeddingVector>, lattice_embedding::EmbeddingError> {
        self.client
            .embed_documents_rpc(requests)
            .await
            .map_err(map_to_embedding_error)
    }
}

/// [`EmbeddingProvider`] that talks to embed-host over UDS and reconnects after
/// host crashes / restarts.
///
/// Specification is stored behind an atomic pointer so [`Self::reload_model`]
/// can swap the loaded GGUF after E5 prepare without rebuilding the provider.
/// Outstanding `&EmbeddingSpecification` references from before a swap remain
/// valid for the process lifetime (previous boxes are leaked on change).
pub struct ReconnectableEmbedHostProvider {
    socket: PathBuf,
    model_dir: Mutex<PathBuf>,
    dimensions: Mutex<Option<u32>>,
    specification: AtomicPtr<EmbeddingSpecification>,
    session: Mutex<Option<EmbedHostSession>>,
}

impl ReconnectableEmbedHostProvider {
    /// Connect, load the model, and return a reconnecting provider.
    pub async fn connect(
        socket: impl Into<PathBuf>,
        model_dir: impl Into<PathBuf>,
        dimensions: Option<u32>,
    ) -> Result<Self, EmbedHostError> {
        let socket = socket.into();
        let model_dir = model_dir.into();
        let session = EmbedHostClient::connect_and_load(&socket, &model_dir, dimensions).await?;
        let specification = Box::into_raw(Box::new(session.specification().clone()));
        Ok(Self {
            socket,
            model_dir: Mutex::new(model_dir),
            dimensions: Mutex::new(dimensions),
            specification: AtomicPtr::new(specification),
            session: Mutex::new(Some(session)),
        })
    }

    /// Drop the live session and reconnect + reload the configured model.
    pub async fn reconnect(&self) -> Result<(), EmbedHostError> {
        let model_dir = self.model_dir.lock().await.clone();
        let dimensions = *self.dimensions.lock().await;
        let session =
            EmbedHostClient::connect_and_load(&self.socket, &model_dir, dimensions).await?;
        self.store_specification(session.specification().clone());
        *self.session.lock().await = Some(session);
        Ok(())
    }

    /// Point at a different verified model directory and reload (post-prepare).
    pub async fn reload_model(
        &self,
        model_dir: impl Into<PathBuf>,
        dimensions: Option<u32>,
    ) -> Result<(), EmbedHostError> {
        let model_dir = model_dir.into();
        *self.model_dir.lock().await = model_dir.clone();
        *self.dimensions.lock().await = dimensions;
        let session =
            EmbedHostClient::connect_and_load(&self.socket, &model_dir, dimensions).await?;
        self.store_specification(session.specification().clone());
        *self.session.lock().await = Some(session);
        Ok(())
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    pub async fn model_dir(&self) -> PathBuf {
        self.model_dir.lock().await.clone()
    }

    fn store_specification(&self, specification: EmbeddingSpecification) {
        let new = Box::into_raw(Box::new(specification));
        let old = self.specification.swap(new, Ordering::AcqRel);
        if !old.is_null() {
            // Keep outstanding `specification()` refs valid; reloads are rare.
            std::mem::forget(unsafe { Box::from_raw(old) });
        }
    }

    async fn client(&self) -> Result<Arc<EmbedHostClient>, EmbedHostError> {
        {
            let guard = self.session.lock().await;
            if let Some(session) = guard.as_ref() {
                return Ok(Arc::clone(&session.client));
            }
        }
        self.reconnect().await?;
        let guard = self.session.lock().await;
        guard
            .as_ref()
            .map(|session| Arc::clone(&session.client))
            .ok_or_else(|| EmbedHostError::protocol("embed-host session missing after reconnect"))
    }
}

impl Drop for ReconnectableEmbedHostProvider {
    fn drop(&mut self) {
        let ptr = self.specification.swap(std::ptr::null_mut(), Ordering::AcqRel);
        if !ptr.is_null() {
            unsafe {
                drop(Box::from_raw(ptr));
            }
        }
    }
}

#[async_trait]
impl EmbeddingProvider for ReconnectableEmbedHostProvider {
    fn specification(&self) -> &EmbeddingSpecification {
        let ptr = self.specification.load(Ordering::Acquire);
        assert!(!ptr.is_null(), "embed-host specification pointer missing");
        unsafe { &*ptr }
    }

    async fn embed_query(
        &self,
        request: EmbedQueryRequest,
    ) -> Result<EmbeddingVector, lattice_embedding::EmbeddingError> {
        let client = self.client().await.map_err(map_to_embedding_error)?;
        match client.embed_query_rpc(request.clone()).await {
            Ok(vector) => Ok(vector),
            Err(_) => {
                self.reconnect().await.map_err(map_to_embedding_error)?;
                let client = self.client().await.map_err(map_to_embedding_error)?;
                client
                    .embed_query_rpc(request)
                    .await
                    .map_err(map_to_embedding_error)
            }
        }
    }

    async fn embed_documents(
        &self,
        requests: Vec<EmbedDocumentRequest>,
    ) -> Result<Vec<EmbeddingVector>, lattice_embedding::EmbeddingError> {
        let client = self.client().await.map_err(map_to_embedding_error)?;
        match client.embed_documents_rpc(requests.clone()).await {
            Ok(vectors) => Ok(vectors),
            Err(_) => {
                self.reconnect().await.map_err(map_to_embedding_error)?;
                let client = self.client().await.map_err(map_to_embedding_error)?;
                client
                    .embed_documents_rpc(requests)
                    .await
                    .map_err(map_to_embedding_error)
            }
        }
    }
}

fn map_to_embedding_error(error: EmbedHostError) -> lattice_embedding::EmbeddingError {
    match error {
        EmbedHostError::Embedding(inner) => inner,
        EmbedHostError::ModelNotLoaded => {
            lattice_embedding::EmbeddingError::provider("model not loaded")
        }
        EmbedHostError::Cancelled => lattice_embedding::EmbeddingError::provider("cancelled"),
        EmbedHostError::Remote { code, message } => {
            lattice_embedding::EmbeddingError::provider(format!("{code}: {message}"))
        }
        other => lattice_embedding::EmbeddingError::provider(other.to_string()),
    }
}

/// Helper used by tests to build a socket path under a temp directory.
pub fn socket_path_in(dir: impl AsRef<Path>) -> PathBuf {
    dir.as_ref().join("embed-host.sock")
}

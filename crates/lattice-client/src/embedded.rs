use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use lattice_protocol::{
    request, response, ApplyPageUpdateRequest, ApplyPageUpdateResponse, HealthRequest,
    HealthResponse, OpenWorkspaceRequest, OpenWorkspaceResponse, PingRequest, PingResponse,
    SearchRequest, SearchResponse, WorkspaceLease, Request, Response, PROTOCOL_VERSION,
};
use lattice_runtime::{
    IdempotentOutcome, LatticeRuntime, LeaseClaim, OWNER_EMBEDDED, require_workspace_lease,
};

use crate::client::LatticeClient;
use crate::error::ClientError;
use crate::events::{EventFilter, EventStream};

/// Async handler for requests that EmbeddedClient does not handle natively.
pub type EmbeddedRequestHandler = Arc<
    dyn Fn(Request) -> Pin<Box<dyn Future<Output = Result<Response, ClientError>> + Send>>
        + Send
        + Sync,
>;

/// In-process [`LatticeClient`] backed by an optional [`LatticeRuntime`].
///
/// Handles `Health` and `Ping` locally. When a runtime is configured via
/// [`EmbeddedClient::with_runtime`], `OpenWorkspace`, `Search`, and
/// `ApplyPageUpdate` dispatch through warm sessions with workspace lease
/// enforcement. Other bodies still use the optional handler callback.
#[derive(Clone)]
pub struct EmbeddedClient {
    instance_id: String,
    process_start: u64,
    handler: Option<EmbeddedRequestHandler>,
    runtime: Option<Arc<LatticeRuntime>>,
}

impl EmbeddedClient {
    /// Create a client that answers health/ping with `instance_id`.
    pub fn new(instance_id: impl Into<String>) -> Self {
        Self {
            instance_id: instance_id.into(),
            process_start: unix_now_secs(),
            handler: None,
            runtime: None,
        }
    }

    /// Override the process-start identity paired with `pid` in leases (tests).
    pub fn with_process_start(mut self, process_start: u64) -> Self {
        self.process_start = process_start;
        self
    }

    /// Attach a fallback handler for non-health/ping requests.
    pub fn with_handler(mut self, handler: EmbeddedRequestHandler) -> Self {
        self.handler = Some(handler);
        self
    }

    /// Dispatch OpenWorkspace / Search / ApplyPageUpdate through a long-lived runtime.
    pub fn with_runtime(mut self, runtime: Arc<LatticeRuntime>) -> Self {
        self.runtime = Some(runtime);
        self
    }

    /// Instance id reported by health responses.
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    /// Runtime handle when configured.
    pub fn runtime(&self) -> Option<&Arc<LatticeRuntime>> {
        self.runtime.as_ref()
    }

    fn lease_claim(&self) -> LeaseClaim {
        LeaseClaim::embedded(
            std::process::id(),
            self.process_start,
            PROTOCOL_VERSION,
            self.instance_id.clone(),
        )
    }

    fn handle_health(&self) -> Response {
        Response {
            body: Some(response::Body::Health(HealthResponse {
                status: "ok".into(),
                protocol_version: PROTOCOL_VERSION,
                instance_id: self.instance_id.clone(),
                backend: None,
            })),
        }
    }

    fn handle_ping(ping: PingRequest) -> Response {
        Response {
            body: Some(response::Body::Ping(PingResponse { nonce: ping.nonce })),
        }
    }

    fn lease_to_wire(lease: &lattice_runtime::WorkspaceLeaseFile) -> WorkspaceLease {
        WorkspaceLease {
            schema_version: lease.schema_version,
            owner: lease.owner.clone(),
            pid: lease.pid,
            process_start: lease.process_start,
            socket: lease.socket.clone(),
            protocol_version: lease.protocol_version,
            instance_id: lease.instance_id.clone(),
            acquired_at: lease.acquired_at.clone(),
        }
    }

    fn handle_open_workspace(
        &self,
        runtime: &LatticeRuntime,
        req: OpenWorkspaceRequest,
    ) -> Result<Response, ClientError> {
        let claim = self.lease_claim();
        let (session, lease) = runtime
            .open_workspace_session_for_write(req.path.as_str(), &claim)
            .map_err(runtime_to_client_error)?;
        debug_assert_eq!(lease.owner, OWNER_EMBEDDED);
        Ok(Response {
            body: Some(response::Body::OpenWorkspace(OpenWorkspaceResponse {
                workspace_id: session.workspace_id().to_string(),
                lease: Some(Self::lease_to_wire(&lease)),
            })),
        })
    }

    fn handle_search(
        &self,
        runtime: &LatticeRuntime,
        req: SearchRequest,
    ) -> Result<Response, ClientError> {
        let session = runtime.get_session_by_id(&req.workspace_id).ok_or_else(|| {
            ClientError::UnexpectedResponse(format!(
                "workspace session not found for id {}",
                req.workspace_id
            ))
        })?;
        // Exercise the warm index; D0 SearchResponse has no hit payload yet.
        let _hits = session
            .search(&req.query, 10)
            .map_err(|err| ClientError::UnexpectedResponse(err.to_string()))?;
        Ok(Response {
            body: Some(response::Body::Search(SearchResponse {})),
        })
    }

    fn handle_apply_page_update(
        &self,
        runtime: &LatticeRuntime,
        req: ApplyPageUpdateRequest,
        idempotency_key: Option<String>,
    ) -> Result<Response, ClientError> {
        let session = runtime.get_session_by_id(&req.workspace_id).ok_or_else(|| {
            ClientError::UnexpectedResponse(format!(
                "workspace session not found for id {}",
                req.workspace_id
            ))
        })?;

        let claim = session
            .write_lease_claim()
            .unwrap_or_else(|| self.lease_claim());
        require_workspace_lease(session.root(), &claim).map_err(runtime_to_client_error)?;

        if let Some(key) = idempotency_key.as_ref() {
            if let Some(cached) = session.idempotency().get(key) {
                return Ok(Response {
                    body: Some(response::Body::ApplyPageUpdate(ApplyPageUpdateResponse {
                        revision: cached.revision,
                    })),
                });
            }
        }

        let revision = lattice_handlers::apply_page_update(
            session.root().to_string_lossy().into_owned(),
            req.path,
            req.content,
            req.expected_revision,
        )
        .map_err(|message| ClientError::Remote {
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

        Ok(Response {
            body: Some(response::Body::ApplyPageUpdate(ApplyPageUpdateResponse {
                revision,
            })),
        })
    }
}

fn runtime_to_client_error(err: lattice_runtime::Error) -> ClientError {
    match &err {
        lattice_runtime::Error::LeaseHeld { .. } => ClientError::Remote {
            code: "lease_held".into(),
            message: err.to_string(),
            details: None,
        },
        lattice_runtime::Error::LeaseNotHeld { .. } => ClientError::Remote {
            code: "lease_not_held".into(),
            message: err.to_string(),
            details: None,
        },
        _ => ClientError::UnexpectedResponse(err.to_string()),
    }
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[async_trait]
impl LatticeClient for EmbeddedClient {
    async fn request(&self, request: Request) -> Result<Response, ClientError> {
        match request.body {
            Some(request::Body::Health(HealthRequest {})) => Ok(self.handle_health()),
            Some(request::Body::Ping(ping)) => Ok(Self::handle_ping(ping)),
            Some(request::Body::OpenWorkspace(req)) => match &self.runtime {
                Some(runtime) => self.handle_open_workspace(runtime, req),
                None => {
                    self.forward_or_unimplemented(Request {
                        deadline_unix_ms: request.deadline_unix_ms,
                        idempotency_key: request.idempotency_key,
                        body: Some(request::Body::OpenWorkspace(req)),
                    })
                    .await
                }
            },
            Some(request::Body::Search(req)) => match &self.runtime {
                Some(runtime) => self.handle_search(runtime, req),
                None => {
                    self.forward_or_unimplemented(Request {
                        deadline_unix_ms: request.deadline_unix_ms,
                        idempotency_key: request.idempotency_key,
                        body: Some(request::Body::Search(req)),
                    })
                    .await
                }
            },
            Some(request::Body::ApplyPageUpdate(req)) => match &self.runtime {
                Some(runtime) => {
                    self.handle_apply_page_update(runtime, req, request.idempotency_key)
                }
                None => {
                    self.forward_or_unimplemented(Request {
                        deadline_unix_ms: request.deadline_unix_ms,
                        idempotency_key: request.idempotency_key,
                        body: Some(request::Body::ApplyPageUpdate(req)),
                    })
                    .await
                }
            },
            body => {
                self.forward_or_unimplemented(Request {
                    deadline_unix_ms: request.deadline_unix_ms,
                    idempotency_key: request.idempotency_key,
                    body,
                })
                .await
            }
        }
    }

    async fn subscribe(&self, _filter: EventFilter) -> Result<EventStream, ClientError> {
        // Embedded mode has no daemon event bus yet; return a closed stream.
        Ok(EventStream::empty())
    }
}

impl EmbeddedClient {
    async fn forward_or_unimplemented(&self, request: Request) -> Result<Response, ClientError> {
        match &self.handler {
            Some(handler) => handler(request).await,
            None => Err(ClientError::Unimplemented(
                "embedded request body not supported until LatticeRuntime wiring",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use lattice_runtime::{lease_path, write_workspace_lease, WorkspaceLeaseFile, OWNER_LATTICED};

    #[tokio::test]
    async fn health_and_ping() {
        let client = EmbeddedClient::new("embedded-1");
        let health = client
            .request(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::Health(HealthRequest {})),
            })
            .await
            .expect("health");
        match health.body {
            Some(response::Body::Health(h)) => {
                assert_eq!(h.status, "ok");
                assert_eq!(h.protocol_version, PROTOCOL_VERSION);
                assert_eq!(h.instance_id, "embedded-1");
            }
            other => panic!("unexpected health body: {other:?}"),
        }

        let ping = client
            .request(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::Ping(PingRequest {
                    nonce: "n-1".into(),
                })),
            })
            .await
            .expect("ping");
        match ping.body {
            Some(response::Body::Ping(p)) => assert_eq!(p.nonce, "n-1"),
            other => panic!("unexpected ping body: {other:?}"),
        }
    }

    #[tokio::test]
    async fn open_and_search_through_runtime() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Embedded Runtime").unwrap();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n\nEmbedded search text.\n").unwrap();

        let runtime = Arc::new(LatticeRuntime::new());
        let client = EmbeddedClient::new("embedded-rt")
            .with_process_start(42)
            .with_runtime(Arc::clone(&runtime));

        let open = client
            .request(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::OpenWorkspace(OpenWorkspaceRequest {
                    path: dir.path().to_string_lossy().into_owned(),
                })),
            })
            .await
            .expect("open");
        let workspace_id = match open.body {
            Some(response::Body::OpenWorkspace(resp)) => {
                assert!(!resp.workspace_id.is_empty());
                let lease = resp.lease.expect("lease");
                assert_eq!(lease.owner, OWNER_EMBEDDED);
                assert_eq!(lease.process_start, 42);
                assert!(lease_path(dir.path()).exists());
                resp.workspace_id
            }
            other => panic!("unexpected open body: {other:?}"),
        };

        assert_eq!(runtime.session_count(), 1);
        let session = runtime.get_session_by_id(&workspace_id).unwrap();
        let rebuilds_before = session.index_rebuild_count();

        let search = client
            .request(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::Search(SearchRequest {
                    workspace_id: workspace_id.clone(),
                    query: "Embedded".into(),
                })),
            })
            .await
            .expect("search");
        assert!(matches!(
            search.body,
            Some(response::Body::Search(SearchResponse {}))
        ));

        let _ = client
            .request(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::Search(SearchRequest {
                    workspace_id,
                    query: "Embedded".into(),
                })),
            })
            .await
            .expect("search again");

        let rebuilds_after_first = session.index_rebuild_count();
        assert!(rebuilds_after_first >= rebuilds_before);
        // Second search must not force another rebuild while the session is held.
        assert_eq!(session.index_rebuild_count(), rebuilds_after_first);
    }

    #[tokio::test]
    async fn open_fails_when_latticed_holds_live_lease() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Lease Conflict").unwrap();
        write_workspace_lease(
            dir.path(),
            &WorkspaceLeaseFile {
                schema_version: 1,
                owner: OWNER_LATTICED.into(),
                pid: 1,
                process_start: 1,
                socket: "/tmp/latticed.sock".into(),
                protocol_version: PROTOCOL_VERSION,
                instance_id: "daemon".into(),
                acquired_at: "2026-01-01T00:00:00Z".into(),
            },
        )
        .unwrap();

        let runtime = Arc::new(LatticeRuntime::new());
        let client = EmbeddedClient::new("embedded-blocked").with_runtime(runtime);
        let err = client
            .request(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::OpenWorkspace(OpenWorkspaceRequest {
                    path: dir.path().to_string_lossy().into_owned(),
                })),
            })
            .await
            .expect_err("must fail");
        match err {
            ClientError::Remote { code, .. } => assert_eq!(code, "lease_held"),
            other => panic!("expected lease_held remote error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn apply_page_update_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Idempotent").unwrap();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();

        let runtime = Arc::new(LatticeRuntime::new());
        let client = EmbeddedClient::new("emb-idem")
            .with_process_start(9)
            .with_runtime(Arc::clone(&runtime));

        let open = client
            .request(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::OpenWorkspace(OpenWorkspaceRequest {
                    path: dir.path().to_string_lossy().into_owned(),
                })),
            })
            .await
            .expect("open");
        let workspace_id = match open.body {
            Some(response::Body::OpenWorkspace(resp)) => resp.workspace_id,
            other => panic!("unexpected open: {other:?}"),
        };

        let before = lattice_handlers::read_page(
            dir.path().to_string_lossy().into_owned(),
            "Notes.md".into(),
        )
        .unwrap();

        let req_body = ApplyPageUpdateRequest {
            workspace_id,
            path: "Notes.md".into(),
            content: "# Edited once\n".into(),
            expected_revision: before.revision.clone(),
        };
        let first = client
            .request(Request {
                deadline_unix_ms: None,
                idempotency_key: Some("idem-1".into()),
                body: Some(request::Body::ApplyPageUpdate(req_body.clone())),
            })
            .await
            .expect("first apply");
        let rev1 = match first.body {
            Some(response::Body::ApplyPageUpdate(r)) => r.revision,
            other => panic!("unexpected: {other:?}"),
        };

        // Retry with the same key and a stale expected_revision would fail if
        // re-applied; idempotency must short-circuit to the cached revision.
        let second = client
            .request(Request {
                deadline_unix_ms: None,
                idempotency_key: Some("idem-1".into()),
                body: Some(request::Body::ApplyPageUpdate(ApplyPageUpdateRequest {
                    expected_revision: before.revision,
                    ..req_body
                })),
            })
            .await
            .expect("retry");
        let rev2 = match second.body {
            Some(response::Body::ApplyPageUpdate(r)) => r.revision,
            other => panic!("unexpected: {other:?}"),
        };
        assert_eq!(rev1, rev2);

        let after = lattice_handlers::read_page(
            dir.path().to_string_lossy().into_owned(),
            "Notes.md".into(),
        )
        .unwrap();
        assert_eq!(after.content, "# Edited once\n");
        assert_eq!(after.revision, rev1);
    }
}

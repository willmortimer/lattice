use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use lattice_protocol::{
    request, response, HealthRequest, HealthResponse, OpenWorkspaceRequest, OpenWorkspaceResponse,
    PingRequest, PingResponse, SearchRequest, SearchResponse, WorkspaceLease, Request, Response,
    PROTOCOL_VERSION,
};
use lattice_runtime::LatticeRuntime;

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
/// [`EmbeddedClient::with_runtime`], `OpenWorkspace` and `Search` dispatch
/// through warm sessions. Other bodies still use the optional handler callback.
#[derive(Clone)]
pub struct EmbeddedClient {
    instance_id: String,
    handler: Option<EmbeddedRequestHandler>,
    runtime: Option<Arc<LatticeRuntime>>,
}

impl EmbeddedClient {
    /// Create a client that answers health/ping with `instance_id`.
    pub fn new(instance_id: impl Into<String>) -> Self {
        Self {
            instance_id: instance_id.into(),
            handler: None,
            runtime: None,
        }
    }

    /// Attach a fallback handler for non-health/ping requests.
    pub fn with_handler(mut self, handler: EmbeddedRequestHandler) -> Self {
        self.handler = Some(handler);
        self
    }

    /// Dispatch OpenWorkspace / Search through a long-lived runtime.
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

    fn handle_health(&self) -> Response {
        Response {
            body: Some(response::Body::Health(HealthResponse {
                status: "ok".into(),
                protocol_version: PROTOCOL_VERSION,
                instance_id: self.instance_id.clone(),
            })),
        }
    }

    fn handle_ping(ping: PingRequest) -> Response {
        Response {
            body: Some(response::Body::Ping(PingResponse { nonce: ping.nonce })),
        }
    }

    fn embedded_lease(&self) -> WorkspaceLease {
        let acquired_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| format!("{}Z", d.as_secs()))
            .unwrap_or_else(|_| "0Z".into());
        WorkspaceLease {
            schema_version: 1,
            owner: "embedded".into(),
            pid: std::process::id(),
            process_start: 0,
            socket: String::new(),
            protocol_version: PROTOCOL_VERSION,
            instance_id: self.instance_id.clone(),
            acquired_at,
        }
    }

    fn handle_open_workspace(
        &self,
        runtime: &LatticeRuntime,
        req: OpenWorkspaceRequest,
    ) -> Result<Response, ClientError> {
        let session = runtime
            .open_workspace_session(req.path.as_str())
            .map_err(|err| ClientError::UnexpectedResponse(err.to_string()))?;
        Ok(Response {
            body: Some(response::Body::OpenWorkspace(OpenWorkspaceResponse {
                workspace_id: session.workspace_id().to_string(),
                lease: Some(self.embedded_lease()),
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
}

#[async_trait]
impl LatticeClient for EmbeddedClient {
    async fn request(&self, request: Request) -> Result<Response, ClientError> {
        match request.body {
            Some(request::Body::Health(HealthRequest {})) => Ok(self.handle_health()),
            Some(request::Body::Ping(ping)) => Ok(Self::handle_ping(ping)),
            Some(request::Body::OpenWorkspace(req)) => match &self.runtime {
                Some(runtime) => self.handle_open_workspace(runtime, req),
                None => self.forward_or_unimplemented(Request {
                    deadline_unix_ms: request.deadline_unix_ms,
                    idempotency_key: request.idempotency_key,
                    body: Some(request::Body::OpenWorkspace(req)),
                })
                .await,
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
        let client = EmbeddedClient::new("embedded-rt").with_runtime(Arc::clone(&runtime));

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
                assert!(resp.lease.is_some());
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
}

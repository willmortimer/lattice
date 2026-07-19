use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use lattice_protocol::{
    request, response, HealthRequest, HealthResponse, PingRequest, PingResponse, Request, Response,
    PROTOCOL_VERSION,
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

/// In-process [`LatticeClient`] stub for D0/D1 before `LatticeRuntime` exists.
///
/// Handles `Health` and `Ping` locally. Other bodies are forwarded to an
/// optional handler callback so tests can inject doubles.
#[derive(Clone)]
pub struct EmbeddedClient {
    instance_id: String,
    handler: Option<EmbeddedRequestHandler>,
}

impl EmbeddedClient {
    /// Create a client that answers health/ping with `instance_id`.
    pub fn new(instance_id: impl Into<String>) -> Self {
        Self {
            instance_id: instance_id.into(),
            handler: None,
        }
    }

    /// Attach a fallback handler for non-health/ping requests.
    pub fn with_handler(mut self, handler: EmbeddedRequestHandler) -> Self {
        self.handler = Some(handler);
        self
    }

    /// Instance id reported by health responses.
    pub fn instance_id(&self) -> &str {
        &self.instance_id
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
}

#[async_trait]
impl LatticeClient for EmbeddedClient {
    async fn request(&self, request: Request) -> Result<Response, ClientError> {
        match request.body {
            Some(request::Body::Health(HealthRequest {})) => Ok(self.handle_health()),
            Some(request::Body::Ping(ping)) => Ok(Self::handle_ping(ping)),
            body => {
                let forwarded = Request {
                    deadline_unix_ms: request.deadline_unix_ms,
                    idempotency_key: request.idempotency_key,
                    body,
                };
                match &self.handler {
                    Some(handler) => handler(forwarded).await,
                    None => Err(ClientError::Unimplemented(
                        "embedded request body not supported until LatticeRuntime wiring",
                    )),
                }
            }
        }
    }

    async fn subscribe(&self, _filter: EventFilter) -> Result<EventStream, ClientError> {
        // Embedded mode has no daemon event bus yet; return a closed stream.
        Ok(EventStream::empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

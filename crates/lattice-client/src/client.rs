use async_trait::async_trait;
use lattice_protocol::{Request, Response};

use crate::error::ClientError;
use crate::events::{EventFilter, EventStream};

/// Shared frontend/runtime contract for embedded and daemon execution modes.
///
/// Adapters should call this trait instead of domain handlers directly so
/// embedded and `latticed` paths stay interchangeable (ADR 0041).
#[async_trait]
pub trait LatticeClient: Send + Sync {
    /// Send one request and wait for the matching response.
    async fn request(&self, request: Request) -> Result<Response, ClientError>;

    /// Subscribe to sequenced daemon events matching `filter`.
    async fn subscribe(&self, filter: EventFilter) -> Result<EventStream, ClientError>;
}

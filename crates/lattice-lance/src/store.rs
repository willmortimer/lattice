use async_trait::async_trait;

use crate::error::{LanceError, Result};
use crate::types::{
    Commit, DatasetRef, DatasetSnapshot, SearchElementBatch, SearchElementRow, SearchRequest,
    SearchResults,
};

/// Async contract for embedded multimodal derived-data storage.
///
/// Implementations convert between Lattice-owned row types and Lance/Arrow
/// internally so callers stay Arrow-version agnostic.
#[async_trait]
pub trait MultimodalStore: Send + Sync {
    async fn append(&self, dataset: &DatasetRef, batch: SearchElementBatch) -> Result<Commit>;

    async fn search(&self, request: SearchRequest) -> Result<SearchResults>;

    async fn scan(&self, dataset: &DatasetRef) -> Result<Vec<SearchElementRow>>;

    async fn snapshot(&self, dataset: &DatasetRef) -> Result<DatasetSnapshot>;

    async fn remove(&self, dataset: &DatasetRef, element_ids: &[String]) -> Result<Commit>;
}

/// Placeholder store used until `EmbeddedLanceStore` lands in T2.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnsupportedStore;

#[async_trait]
impl MultimodalStore for UnsupportedStore {
    async fn append(&self, _dataset: &DatasetRef, _batch: SearchElementBatch) -> Result<Commit> {
        Err(LanceError::not_implemented("append"))
    }

    async fn search(&self, _request: SearchRequest) -> Result<SearchResults> {
        Err(LanceError::not_implemented("search"))
    }

    async fn scan(&self, _dataset: &DatasetRef) -> Result<Vec<SearchElementRow>> {
        Err(LanceError::not_implemented("scan"))
    }

    async fn snapshot(&self, _dataset: &DatasetRef) -> Result<DatasetSnapshot> {
        Err(LanceError::not_implemented("snapshot"))
    }

    async fn remove(&self, _dataset: &DatasetRef, _element_ids: &[String]) -> Result<Commit> {
        Err(LanceError::not_implemented("remove"))
    }
}

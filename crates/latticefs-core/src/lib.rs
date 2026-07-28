//! LatticeFS core types and workspace resource registry.
//!
//! Stable resource identity, authority/materialization metadata, and a
//! persisted path registry live here so the public client can reason about
//! cloud-backed objects without depending on private cloud crates.

mod cloud;
mod error;
mod registry;
mod stat;
mod types;

pub use cloud::{
    roundtrip_verify_blob, CloudBlobClient, InMemoryCloudBlobClient,
};
pub use error::{Error, Result};
pub use registry::{NamespaceRegistry, OPERATIONAL_DIR, REGISTRY_FILENAME};
pub use stat::{materialize_to_cloud, resource_stat, resource_stat_or_register};
pub use types::{
    AuthorityMode, ContentHash, MaterializationState, NamespaceEntry, ResourceId, ResourceStat,
    ResourceVersionId,
};

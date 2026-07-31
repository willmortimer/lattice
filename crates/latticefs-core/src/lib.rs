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
    fetch_cloud_blob, roundtrip_verify_blob, CloudBlobClient, InMemoryCloudBlobClient,
};
pub use error::{Error, Result};
pub use registry::{NamespaceRegistry, OPERATIONAL_DIR, REGISTRY_FILENAME};
pub use stat::{
    attach_accept_hydration_lineage, materialize_to_cloud, open_cloud_authoritative_bytes,
    resource_stat, resource_stat_or_register,
};
pub use types::{
    AuthorityMode, ContentHash, HydrationInputDigest, MaterializationState, NamespaceEntry,
    ResourceId, ResourceStat, ResourceVersionId,
};

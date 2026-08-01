//! Process-wide KernelFS export lease registry (KF-LEASE).
//!
//! Production materialize/export paths register runs here, hold an [`ExportLease`]
//! while the export is live, and GC the export root after the last release.
//!
//! [`kernelfs_allow_replace`] is an explicit escape hatch for tests and dogfood
//! wipe flows (`LATTICE_KERNELFS_ALLOW_REPLACE=1`).

use std::sync::LazyLock;

use kernelfs::{ExportLease, ExportLeaseRegistry, GcOutcome, LeaseError};

/// Env var: when `1`/`true`, permit wiping an existing run export (test/dogfood only).
pub const ALLOW_REPLACE_ENV: &str = "LATTICE_KERNELFS_ALLOW_REPLACE";

static EXPORT_LEASE_REGISTRY: LazyLock<ExportLeaseRegistry> =
    LazyLock::new(ExportLeaseRegistry::new);

/// Shared in-process export lease registry for agentd materialize/export paths.
pub fn export_lease_registry() -> &'static ExportLeaseRegistry {
    &EXPORT_LEASE_REGISTRY
}

/// Whether materialize/export may replace an existing run tree (test/dogfood only).
pub fn kernelfs_allow_replace() -> bool {
    matches!(
        std::env::var(ALLOW_REPLACE_ENV).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    )
}

/// RAII export lease; GCs the registered export root on drop.
#[derive(Debug)]
pub struct HeldExportLease {
    registry: &'static ExportLeaseRegistry,
    lease: Option<ExportLease>,
    run_id: String,
}

impl HeldExportLease {
    pub fn hold(run_id: &str) -> Result<Self, LeaseError> {
        let registry = export_lease_registry();
        let lease = registry.hold(run_id)?;
        Ok(Self {
            registry,
            lease: Some(lease),
            run_id: run_id.to_string(),
        })
    }
}

impl Drop for HeldExportLease {
    fn drop(&mut self) {
        drop(self.lease.take());
        match self.registry.gc_export_root(&self.run_id) {
            Ok(GcOutcome::Removed | GcOutcome::Skipped) => {}
            Ok(GcOutcome::Leased) => {
                tracing::debug!(
                    target: "lattice_agentd",
                    run_id = %self.run_id,
                    "export lease GC skipped: refcount still held"
                );
            }
            Err(err) => {
                tracing::warn!(
                    target: "lattice_agentd",
                    run_id = %self.run_id,
                    error = %err,
                    "export lease GC failed"
                );
            }
        }
    }
}

/// Whether materialize/export may replace an existing run tree (test/dogfood only).
pub fn materialize_allow_replace(default: bool) -> bool {
    default || kernelfs_allow_replace()
}

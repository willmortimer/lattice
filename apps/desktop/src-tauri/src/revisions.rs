use std::path::Path;
use std::time::Duration;

use lattice_commands::{
    CommandEngine, ConflictEnvelope, HistoryCleanupCandidate, HistoryCleanupReport,
    HistoryRetentionPolicy, ResourceRevisionDetail, ResourceRevisionSummary, RevisionDiff,
    RevisionPayload, RevisionSource,
};
use serde::Serialize;

use crate::commands::command_error_to_string;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRevisionSummaryWire {
    revision_id: String,
    resource_path: String,
    transaction_id: Option<String>,
    summary: Option<String>,
    created_at: i64,
    parent_revision: Option<String>,
    before_hash: Option<String>,
    after_hash: Option<String>,
    before_len: Option<u64>,
    after_len: Option<u64>,
    source: RevisionSource,
    prior_available: bool,
    pinned: bool,
    current_baseline: bool,
    unresolved_conflict: bool,
}

impl From<ResourceRevisionSummary> for ResourceRevisionSummaryWire {
    fn from(value: ResourceRevisionSummary) -> Self {
        Self {
            revision_id: value.revision_id,
            resource_path: value.resource_path.to_string_lossy().replace('\\', "/"),
            transaction_id: value.transaction_id,
            summary: value.summary,
            created_at: value.created_at,
            parent_revision: value.parent_revision,
            before_hash: value.before_hash,
            after_hash: value.after_hash,
            before_len: value.before_len,
            after_len: value.after_len,
            source: value.source,
            prior_available: value.prior_available,
            pinned: value.pinned,
            current_baseline: value.current_baseline,
            unresolved_conflict: value.unresolved_conflict,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionPayloadWire {
    hash: String,
    len: u64,
    is_text: bool,
    /// Historical text crosses IPC as a string, never a JSON byte array.
    text: Option<String>,
}

impl From<RevisionPayload> for RevisionPayloadWire {
    fn from(value: RevisionPayload) -> Self {
        let text = value
            .is_text
            .then(|| value.bytes.and_then(|bytes| String::from_utf8(bytes).ok()))
            .flatten();
        Self {
            hash: value.hash,
            len: value.len,
            is_text: value.is_text,
            text,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionDiffWire {
    is_binary: bool,
    unified: Option<String>,
    added_lines: u64,
    removed_lines: u64,
    base_len: Option<u64>,
    local_len: Option<u64>,
}

impl From<RevisionDiff> for RevisionDiffWire {
    fn from(value: RevisionDiff) -> Self {
        Self {
            is_binary: value.is_binary,
            unified: value.unified,
            added_lines: value.added_lines,
            removed_lines: value.removed_lines,
            base_len: value.base_len,
            local_len: value.local_len,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictEnvelopeWire {
    resource: String,
    base_revision: Option<String>,
    incompatible_descendants: Vec<String>,
    affected_units: Vec<String>,
    failure_reason: String,
    resolution_options: Vec<String>,
}

impl From<ConflictEnvelope> for ConflictEnvelopeWire {
    fn from(value: ConflictEnvelope) -> Self {
        Self {
            resource: value.resource.to_string_lossy().replace('\\', "/"),
            base_revision: value.base_revision,
            incompatible_descendants: value.incompatible_descendants,
            affected_units: value.affected_units,
            failure_reason: value.failure_reason,
            resolution_options: value.resolution_options,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRevisionDetailWire {
    summary: ResourceRevisionSummaryWire,
    base: Option<RevisionPayloadWire>,
    local: Option<RevisionPayloadWire>,
    incoming: Option<RevisionPayloadWire>,
    diff: RevisionDiffWire,
    conflict: Option<ConflictEnvelopeWire>,
}

impl From<ResourceRevisionDetail> for ResourceRevisionDetailWire {
    fn from(value: ResourceRevisionDetail) -> Self {
        Self {
            summary: value.summary.into(),
            base: value.base.map(Into::into),
            local: value.local.map(Into::into),
            incoming: value.incoming.map(Into::into),
            diff: value.diff.into(),
            conflict: value.conflict.map(Into::into),
        }
    }
}

/// List bounded per-resource revision metadata for the inspector.
#[tauri::command]
pub fn list_resource_revisions(
    root: String,
    rel_path: String,
    limit: usize,
) -> Result<Vec<ResourceRevisionSummaryWire>, String> {
    let engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;
    engine
        .list_resource_revisions(Path::new(&rel_path), limit.min(100))
        .map(|revisions| revisions.into_iter().map(Into::into).collect())
        .map_err(command_error_to_string)
}

/// Load one revision's base/local/incoming metadata and text diff. Binary
/// payloads are returned as metadata-only by the command core.
#[tauri::command]
pub fn get_resource_revision(
    root: String,
    rel_path: String,
    revision_id: String,
) -> Result<Option<ResourceRevisionDetailWire>, String> {
    let engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;
    engine
        .resource_revision_detail(Path::new(&rel_path), &revision_id)
        .map(|revision| revision.map(Into::into))
        .map_err(command_error_to_string)
}

/// Revert a resource as a fresh guarded semantic revision. The caller must
/// pass the current content revision it displayed to the user.
#[tauri::command]
pub fn revert_resource_revision(
    root: String,
    rel_path: String,
    revision_id: String,
    expected_current_revision: String,
) -> Result<String, String> {
    let mut engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;
    let receipt = engine
        .revert_resource_revision(
            Path::new(&rel_path),
            &revision_id,
            &expected_current_revision,
        )
        .map_err(command_error_to_string)?;
    receipt
        .outcomes
        .first()
        .and_then(|outcome| outcome.resulting_revision.clone())
        .ok_or_else(|| "revision revert did not produce a resulting revision".into())
}

/// Run retention cleanup. Optional age/size override the 180d / 1 GiB defaults.
/// Destructive cleanup always reports the first notice/dry-run boundary before
/// deleting any object when `requires_confirmation` is returned.
#[tauri::command]
pub fn cleanup_history(
    root: String,
    dry_run: bool,
    max_age_days: Option<u64>,
    max_bytes: Option<u64>,
) -> Result<HistoryCleanupReportWire, String> {
    let defaults = HistoryRetentionPolicy::default();
    let policy = HistoryRetentionPolicy {
        max_age: max_age_days
            .map(|days| Duration::from_secs(days.saturating_mul(24 * 60 * 60)))
            .unwrap_or(defaults.max_age),
        max_bytes: max_bytes.unwrap_or(defaults.max_bytes),
    };
    let engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;
    engine
        .cleanup_history(policy, dry_run)
        .map(HistoryCleanupReportWire::from)
        .map_err(command_error_to_string)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCleanupCandidateWire {
    object_hash: String,
    size: u64,
    created_at: i64,
}

impl From<HistoryCleanupCandidate> for HistoryCleanupCandidateWire {
    fn from(value: HistoryCleanupCandidate) -> Self {
        Self {
            object_hash: value.object_hash,
            size: value.size,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCleanupReportWire {
    dry_run: bool,
    requires_confirmation: bool,
    notice: Option<String>,
    total_bytes: u64,
    reclaimable_bytes: u64,
    candidates: Vec<HistoryCleanupCandidateWire>,
    deleted_objects: u64,
    deleted_bytes: u64,
}

impl From<HistoryCleanupReport> for HistoryCleanupReportWire {
    fn from(value: HistoryCleanupReport) -> Self {
        Self {
            dry_run: value.dry_run,
            requires_confirmation: value.requires_confirmation,
            notice: value.notice,
            total_bytes: value.total_bytes,
            reclaimable_bytes: value.reclaimable_bytes,
            candidates: value
                .candidates
                .into_iter()
                .map(HistoryCleanupCandidateWire::from)
                .collect(),
            deleted_objects: value.deleted_objects,
            deleted_bytes: value.deleted_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_payloads_serialize_as_text_instead_of_json_byte_arrays() {
        let wire = RevisionPayloadWire::from(RevisionPayload {
            hash: "abc".into(),
            len: 5,
            is_text: true,
            bytes: Some(b"hello".to_vec()),
        });
        let value = serde_json::to_value(wire).unwrap();
        assert_eq!(value["text"], "hello");
        assert!(value.get("bytes").is_none());
    }

    #[test]
    fn binary_payloads_remain_metadata_only() {
        let wire = RevisionPayloadWire::from(RevisionPayload {
            hash: "def".into(),
            len: 3,
            is_text: false,
            bytes: None,
        });
        let value = serde_json::to_value(wire).unwrap();
        assert!(value["text"].is_null());
        assert_eq!(value["len"], 3);
    }

    #[test]
    fn cleanup_report_wire_uses_camel_case() {
        let wire = HistoryCleanupReportWire::from(HistoryCleanupReport {
            dry_run: true,
            requires_confirmation: true,
            notice: Some("preview".into()),
            total_bytes: 10,
            reclaimable_bytes: 4,
            candidates: vec![HistoryCleanupCandidate {
                object_hash: "abc".into(),
                size: 4,
                created_at: 1,
            }],
            deleted_objects: 0,
            deleted_bytes: 0,
        });
        let value = serde_json::to_value(wire).unwrap();
        assert_eq!(value["dryRun"], true);
        assert_eq!(value["requiresConfirmation"], true);
        assert_eq!(value["reclaimableBytes"], 4);
        assert_eq!(value["candidates"][0]["objectHash"], "abc");
    }
}

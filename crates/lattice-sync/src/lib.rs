//! Pure sync planner: classify local registry entries against cloud sync-heads.
//!
//! # Boundary
//!
//! - Planner (`plan`) has no network I/O.
//! - [`executor`] applies planner output via `lattice-cloud-client`.
//! - Capture, presence, and app lock (desktop) are **out of scope**. Do not
//!   couple this crate to `capture/**`, `presence.rs`, or `app_lock.rs`.
//!
//! # Hash normalization
//!
//! Cloud sync-heads (S2) return lowercase SHA-256 **hex** without a prefix.
//! Local LatticeFS `ContentHash` values use `sha256:<hex>`.
//! [`normalize_content_hash`] strips an optional `sha256:` prefix and lowercases
//! so both forms compare equal when the digest matches.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

mod executor;
mod sync_state;

pub use executor::{
    execute_plan_entry, local_snapshot_from_workspace, resolve_conflict, run_workspace_sync,
    ConflictResolution, ExecuteOutcome, ExecuteResult, ExecutorError, SyncRunReport,
};
pub use sync_state::{SyncState, SyncStateError, SYNC_STATE_FILENAME};

/// Per-resource sync classification produced by [`plan`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    /// Local and cloud content hashes match.
    InSync,
    /// Local advanced past the last known sync head; cloud still matches that head.
    ///
    /// Only emitted when [`LocalSnapshotEntry::last_synced_hash`] is present and
    /// equals the cloud head. Without `last_synced_hash`, a hash mismatch is
    /// [`Conflicted`](Self::Conflicted) (v1).
    Dirty,
    /// Cloud has a head; local registry has no entry (needs pull).
    MissingLocal,
    /// Local has content; cloud has no sync-head (needs push).
    MissingCloud,
    /// Both sides present with different hashes, and direction is ambiguous.
    Conflicted,
}

/// One local registry snapshot row fed into the planner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalSnapshotEntry {
    pub resource_id: String,
    /// Digest in either bare hex or `sha256:<hex>` form.
    pub content_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Optional last-known synced digest (same hash forms as `content_hash`).
    ///
    /// When set and equal to the cloud head while local differs, the planner
    /// emits [`SyncStatus::Dirty`] instead of [`SyncStatus::Conflicted`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_synced_hash: Option<String>,
}

/// Cloud sync-head row matching S2 `GET .../sync-heads` JSON shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncHead {
    pub resource_id: String,
    /// Lowercase SHA-256 hex (S2), or `sha256:<hex>` (accepted and normalized).
    pub content_hash: String,
    /// Unix seconds when this head last advanced.
    pub updated_at: i64,
}

/// Planner output for one resource id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanEntry {
    pub resource_id: String,
    pub status: SyncStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Normalized local digest when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_hash: Option<String>,
    /// Normalized cloud digest when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_updated_at: Option<i64>,
}

/// Normalize a content hash for comparison.
///
/// Accepts bare hex or `sha256:<hex>` (case-insensitive prefix). Returns
/// lowercase hex without prefix. Empty / whitespace-only input stays empty.
pub fn normalize_content_hash(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let without_prefix = trimmed
        .strip_prefix("sha256:")
        .or_else(|| trimmed.strip_prefix("SHA256:"))
        .unwrap_or(trimmed);
    without_prefix.trim().to_ascii_lowercase()
}

/// Classify every resource appearing in `local` and/or `cloud`.
///
/// Rules (v1):
/// - Same normalized hash both sides → [`SyncStatus::InSync`]
/// - Local only → [`SyncStatus::MissingCloud`]
/// - Cloud only → [`SyncStatus::MissingLocal`]
/// - Both present, hashes differ, `last_synced_hash` equals cloud → [`SyncStatus::Dirty`]
/// - Both present, hashes differ otherwise → [`SyncStatus::Conflicted`]
///
/// Duplicate `resource_id` values: last entry wins within each input slice.
/// Output is sorted by `resource_id` (BTreeMap order).
pub fn plan(local: &[LocalSnapshotEntry], cloud: &[SyncHead]) -> Vec<PlanEntry> {
    #[derive(Default)]
    struct Sides {
        local_hash: Option<String>,
        path: Option<String>,
        last_synced_hash: Option<String>,
        cloud_hash: Option<String>,
        cloud_updated_at: Option<i64>,
    }

    let mut by_id: BTreeMap<String, Sides> = BTreeMap::new();

    for entry in local {
        let sides = by_id.entry(entry.resource_id.clone()).or_default();
        sides.local_hash = Some(normalize_content_hash(&entry.content_hash));
        sides.path = entry.path.clone();
        sides.last_synced_hash = entry
            .last_synced_hash
            .as_deref()
            .map(normalize_content_hash);
    }

    for head in cloud {
        let sides = by_id.entry(head.resource_id.clone()).or_default();
        sides.cloud_hash = Some(normalize_content_hash(&head.content_hash));
        sides.cloud_updated_at = Some(head.updated_at);
    }

    by_id
        .into_iter()
        .map(|(resource_id, sides)| {
            let status = classify(
                sides.local_hash.as_deref(),
                sides.cloud_hash.as_deref(),
                sides.last_synced_hash.as_deref(),
            );
            PlanEntry {
                resource_id,
                status,
                path: sides.path,
                local_hash: sides.local_hash,
                cloud_hash: sides.cloud_hash,
                cloud_updated_at: sides.cloud_updated_at,
            }
        })
        .collect()
}

fn classify(
    local_hash: Option<&str>,
    cloud_hash: Option<&str>,
    last_synced_hash: Option<&str>,
) -> SyncStatus {
    match (local_hash, cloud_hash) {
        (Some(local), Some(cloud)) if local == cloud => SyncStatus::InSync,
        (Some(_), None) => SyncStatus::MissingCloud,
        (None, Some(_)) => SyncStatus::MissingLocal,
        (Some(local), Some(cloud)) => {
            // Hash mismatch: Dirty only when we know cloud still matches the
            // last synced head and local has moved ahead.
            if last_synced_hash == Some(cloud) && local != cloud {
                SyncStatus::Dirty
            } else {
                SyncStatus::Conflicted
            }
        }
        (None, None) => SyncStatus::Conflicted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEX_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HEX_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const HEX_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn local(id: &str, hash: &str) -> LocalSnapshotEntry {
        LocalSnapshotEntry {
            resource_id: id.into(),
            content_hash: hash.into(),
            path: None,
            last_synced_hash: None,
        }
    }

    fn local_with(
        id: &str,
        hash: &str,
        path: Option<&str>,
        last_synced: Option<&str>,
    ) -> LocalSnapshotEntry {
        LocalSnapshotEntry {
            resource_id: id.into(),
            content_hash: hash.into(),
            path: path.map(str::to_owned),
            last_synced_hash: last_synced.map(str::to_owned),
        }
    }

    fn head(id: &str, hash: &str, updated_at: i64) -> SyncHead {
        SyncHead {
            resource_id: id.into(),
            content_hash: hash.into(),
            updated_at,
        }
    }

    fn status_for(plan: &[PlanEntry], id: &str) -> SyncStatus {
        plan.iter()
            .find(|e| e.resource_id == id)
            .unwrap_or_else(|| panic!("missing plan entry for {id}"))
            .status
    }

    #[test]
    fn normalize_strips_sha256_prefix_and_lowercases() {
        assert_eq!(
            normalize_content_hash(&format!("sha256:{HEX_A}")),
            HEX_A
        );
        assert_eq!(
            normalize_content_hash(&format!("SHA256:{}", HEX_A.to_ascii_uppercase())),
            HEX_A
        );
        assert_eq!(normalize_content_hash(HEX_A), HEX_A);
        assert_eq!(normalize_content_hash("  "), "");
    }

    #[test]
    fn table_driven_five_outcomes() {
        // (label, local entries, cloud heads, resource_id, expected)
        let cases: &[(&str, Vec<LocalSnapshotEntry>, Vec<SyncHead>, &str, SyncStatus)] = &[
            (
                "in_sync_equal_hashes",
                vec![local("r-in", &format!("sha256:{HEX_A}"))],
                vec![head("r-in", HEX_A, 100)],
                "r-in",
                SyncStatus::InSync,
            ),
            (
                "missing_cloud_local_only",
                vec![local_with("r-push", HEX_A, Some("docs/a.md"), None)],
                vec![],
                "r-push",
                SyncStatus::MissingCloud,
            ),
            (
                "missing_local_cloud_only",
                vec![],
                vec![head("r-pull", HEX_B, 200)],
                "r-pull",
                SyncStatus::MissingLocal,
            ),
            (
                "conflicted_mismatch_without_last_synced",
                vec![local("r-conflict", &format!("sha256:{HEX_A}"))],
                vec![head("r-conflict", HEX_B, 300)],
                "r-conflict",
                SyncStatus::Conflicted,
            ),
            (
                "dirty_local_advanced_cloud_at_last_synced",
                vec![local_with(
                    "r-dirty",
                    &format!("sha256:{HEX_B}"),
                    Some("notes.md"),
                    Some(HEX_A),
                )],
                vec![head("r-dirty", HEX_A, 400)],
                "r-dirty",
                SyncStatus::Dirty,
            ),
        ];

        for (label, local_entries, cloud_heads, id, expected) in cases {
            let plan = plan(local_entries, cloud_heads);
            assert_eq!(
                status_for(&plan, id),
                *expected,
                "case {label}: expected {expected:?}"
            );
        }
    }

    #[test]
    fn mismatch_with_last_synced_not_matching_cloud_is_conflicted() {
        let plan = plan(
            &[local_with("r", HEX_A, None, Some(HEX_C))],
            &[head("r", HEX_B, 1)],
        );
        assert_eq!(status_for(&plan, "r"), SyncStatus::Conflicted);
    }

    #[test]
    fn mixed_batch_covers_all_statuses_sorted_by_id() {
        let plan = plan(
            &[
                local("a-insync", HEX_A),
                local_with("b-dirty", HEX_B, None, Some(HEX_A)),
                local("c-push", HEX_A),
                local("e-conflict", HEX_A),
            ],
            &[
                head("a-insync", HEX_A, 1),
                head("b-dirty", HEX_A, 2),
                head("d-pull", HEX_C, 3),
                head("e-conflict", HEX_B, 4),
            ],
        );

        let statuses: Vec<(&str, SyncStatus)> = plan
            .iter()
            .map(|e| (e.resource_id.as_str(), e.status))
            .collect();

        assert_eq!(
            statuses,
            vec![
                ("a-insync", SyncStatus::InSync),
                ("b-dirty", SyncStatus::Dirty),
                ("c-push", SyncStatus::MissingCloud),
                ("d-pull", SyncStatus::MissingLocal),
                ("e-conflict", SyncStatus::Conflicted),
            ]
        );
    }

    #[test]
    fn sync_head_deserializes_s2_json_shape() {
        let json = r#"[
            {
                "resource_id": "0195a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
                "content_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                "updated_at": 1753660800
            }
        ]"#;
        let heads: Vec<SyncHead> = serde_json::from_str(json).expect("S2 shape");
        assert_eq!(heads.len(), 1);
        assert_eq!(heads[0].updated_at, 1753660800);
        assert_eq!(
            normalize_content_hash(&heads[0].content_hash),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}

//! Walk `/output` after a run and build a commit plan for Lattice proposal overlays.
//!
//! Lattice wiring (host responsibility):
//! 1. `collect_output_commit_plan` after Wasmtime returns.
//! 2. Inspect [`OutputCommitEntry::kind`] (`text` vs `bytes`); map only what the
//!    host propose API supports (UTF-8 `propose_resource` vs binary blob APIs).
//! 3. `LatticeProposalAdapter::drafts` → one draft per entry (payload retained as bytes).
//! 4. For each draft, call `lattice_commands::propose_helpers::propose_resource`
//!    (or `api_propose_resource` / `create_proposal` via `latticed`).
//! 5. Surface drafts in the existing accept/reject proposal UI — no silent writes.

use std::fs;
use std::io::Read;
use std::path::{Component, Path};

use sha2::{Digest, Sha256};

use crate::manifest::ExecutionManifest;
use crate::materialize::{normalize_guest_path, MaterializeError};

/// How a collected artifact should be interpreted by hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    /// Valid UTF-8 text (typical for `propose_resource`).
    Text,
    /// Opaque bytes; hosts must use a binary-capable propose path.
    Bytes,
}

/// Plan describing proposed workspace writes from a completed run's `/output`
/// and any allowlisted [`Mounts::work_promote_paths`](crate::Mounts::work_promote_paths).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutputCommitPlan {
    pub run_id: String,
    pub proposal_target_prefix: Option<String>,
    pub entries: Vec<OutputCommitEntry>,
}

/// One file produced under `/output` or promoted from `/work`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutputCommitEntry {
    pub relative_path: String,
    pub content: Vec<u8>,
    pub sha256: String,
    /// Classified from payload bytes (UTF-8 → [`ContentKind::Text`]).
    pub kind: ContentKind,
    /// Optional MIME-ish hint for hosts (e.g. `text/plain`, `application/octet-stream`).
    pub content_type_hint: Option<String>,
}

/// Lattice-facing draft before `propose_resource` / `create_proposal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatticeProposalDraft {
    pub summary: String,
    pub resource_path: String,
    pub content: Vec<u8>,
    pub kind: ContentKind,
}

/// Adapter that maps [`OutputCommitPlan`] entries to Lattice proposal drafts.
#[derive(Debug, Clone)]
pub struct LatticeProposalAdapter {
    pub proposal_target_prefix: String,
}

impl LatticeProposalAdapter {
    pub fn new(proposal_target_prefix: impl Into<String>) -> Self {
        Self {
            proposal_target_prefix: normalize_prefix(proposal_target_prefix.into()),
        }
    }

    pub fn from_manifest(manifest: &ExecutionManifest) -> Self {
        let prefix = manifest
            .mounts
            .output_proposal_target
            .clone()
            .unwrap_or_else(|| "output".to_string());
        Self::new(prefix)
    }

    /// Convert a commit plan into workspace-relative proposal drafts.
    pub fn drafts(&self, plan: &OutputCommitPlan) -> Vec<LatticeProposalDraft> {
        lattice_proposal_drafts(plan, &self.proposal_target_prefix)
    }
}

/// Walk `run_root/output` and collect file payloads for proposal bridging,
/// including allowlisted `/work` promotions from the manifest.
pub fn collect_output_commit_plan(
    run_root: &Path,
    manifest: &ExecutionManifest,
) -> Result<OutputCommitPlan, MaterializeError> {
    let output_root = run_root.join("output");
    let mut entries = Vec::new();
    if output_root.is_dir() {
        walk_output(&output_root, &output_root, &mut entries)?;
    }
    collect_work_promotions(run_root, manifest, &mut entries)?;

    Ok(OutputCommitPlan {
        run_id: manifest.run_id.clone(),
        proposal_target_prefix: manifest.mounts.output_proposal_target.clone(),
        entries,
    })
}

/// Classify payload bytes for host propose mapping.
pub fn classify_content(content: &[u8]) -> ContentKind {
    if std::str::from_utf8(content).is_ok() {
        ContentKind::Text
    } else {
        ContentKind::Bytes
    }
}

fn content_type_hint_for(kind: ContentKind, relative_path: &str) -> Option<String> {
    match kind {
        ContentKind::Text => {
            if relative_path.ends_with(".json") {
                Some("application/json".into())
            } else if relative_path.ends_with(".md") {
                Some("text/markdown".into())
            } else {
                Some("text/plain".into())
            }
        }
        ContentKind::Bytes => Some("application/octet-stream".into()),
    }
}

fn push_entry(entries: &mut Vec<OutputCommitEntry>, relative_path: String, content: Vec<u8>) {
    let kind = classify_content(&content);
    let content_type_hint = content_type_hint_for(kind, &relative_path);
    let sha256 = hex::encode(Sha256::digest(&content));
    entries.push(OutputCommitEntry {
        relative_path,
        content,
        sha256,
        kind,
        content_type_hint,
    });
}

fn collect_work_promotions(
    run_root: &Path,
    manifest: &ExecutionManifest,
    entries: &mut Vec<OutputCommitEntry>,
) -> Result<(), MaterializeError> {
    if manifest.mounts.work_promote_paths.is_empty() {
        return Ok(());
    }

    let work_root = run_root.join("work");
    for rel in &manifest.mounts.work_promote_paths {
        let guest_rel = normalize_guest_path(rel)?;
        validate_output_relative(&guest_rel)?;
        let host = work_root.join(&guest_rel);
        if !host.is_file() {
            continue;
        }

        let relative_path = guest_rel.to_string_lossy().replace('\\', "/");
        if entries.iter().any(|entry| entry.relative_path == relative_path) {
            continue;
        }

        let mut file = fs::File::open(&host).map_err(|source| MaterializeError::Io {
            path: host.clone(),
            source,
        })?;
        let mut content = Vec::new();
        file.read_to_end(&mut content)
            .map_err(|source| MaterializeError::Io {
                path: host.clone(),
                source,
            })?;

        push_entry(entries, relative_path, content);
    }

    Ok(())
}

fn walk_output(
    output_root: &Path,
    current: &Path,
    entries: &mut Vec<OutputCommitEntry>,
) -> Result<(), MaterializeError> {
    for entry in fs::read_dir(current).map_err(|source| MaterializeError::Io {
        path: current.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| MaterializeError::Io {
            path: current.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| MaterializeError::Io {
            path: path.clone(),
            source,
        })?;

        if file_type.is_dir() {
            walk_output(output_root, &path, entries)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let rel = path
            .strip_prefix(output_root)
            .map_err(|_| MaterializeError::PathEscape {
                guest_path: path.display().to_string(),
                reason: "output path left output root".into(),
            })?;
        validate_output_relative(rel)?;

        let mut file = fs::File::open(&path).map_err(|source| MaterializeError::Io {
            path: path.clone(),
            source,
        })?;
        let mut content = Vec::new();
        file.read_to_end(&mut content)
            .map_err(|source| MaterializeError::Io {
                path: path.clone(),
                source,
            })?;

        push_entry(
            entries,
            rel.to_string_lossy().replace('\\', "/"),
            content,
        );
    }

    Ok(())
}

fn validate_output_relative(path: &Path) -> Result<(), MaterializeError> {
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir => {
                return Err(MaterializeError::PathEscape {
                    guest_path: path.display().to_string(),
                    reason: "output relative path must not contain `..`".into(),
                });
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(MaterializeError::PathEscape {
                    guest_path: path.display().to_string(),
                    reason: "output relative path must not be absolute".into(),
                });
            }
        }
    }
    Ok(())
}

fn normalize_prefix(prefix: String) -> String {
    let trimmed = prefix.trim().trim_matches('/');
    if trimmed.is_empty() {
        "output".to_string()
    } else {
        trimmed.to_string()
    }
}

fn join_proposal_path(prefix: &str, relative: &str) -> String {
    let rel = relative.trim_start_matches('/');
    if prefix.is_empty() {
        rel.to_string()
    } else {
        format!("{prefix}/{rel}")
    }
}

/// Map plan entries to Lattice proposal drafts (hosts decide how to propose by `kind`).
pub fn lattice_proposal_drafts(
    plan: &OutputCommitPlan,
    proposal_target_prefix: &str,
) -> Vec<LatticeProposalDraft> {
    let prefix = normalize_prefix(proposal_target_prefix.to_string());
    plan.entries
        .iter()
        .map(|entry| {
            let resource_path = join_proposal_path(&prefix, &entry.relative_path);
            LatticeProposalDraft {
                summary: format!("Create resource {resource_path} from KernelFS run {}", plan.run_id),
                resource_path,
                content: entry.content.clone(),
                kind: entry.kind,
            }
        })
        .collect()
}

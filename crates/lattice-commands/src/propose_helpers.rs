//! Typed helpers that validate agent payloads and build proposal command bundles.
//!
//! These helpers never apply mutations — callers persist via [`create_proposal`]
//! (MCP/HTTP) or the desktop inbox. Apply remains desktop-only.

use std::path::{Component, Path, PathBuf};

use lattice_data::InterfaceDef;

use crate::artifact::ArtifactManifest;
use crate::command::Command;
use crate::contracts::{ProposalSource, ProposalStatus, TransactionProposal};
use crate::proposal::create_proposal;
use crate::workflow::WorkflowManifest;
use crate::{Error, Result};

/// Validated command bundle ready for [`create_proposal`].
#[derive(Debug, Clone, PartialEq)]
pub struct ProposeBundle {
    pub summary: String,
    pub commands: Vec<Command>,
    pub affected_paths: Vec<String>,
    pub warnings: Vec<String>,
}

impl ProposeBundle {
    /// Persist as a pending proposal under `.lattice/proposals/`.
    pub fn create(
        self,
        workspace_root: &Path,
        source: ProposalSource,
    ) -> Result<TransactionProposal> {
        create_proposal(
            workspace_root,
            TransactionProposal {
                id: String::new(),
                source,
                summary: self.summary,
                commands: self.commands,
                affected_paths: self.affected_paths,
                warnings: self.warnings,
                created_at: String::new(),
                status: ProposalStatus::Pending,
            },
        )
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Normalize a workspace-relative path (reject absolute / `..` escapes).
pub fn normalize_proposal_rel_path(path: &str) -> Result<PathBuf> {
    let mut text = path.trim().replace('\\', "/");
    while text.starts_with("./") {
        text = text[2..].to_string();
    }
    if text.is_empty() {
        return Err(Error::InvalidResourceTarget {
            path: PathBuf::from(path),
            reason: "path must not be empty".into(),
        });
    }
    let candidate = PathBuf::from(&text);
    if candidate.is_absolute() {
        return Err(Error::InvalidResourceTarget {
            path: candidate,
            reason: "path must be workspace-relative".into(),
        });
    }
    for component in candidate.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(Error::InvalidResourceTarget {
                    path: candidate,
                    reason: "path must not escape the workspace root".into(),
                });
            }
        }
    }
    Ok(candidate)
}

fn resource_create(path: PathBuf, content: &str) -> Command {
    Command::ResourceCreate {
        path,
        content: content.as_bytes().to_vec(),
    }
}

fn looks_like_suffix(path: &Path, suffix: &str) -> bool {
    path_string(path)
        .to_ascii_lowercase()
        .ends_with(&suffix.to_ascii_lowercase())
}

/// Propose creating an arbitrary text resource via [`Command::ResourceCreate`].
pub fn propose_resource(path: &str, content: &str) -> Result<ProposeBundle> {
    let rel = normalize_proposal_rel_path(path)?;
    let display = path_string(&rel);
    Ok(ProposeBundle {
        summary: format!("Create resource {display}"),
        commands: vec![resource_create(rel, content)],
        affected_paths: vec![display],
        warnings: Vec::new(),
    })
}

/// Validate workflow YAML and propose creating the workflow file.
pub fn propose_workflow(path: &str, yaml: &str) -> Result<ProposeBundle> {
    let rel = normalize_proposal_rel_path(path)?;
    WorkflowManifest::parse(&rel, yaml).map_err(|err| Error::InvalidResourceTarget {
        path: rel.clone(),
        reason: err.to_string(),
    })?;
    let display = path_string(&rel);
    let mut warnings = Vec::new();
    if !looks_like_suffix(&rel, ".workflow.yaml") {
        warnings.push(format!(
            "path {display:?} does not end with .workflow.yaml; workflow discovery may ignore it"
        ));
    }
    Ok(ProposeBundle {
        summary: format!("Create workflow {display}"),
        commands: vec![resource_create(rel, yaml)],
        affected_paths: vec![display],
        warnings,
    })
}

/// Validate interface YAML and propose creating the interface file.
pub fn propose_interface(path: &str, yaml: &str) -> Result<ProposeBundle> {
    let rel = normalize_proposal_rel_path(path)?;
    InterfaceDef::parse_str(yaml, &rel).map_err(|err| Error::InvalidResourceTarget {
        path: rel.clone(),
        reason: err.to_string(),
    })?;
    let display = path_string(&rel);
    let mut warnings = Vec::new();
    if !looks_like_suffix(&rel, ".interface.yaml") {
        warnings.push(format!(
            "path {display:?} does not end with .interface.yaml; package loaders may ignore it"
        ));
    }
    Ok(ProposeBundle {
        summary: format!("Create interface {display}"),
        commands: vec![resource_create(rel, yaml)],
        affected_paths: vec![display],
        warnings,
    })
}

/// Validate `artifact.yaml` and propose creating the manifest file.
///
/// `path` may be the manifest path (`….artifact/artifact.yaml`) or the package
/// directory (`….artifact`); the latter is rewritten to `artifact.yaml`.
pub fn propose_artifact(path: &str, yaml: &str) -> Result<ProposeBundle> {
    let rel = normalize_proposal_rel_path(path)?;
    let manifest_rel = if looks_like_suffix(&rel, "artifact.yaml") {
        rel
    } else if looks_like_suffix(&rel, ".artifact") {
        rel.join("artifact.yaml")
    } else {
        return Err(Error::InvalidResourceTarget {
            path: rel,
            reason: "artifact path must end with .artifact or artifact.yaml".into(),
        });
    };
    ArtifactManifest::parse_str(yaml, &manifest_rel).map_err(|err| {
        Error::InvalidResourceTarget {
            path: manifest_rel.clone(),
            reason: err.to_string(),
        }
    })?;
    let display = path_string(&manifest_rel);
    Ok(ProposeBundle {
        summary: format!("Create artifact manifest {display}"),
        commands: vec![resource_create(manifest_rel, yaml)],
        affected_paths: vec![display],
        warnings: vec![
            "proposal writes artifact.yaml only; entrypoint HTML and package dirs still need separate commands"
                .into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::ProposalSourceType;
    use lattice_core::Workspace;
    use tempfile::TempDir;

    fn valid_workflow_yaml() -> &'static str {
        r#"format: lattice-workflow
version: 1
name: Demo
enabled: true
trigger:
  type: manual
steps:
  - id: notify
    action: notification
    with:
      message: hi
"#
    }

    fn valid_interface_yaml() -> &'static str {
        r#"format: lattice-interface
version: 1
name: Overview
views:
  - Main
"#
    }

    fn valid_artifact_yaml() -> &'static str {
        r#"format: lattice-artifact
version: 1
title: Pulse
entrypoint: ./index.html
bindings: {}
permissions:
  network: []
  workspace_write: []
"#
    }

    #[test]
    fn propose_resource_builds_resource_create() {
        let bundle = propose_resource("Notes/raw.txt", "hello\n").unwrap();
        assert_eq!(bundle.summary, "Create resource Notes/raw.txt");
        assert_eq!(bundle.affected_paths, vec!["Notes/raw.txt"]);
        match &bundle.commands[0] {
            Command::ResourceCreate { path, content } => {
                assert_eq!(path, Path::new("Notes/raw.txt"));
                assert_eq!(content, b"hello\n");
            }
            other => panic!("expected ResourceCreate, got {other:?}"),
        }
    }

    #[test]
    fn propose_resource_rejects_escape() {
        let err = propose_resource("../outside.txt", "x").unwrap_err();
        assert!(err.to_string().contains("escape") || err.to_string().contains("workspace"));
    }

    #[test]
    fn propose_workflow_validates_and_warns_on_suffix() {
        let bundle = propose_workflow("Automations/Demo.workflow.yaml", valid_workflow_yaml()).unwrap();
        assert!(bundle.warnings.is_empty());
        assert!(matches!(bundle.commands[0], Command::ResourceCreate { .. }));

        let warned = propose_workflow("Automations/Demo.yaml", valid_workflow_yaml()).unwrap();
        assert_eq!(warned.warnings.len(), 1);

        let err = propose_workflow(
            "Automations/Bad.workflow.yaml",
            "format: no\nversion: 1\nname: x\ntrigger:\n  type: manual\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("invalid") || err.to_string().contains("format"));
    }

    #[test]
    fn propose_interface_validates_stem() {
        let bundle =
            propose_interface("CRM.data/interfaces/Overview.interface.yaml", valid_interface_yaml())
                .unwrap();
        assert!(bundle.warnings.is_empty());

        let err = propose_interface(
            "CRM.data/interfaces/Wrong.interface.yaml",
            valid_interface_yaml(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("does not match") || err.to_string().contains("name"));
    }

    #[test]
    fn propose_artifact_rewrites_package_path() {
        let bundle = propose_artifact("Artifacts/Pulse.artifact", valid_artifact_yaml()).unwrap();
        assert_eq!(
            bundle.affected_paths,
            vec!["Artifacts/Pulse.artifact/artifact.yaml"]
        );
        assert!(!bundle.warnings.is_empty());
    }

    #[test]
    fn propose_bundle_create_persists_pending_proposal() {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "Helpers").unwrap();
        let proposal = propose_workflow("Automations/Demo.workflow.yaml", valid_workflow_yaml())
            .unwrap()
            .create(
                dir.path(),
                ProposalSource {
                    source_type: ProposalSourceType::Mcp,
                    resource: None,
                },
            )
            .unwrap();
        assert!(!proposal.id.is_empty());
        assert_eq!(proposal.status, ProposalStatus::Pending);
        assert!(!dir.path().join("Automations/Demo.workflow.yaml").exists());
        assert!(dir
            .path()
            .join(".lattice/proposals")
            .join(format!("{}.json", proposal.id))
            .is_file());
    }
}

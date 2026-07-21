use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::EnvError;
use crate::path_util::find_on_path;
use crate::Result;

/// Which environment provider to resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvKind {
    /// Host `PATH` interpreter (`python3`, then `python`).
    System,
    /// Directory with `pyproject.toml` and/or `uv.lock`; resolved via `uv`.
    UvProject { project_dir: PathBuf },
    /// Optional Nix flake / `shell.nix` root (stub until J6).
    Nix { root: PathBuf },
}

/// Resolved interpreter and optional `PATH` override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEnv {
    /// Absolute path to the Python interpreter.
    pub python: PathBuf,
    /// Optional `PATH` value to use when spawning tools in this env.
    ///
    /// `None` means keep the ambient process `PATH` (typical for [`EnvKind::System`]).
    pub path_env: Option<OsString>,
    /// Human-readable provenance (which provider / how it was found).
    pub provenance: String,
}

/// Environment resolver with an injectable `PATH` for tests.
#[derive(Debug, Clone, Default)]
pub struct EnvProvider {
    /// When set, used instead of the process `PATH` for tool discovery.
    path_override: Option<OsString>,
}

impl EnvProvider {
    /// Resolve using the ambient process environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve using a fixed `PATH` (tests / sandboxed callers).
    pub fn with_path(path: impl Into<OsString>) -> Self {
        Self {
            path_override: Some(path.into()),
        }
    }

    fn search_path(&self) -> OsString {
        self.path_override.clone().unwrap_or_else(|| {
            std::env::var_os("PATH").unwrap_or_else(|| OsString::from(""))
        })
    }

    /// Resolve `request` to a Python interpreter and optional PATH overlay.
    pub fn resolve(&self, request: EnvKind) -> Result<ResolvedEnv> {
        match request {
            EnvKind::System => self.resolve_system(),
            EnvKind::UvProject { project_dir } => self.resolve_uv_project(&project_dir),
            EnvKind::Nix { root } => resolve_nix_stub(&root),
        }
    }

    fn resolve_system(&self) -> Result<ResolvedEnv> {
        let path = self.search_path();
        let python = find_on_path("python3", &path)
            .or_else(|| find_on_path("python", &path))
            .ok_or_else(|| EnvError::MissingTool {
                tool: "python3".into(),
            })?;

        let name = python
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("python")
            .to_string();
        Ok(ResolvedEnv {
            python,
            path_env: None,
            provenance: format!("system:{name}"),
        })
    }

    fn resolve_uv_project(&self, project_dir: &Path) -> Result<ResolvedEnv> {
        let project_dir = project_dir
            .canonicalize()
            .map_err(|source| EnvError::Io(source))?;

        let has_pyproject = project_dir.join("pyproject.toml").is_file();
        let has_lock = project_dir.join("uv.lock").is_file();
        if !has_pyproject && !has_lock {
            return Err(EnvError::NotAUvProject {
                path: project_dir,
            });
        }

        let search = self.search_path();
        let uv = find_on_path("uv", &search).ok_or_else(|| EnvError::MissingTool {
            tool: "uv".into(),
        })?;

        let output = Command::new(&uv)
            .args([
                "python",
                "find",
                "--directory",
                project_dir.to_str().ok_or_else(|| EnvError::Unavailable {
                    reason: format!(
                        "project directory is not valid UTF-8: {}",
                        project_dir.display()
                    ),
                })?,
            ])
            .env("PATH", &search)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let detail = if stderr.is_empty() {
                format!("exit status {}", output.status)
            } else {
                stderr
            };
            return Err(EnvError::ToolFailed {
                tool: "uv".into(),
                detail,
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let python_line = stdout
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .ok_or_else(|| EnvError::ToolFailed {
                tool: "uv".into(),
                detail: "python find returned empty stdout".into(),
            })?;

        let python = PathBuf::from(python_line);
        if !python.is_file() {
            return Err(EnvError::ToolFailed {
                tool: "uv".into(),
                detail: format!("python find returned non-file path: {python_line}"),
            });
        }

        let path_env = python
            .parent()
            .map(|bin_dir| prepend_path(bin_dir, &search));

        Ok(ResolvedEnv {
            python,
            path_env,
            provenance: format!("uv-project:{}", project_dir.display()),
        })
    }
}

/// Resolve using the default [`EnvProvider`] (ambient `PATH`).
pub fn resolve(request: EnvKind) -> Result<ResolvedEnv> {
    EnvProvider::new().resolve(request)
}

fn resolve_nix_stub(root: &Path) -> Result<ResolvedEnv> {
    // J6 implements real flake / shell.nix resolution. Never fall back to system.
    Err(EnvError::Unavailable {
        reason: format!(
            "nix EnvProvider is not implemented yet (planned in J6); requested root={}",
            root.display()
        ),
    })
}

fn prepend_path(bin_dir: &Path, existing: &OsStr) -> OsString {
    let mut dirs = vec![bin_dir.to_path_buf()];
    dirs.extend(std::env::split_paths(existing));
    std::env::join_paths(dirs).unwrap_or_else(|_| bin_dir.as_os_str().to_os_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn write_executable(path: &Path, body: &[u8]) {
        fs::write(path, body).unwrap();
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    fn path_with(dirs: &[&Path]) -> OsString {
        std::env::join_paths(dirs.iter().copied()).unwrap()
    }

    #[test]
    fn system_prefers_python3_over_python() {
        let dir = tempfile::tempdir().unwrap();
        let python3 = dir.path().join("python3");
        let python = dir.path().join("python");
        write_executable(&python3, b"#!/bin/sh\n");
        write_executable(&python, b"#!/bin/sh\n");

        let provider = EnvProvider::with_path(path_with(&[dir.path()]));
        let resolved = provider.resolve(EnvKind::System).unwrap();
        assert_eq!(resolved.python, python3);
        assert!(resolved.path_env.is_none());
        assert_eq!(resolved.provenance, "system:python3");
    }

    #[test]
    fn system_falls_back_to_python() {
        let dir = tempfile::tempdir().unwrap();
        let python = dir.path().join("python");
        write_executable(&python, b"#!/bin/sh\n");

        let provider = EnvProvider::with_path(path_with(&[dir.path()]));
        let resolved = provider.resolve(EnvKind::System).unwrap();
        assert_eq!(resolved.python, python);
        assert_eq!(resolved.provenance, "system:python");
    }

    #[test]
    fn system_missing_tool_when_empty_path() {
        let empty = tempfile::tempdir().unwrap();
        let provider = EnvProvider::with_path(path_with(&[empty.path()]));
        let err = provider.resolve(EnvKind::System).unwrap_err();
        match err {
            EnvError::MissingTool { tool } => assert_eq!(tool, "python3"),
            other => panic!("expected MissingTool, got {other:?}"),
        }
    }

    #[test]
    fn uv_project_missing_uv_tool() {
        let project = tempfile::tempdir().unwrap();
        fs::write(project.path().join("pyproject.toml"), b"[project]\nname=\"t\"\n").unwrap();
        let empty = tempfile::tempdir().unwrap();
        let provider = EnvProvider::with_path(path_with(&[empty.path()]));
        let err = provider
            .resolve(EnvKind::UvProject {
                project_dir: project.path().to_path_buf(),
            })
            .unwrap_err();
        match err {
            EnvError::MissingTool { tool } => assert_eq!(tool, "uv"),
            other => panic!("expected MissingTool, got {other:?}"),
        }
    }

    #[test]
    fn uv_project_requires_marker_files() {
        let project = tempfile::tempdir().unwrap();
        let provider = EnvProvider::with_path(path_with(&[project.path()]));
        let err = provider
            .resolve(EnvKind::UvProject {
                project_dir: project.path().to_path_buf(),
            })
            .unwrap_err();
        match err {
            EnvError::NotAUvProject { path } => {
                assert_eq!(path, project.path().canonicalize().unwrap());
            }
            other => panic!("expected NotAUvProject, got {other:?}"),
        }
    }

    #[test]
    fn uv_project_accepts_uv_lock_only() {
        let project = tempfile::tempdir().unwrap();
        fs::write(project.path().join("uv.lock"), b"# lock\n").unwrap();
        let empty = tempfile::tempdir().unwrap();
        let provider = EnvProvider::with_path(path_with(&[empty.path()]));
        let err = provider
            .resolve(EnvKind::UvProject {
                project_dir: project.path().to_path_buf(),
            })
            .unwrap_err();
        // Marker accepted; failure is missing uv, not NotAUvProject.
        match err {
            EnvError::MissingTool { tool } => assert_eq!(tool, "uv"),
            other => panic!("expected MissingTool after lock-only project, got {other:?}"),
        }
    }

    #[test]
    fn nix_stub_is_unavailable_without_system_fallback() {
        let root = tempfile::tempdir().unwrap();
        // Even with python on PATH, Nix must not resolve to system.
        let bin = tempfile::tempdir().unwrap();
        write_executable(&bin.path().join("python3"), b"#!/bin/sh\n");
        let provider = EnvProvider::with_path(path_with(&[bin.path()]));
        let err = provider
            .resolve(EnvKind::Nix {
                root: root.path().to_path_buf(),
            })
            .unwrap_err();
        match err {
            EnvError::Unavailable { reason } => {
                assert!(reason.contains("J6"), "reason={reason}");
                assert!(reason.contains("not implemented"), "reason={reason}");
            }
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    #[test]
    fn nix_stub_via_free_function() {
        let err = resolve(EnvKind::Nix {
            root: PathBuf::from("/tmp/fake-nix-root"),
        })
        .unwrap_err();
        assert!(matches!(err, EnvError::Unavailable { .. }));
    }

    /// Integration: real `uv` on the host PATH (skipped when absent).
    #[test]
    fn uv_project_resolves_with_real_uv_when_available() {
        let host_path = match std::env::var_os("PATH") {
            Some(p) => p,
            None => return,
        };
        if find_on_path("uv", &host_path).is_none() {
            return;
        }

        let project = tempfile::tempdir().unwrap();
        fs::write(
            project.path().join("pyproject.toml"),
            b"[project]\nname = \"lattice-env-test\"\nversion = \"0.0.0\"\nrequires-python = \">=3.11\"\n",
        )
        .unwrap();

        let provider = EnvProvider::with_path(host_path);
        let resolved = provider
            .resolve(EnvKind::UvProject {
                project_dir: project.path().to_path_buf(),
            })
            .expect("uv python find should succeed");
        assert!(resolved.python.is_file());
        assert!(resolved.provenance.starts_with("uv-project:"));
        assert!(resolved.path_env.is_some());
    }
}

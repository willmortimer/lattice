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
    /// Optional Nix flake / `shell.nix` root via `nix print-dev-env`.
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

    /// Absolute path to `name` on this provider's search `PATH`, if present.
    pub fn find_tool(&self, name: &str) -> Option<PathBuf> {
        find_on_path(name, &self.search_path())
    }

    /// `PATH` value used for tool discovery and recommended for child processes.
    pub fn path_for_spawn(&self) -> OsString {
        self.search_path()
    }

    fn search_path(&self) -> OsString {
        self.path_override
            .clone()
            .unwrap_or_else(|| std::env::var_os("PATH").unwrap_or_else(|| OsString::from("")))
    }

    /// Resolve `request` to a Python interpreter and optional PATH overlay.
    pub fn resolve(&self, request: EnvKind) -> Result<ResolvedEnv> {
        match request {
            EnvKind::System => self.resolve_system(),
            EnvKind::UvProject { project_dir } => self.resolve_uv_project(&project_dir),
            EnvKind::Nix { root } => self.resolve_nix(&root),
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
            return Err(EnvError::NotAUvProject { path: project_dir });
        }

        let search = self.search_path();
        let uv = find_on_path("uv", &search)
            .ok_or_else(|| EnvError::MissingTool { tool: "uv".into() })?;

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

    fn resolve_nix(&self, root: &Path) -> Result<ResolvedEnv> {
        let root = root
            .canonicalize()
            .map_err(|source| EnvError::Io(source))?;

        let search = self.search_path();
        let nix = find_on_path("nix", &search).ok_or_else(|| EnvError::MissingTool {
            tool: "nix".into(),
        })?;

        let has_flake = root.join("flake.nix").is_file();
        let has_shell = root.join("shell.nix").is_file();
        if !has_flake && !has_shell {
            return Err(EnvError::Unavailable {
                reason: format!(
                    "nix EnvProvider requires flake.nix or shell.nix under {}",
                    root.display()
                ),
            });
        }

        // Prefer flake when both exist; never fall back to system Python.
        let output = if has_flake {
            let root_str = root.to_str().ok_or_else(|| EnvError::Unavailable {
                reason: format!(
                    "nix root is not valid UTF-8: {}",
                    root.display()
                ),
            })?;
            let installable = format!("path:{root_str}#");
            Command::new(&nix)
                .args(["print-dev-env", "--json", &installable])
                .env("PATH", &search)
                .current_dir(&root)
                .output()?
        } else {
            let shell_nix = root.join("shell.nix");
            let shell_str = shell_nix.to_str().ok_or_else(|| EnvError::Unavailable {
                reason: format!(
                    "shell.nix path is not valid UTF-8: {}",
                    shell_nix.display()
                ),
            })?;
            Command::new(&nix)
                .args(["print-dev-env", "--json", "-f", shell_str])
                .env("PATH", &search)
                .current_dir(&root)
                .output()?
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let detail = if stderr.is_empty() {
                format!("exit status {}", output.status)
            } else {
                stderr
            };
            return Err(EnvError::ToolFailed {
                tool: "nix".into(),
                detail,
            });
        }

        let nix_path = path_from_print_dev_env_json(&output.stdout)?;
        let python = find_on_path("python3", &nix_path)
            .or_else(|| find_on_path("python", &nix_path))
            .ok_or_else(|| EnvError::Unavailable {
                reason: format!(
                    "nix print-dev-env PATH has no python3/python (root={})",
                    root.display()
                ),
            })?;

        Ok(ResolvedEnv {
            python,
            path_env: Some(nix_path),
            provenance: format!("nix:{}", root.display()),
        })
    }
}

/// Resolve using the default [`EnvProvider`] (ambient `PATH`).
pub fn resolve(request: EnvKind) -> Result<ResolvedEnv> {
    EnvProvider::new().resolve(request)
}

/// Extract exported `PATH` from `nix print-dev-env --json` stdout.
fn path_from_print_dev_env_json(stdout: &[u8]) -> Result<OsString> {
    let value: serde_json::Value =
        serde_json::from_slice(stdout).map_err(|err| EnvError::ToolFailed {
            tool: "nix".into(),
            detail: format!("print-dev-env --json parse error: {err}"),
        })?;

    let path = value
        .pointer("/variables/PATH/value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| EnvError::ToolFailed {
            tool: "nix".into(),
            detail: "print-dev-env JSON missing variables.PATH.value".into(),
        })?;

    Ok(OsString::from(path))
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

    /// Fake `nix` that emits print-dev-env JSON whose PATH points at `nix_bin`.
    fn write_fake_nix(bin_dir: &Path, nix_bin: &Path, fail: bool) {
        let script = if fail {
            r#"#!/bin/sh
echo "fake nix: forced failure" >&2
exit 1
"#
            .to_string()
        } else {
            // Use printf (shell builtin on most systems) so a PATH that only
            // contains this fake `nix` still works in unit tests.
            let payload = serde_json::json!({
                "variables": {
                    "PATH": {
                        "type": "exported",
                        "value": nix_bin.to_string_lossy(),
                    }
                }
            });
            let escaped = payload.to_string().replace('\\', "\\\\").replace('"', "\\\"");
            format!("#!/bin/sh\necho \"{escaped}\"\n")
        };
        write_executable(&bin_dir.join("nix"), script.as_bytes());
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
        fs::write(
            project.path().join("pyproject.toml"),
            b"[project]\nname=\"t\"\n",
        )
        .unwrap();
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
    fn nix_missing_tool_without_system_fallback() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("flake.nix"), b"{}\n").unwrap();
        // System python on PATH must not satisfy a Nix request when nix is absent.
        let bin = tempfile::tempdir().unwrap();
        write_executable(&bin.path().join("python3"), b"#!/bin/sh\n");
        let provider = EnvProvider::with_path(path_with(&[bin.path()]));
        let err = provider
            .resolve(EnvKind::Nix {
                root: root.path().to_path_buf(),
            })
            .unwrap_err();
        match err {
            EnvError::MissingTool { tool } => assert_eq!(tool, "nix"),
            other => panic!("expected MissingTool, got {other:?}"),
        }
    }

    #[test]
    fn nix_unavailable_without_flake_or_shell() {
        let root = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        write_fake_nix(bin.path(), bin.path(), false);
        write_executable(&bin.path().join("python3"), b"#!/bin/sh\n");
        let provider = EnvProvider::with_path(path_with(&[bin.path()]));
        let err = provider
            .resolve(EnvKind::Nix {
                root: root.path().to_path_buf(),
            })
            .unwrap_err();
        match err {
            EnvError::Unavailable { reason } => {
                assert!(reason.contains("flake.nix"), "reason={reason}");
                assert!(reason.contains("shell.nix"), "reason={reason}");
            }
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    #[test]
    fn nix_resolves_python_from_print_dev_env_path() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("flake.nix"), b"{}\n").unwrap();

        let nix_bin = tempfile::tempdir().unwrap();
        let python3 = nix_bin.path().join("python3");
        write_executable(&python3, b"#!/bin/sh\n");

        let path_bin = tempfile::tempdir().unwrap();
        write_fake_nix(path_bin.path(), nix_bin.path(), false);
        // Ambient system python must be ignored; only nix PATH counts.
        write_executable(&path_bin.path().join("python3"), b"#!/bin/sh\necho system\n");

        let provider = EnvProvider::with_path(path_with(&[path_bin.path()]));
        let resolved = provider
            .resolve(EnvKind::Nix {
                root: root.path().to_path_buf(),
            })
            .expect("fake nix should resolve");
        assert_eq!(resolved.python, python3);
        assert_eq!(
            resolved.path_env.as_ref().map(|p| p.as_os_str()),
            Some(nix_bin.path().as_os_str())
        );
        assert!(
            resolved.provenance.starts_with("nix:"),
            "provenance={}",
            resolved.provenance
        );
    }

    #[test]
    fn nix_shell_nix_resolves_via_print_dev_env() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("shell.nix"), b"{ pkgs ? import <nixpkgs> {} }: pkgs.mkShell {}\n")
            .unwrap();

        let nix_bin = tempfile::tempdir().unwrap();
        let python = nix_bin.path().join("python");
        write_executable(&python, b"#!/bin/sh\n");

        let path_bin = tempfile::tempdir().unwrap();
        write_fake_nix(path_bin.path(), nix_bin.path(), false);

        let provider = EnvProvider::with_path(path_with(&[path_bin.path()]));
        let resolved = provider
            .resolve(EnvKind::Nix {
                root: root.path().to_path_buf(),
            })
            .expect("shell.nix path should resolve");
        assert_eq!(resolved.python, python);
        assert!(resolved.provenance.contains("nix:"));
    }

    #[test]
    fn nix_tool_failed_on_nonzero_exit() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("flake.nix"), b"{}\n").unwrap();
        let path_bin = tempfile::tempdir().unwrap();
        write_fake_nix(path_bin.path(), path_bin.path(), true);
        write_executable(&path_bin.path().join("python3"), b"#!/bin/sh\n");

        let provider = EnvProvider::with_path(path_with(&[path_bin.path()]));
        let err = provider
            .resolve(EnvKind::Nix {
                root: root.path().to_path_buf(),
            })
            .unwrap_err();
        match err {
            EnvError::ToolFailed { tool, detail } => {
                assert_eq!(tool, "nix");
                assert!(detail.contains("forced failure"), "detail={detail}");
            }
            other => panic!("expected ToolFailed, got {other:?}"),
        }
    }

    #[test]
    fn nix_unavailable_when_dev_env_path_has_no_python() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("flake.nix"), b"{}\n").unwrap();

        let empty_nix_path = tempfile::tempdir().unwrap();
        let path_bin = tempfile::tempdir().unwrap();
        write_fake_nix(path_bin.path(), empty_nix_path.path(), false);
        // System python present but must not be used.
        write_executable(&path_bin.path().join("python3"), b"#!/bin/sh\n");

        let provider = EnvProvider::with_path(path_with(&[path_bin.path()]));
        let err = provider
            .resolve(EnvKind::Nix {
                root: root.path().to_path_buf(),
            })
            .unwrap_err();
        match err {
            EnvError::Unavailable { reason } => {
                assert!(reason.contains("python3/python"), "reason={reason}");
            }
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    #[test]
    fn nix_via_free_function_missing_nix_files() {
        // Ambient PATH may contain nix; empty root still fails on missing files
        // (or MissingTool if nix is absent). Either way: no system Python.
        let root = tempfile::tempdir().unwrap();
        let err = resolve(EnvKind::Nix {
            root: root.path().to_path_buf(),
        })
        .unwrap_err();
        assert!(
            matches!(
                err,
                EnvError::Unavailable { .. } | EnvError::MissingTool { .. }
            ),
            "got {err:?}"
        );
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

//! Discover a local Python launcher for the ipykernel bridge.
//!
//! Kept crate-local on purpose: shared `lattice-env` resolution is J4.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::KernelError;

/// How to invoke the bridge script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PythonLauncher {
    /// `uv run --with ipykernel --with jupyter_client -- python <script>`
    Uv { uv: PathBuf },
    /// `python3 <script>` (packages must already be installed).
    System { python: PathBuf },
}

impl PythonLauncher {
    /// Prefer `uv` on `PATH`, else `python3`.
    pub fn discover() -> Result<Self, KernelError> {
        if let Some(uv) = find_on_path("uv") {
            return Ok(Self::Uv { uv });
        }
        if let Some(python) = find_on_path("python3") {
            return Ok(Self::System { python });
        }
        Err(KernelError::PythonNotFound)
    }

    /// Build a [`Command`] that runs `bridge_script` with the right deps story.
    ///
    /// Injects the shipped `packages/lattice-py` SDK onto `PYTHONPATH` and sets
    /// `LATTICE_WORKSPACE` so notebook cells can `import lattice`.
    pub fn command_for(&self, bridge_script: &Path, workspace_root: &Path) -> Command {
        let mut cmd = match self {
            Self::Uv { uv } => {
                let mut cmd = Command::new(uv);
                cmd.arg("run")
                    .arg("--with")
                    .arg("ipykernel")
                    .arg("--with")
                    .arg("jupyter_client")
                    .arg("--")
                    .arg("python")
                    .arg(bridge_script);
                cmd
            }
            Self::System { python } => {
                let mut cmd = Command::new(python);
                cmd.arg(bridge_script);
                cmd
            }
        };
        inject_lattice_python_sdk(&mut cmd, workspace_root);
        cmd
    }
}

/// Directory containing the injectable `lattice` Python package (parent of `lattice/`).
pub fn shipped_lattice_py_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/lattice-py")
}

/// Prepend the shipped SDK to `PYTHONPATH` and set `LATTICE_WORKSPACE`.
pub fn inject_lattice_python_sdk(cmd: &mut Command, workspace_root: &Path) {
    let mut entries = vec![shipped_lattice_py_dir()];
    if let Some(existing) = std::env::var_os("PYTHONPATH") {
        for entry in std::env::split_paths(&existing) {
            if !entry.as_os_str().is_empty() {
                entries.push(entry);
            }
        }
    }
    if let Ok(joined) = std::env::join_paths(&entries) {
        cmd.env("PYTHONPATH", joined);
    }
    cmd.env("LATTICE_WORKSPACE", workspace_root);
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let with_exe = dir.join(format!("{name}.exe"));
            if is_executable(&with_exe) {
                return Some(with_exe);
            }
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_finds_something_on_developer_machines() {
        // CI and local nix/mise envs almost always have python3 or uv.
        match PythonLauncher::discover() {
            Ok(PythonLauncher::Uv { uv }) => assert!(uv.is_file()),
            Ok(PythonLauncher::System { python }) => assert!(python.is_file()),
            Err(KernelError::PythonNotFound) => {
                // Rare bare environment — still a valid outcome for the API.
            }
            Err(other) => panic!("unexpected discover error: {other}"),
        }
    }

    #[test]
    fn uv_command_includes_with_deps_and_sdk_env() {
        let launcher = PythonLauncher::Uv {
            uv: PathBuf::from("/usr/bin/uv"),
        };
        let workspace = Path::new("/tmp/ws");
        let cmd = launcher.command_for(Path::new("/tmp/bridge.py"), workspace);
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.windows(2).any(|w| w == ["--with", "ipykernel"]));
        assert!(args.windows(2).any(|w| w == ["--with", "jupyter_client"]));
        assert_eq!(args.last().map(String::as_str), Some("/tmp/bridge.py"));

        let envs: Vec<(String, String)> = cmd
            .get_envs()
            .filter_map(|(k, v)| Some((k.to_str()?.to_string(), v?.to_str()?.to_string())))
            .collect();
        let pythonpath = envs
            .iter()
            .find(|(k, _)| k == "PYTHONPATH")
            .map(|(_, v)| v.as_str())
            .expect("PYTHONPATH");
        assert!(pythonpath.contains("lattice-py"), "PYTHONPATH={pythonpath}");
        let lattice_ws = envs
            .iter()
            .find(|(k, _)| k == "LATTICE_WORKSPACE")
            .map(|(_, v)| v.as_str())
            .expect("LATTICE_WORKSPACE");
        assert_eq!(lattice_ws, "/tmp/ws");
    }

    #[test]
    fn shipped_sdk_dir_contains_lattice_package() {
        let dir = shipped_lattice_py_dir();
        assert!(
            dir.join("lattice").join("__init__.py").is_file(),
            "missing SDK at {}",
            dir.display()
        );
    }
}

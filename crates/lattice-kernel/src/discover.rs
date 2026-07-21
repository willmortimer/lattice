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
    pub fn command_for(&self, bridge_script: &Path) -> Command {
        match self {
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
        }
    }
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
    fn uv_command_includes_with_deps() {
        let launcher = PythonLauncher::Uv {
            uv: PathBuf::from("/usr/bin/uv"),
        };
        let cmd = launcher.command_for(Path::new("/tmp/bridge.py"));
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.windows(2).any(|w| w == ["--with", "ipykernel"]));
        assert!(args.windows(2).any(|w| w == ["--with", "jupyter_client"]));
        assert_eq!(args.last().map(String::as_str), Some("/tmp/bridge.py"));
    }
}

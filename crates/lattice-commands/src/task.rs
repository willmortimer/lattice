//! Parse and execute `*.task/` packages (`task.yaml` + `uv` runtime).
//!
//! Phase-4 J5: run `provider: uv` tasks via `uv run --directory …` with a
//! timeout and captured stdout/stderr/exit. Proposed-transaction outputs,
//! schedules, and Nix providers are out of scope.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use lattice_env::{EnvError, EnvKind, EnvProvider};
use serde::{Deserialize, Serialize};

pub const TASK_FORMAT: &str = "lattice-task";
pub const TASK_MANIFEST_FILENAME: &str = "task.yaml";
pub const SUPPORTED_VERSION: u32 = 1;
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 300;
pub const UV_PROVIDER: &str = "uv";

/// Errors from loading or running a Lattice task package.
#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    /// A required external tool (for example `uv`) was not found.
    #[error("missing tool `{tool}` on PATH")]
    MissingTool { tool: String },

    /// `runtime.provider` is not supported by this runner.
    #[error("unsupported task runtime provider `{provider}` (only `uv` is supported in J5)")]
    UnsupportedProvider { provider: String },

    /// `task.yaml` failed structural validation after parse.
    #[error("invalid task manifest at {path}: {message}")]
    InvalidManifest { path: PathBuf, message: String },

    /// YAML parse failure.
    #[error("failed to parse {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    /// I/O while reading the package or spawning the process.
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The task exceeded `limits.timeout_seconds`.
    #[error("task timed out after {timeout_seconds}s")]
    TimedOut {
        timeout_seconds: u64,
        stdout: String,
        stderr: String,
    },

    /// Environment resolution failed (missing project markers, tool failure, …).
    #[error(transparent)]
    Env(#[from] EnvError),
}

pub type TaskResult<T> = std::result::Result<T, TaskError>;

/// Parsed `task.yaml` for a `.task/` package.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskManifest {
    pub format: String,
    pub version: u32,
    pub runtime: TaskRuntime,
    pub entrypoint: TaskEntrypoint,
    #[serde(default)]
    pub limits: TaskLimits,
}

/// Runtime block: currently Python via `uv` only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRuntime {
    #[serde(rename = "type")]
    pub runtime_type: String,
    pub provider: String,
    /// Project directory relative to the task package root (default `.`).
    #[serde(default = "default_project")]
    pub project: String,
}

fn default_project() -> String {
    ".".to_string()
}

/// Entrypoint argv after `uv run --directory <project> --`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskEntrypoint {
    pub command: Vec<String>,
}

/// Execution limits. Unknown future fields (e.g. `memory_mb`) are ignored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskLimits {
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
}

impl Default for TaskLimits {
    fn default() -> Self {
        Self {
            timeout_seconds: DEFAULT_TIMEOUT_SECONDS,
        }
    }
}

fn default_timeout_seconds() -> u64 {
    DEFAULT_TIMEOUT_SECONDS
}

impl TaskManifest {
    /// Load and validate `task.yaml` at `path`.
    pub fn load(path: &Path) -> TaskResult<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| TaskError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let manifest: TaskManifest =
            serde_yaml::from_str(&text).map_err(|source| TaskError::Yaml {
                path: path.to_path_buf(),
                source,
            })?;
        manifest.check(path)?;
        Ok(manifest)
    }

    fn check(&self, path: &Path) -> TaskResult<()> {
        let invalid = |message: String| TaskError::InvalidManifest {
            path: path.to_path_buf(),
            message,
        };
        if self.format != TASK_FORMAT {
            return Err(invalid(format!(
                "expected format {TASK_FORMAT:?}, found {:?}",
                self.format
            )));
        }
        if self.version == 0 || self.version > SUPPORTED_VERSION {
            return Err(invalid(format!(
                "manifest version {} is not supported (expected 1..={SUPPORTED_VERSION})",
                self.version
            )));
        }
        if self.runtime.provider != UV_PROVIDER {
            return Err(TaskError::UnsupportedProvider {
                provider: self.runtime.provider.clone(),
            });
        }
        if self.entrypoint.command.is_empty() {
            return Err(invalid(
                "entrypoint.command must be a non-empty argv list".into(),
            ));
        }
        if self.limits.timeout_seconds == 0 {
            return Err(invalid(
                "limits.timeout_seconds must be greater than zero".into(),
            ));
        }
        Ok(())
    }
}

/// Captured result of a successful (possibly non-zero) task process run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRunOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Runs Lattice task packages with an injectable [`EnvProvider`].
#[derive(Debug, Clone, Default)]
pub struct TaskRunner {
    env: EnvProvider,
}

impl TaskRunner {
    /// Use the ambient process environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Use a fixed `PATH` for tool discovery and child processes (tests).
    pub fn with_env(env: EnvProvider) -> Self {
        Self { env }
    }

    /// Resolve `path` (task package dir or `task.yaml`) and run it.
    pub fn run(&self, path: &Path) -> TaskResult<TaskRunOutput> {
        let (package_dir, manifest_path) = resolve_task_paths(path)?;
        let manifest = TaskManifest::load(&manifest_path)?;
        let project_dir = resolve_project_dir(&package_dir, &manifest.runtime.project)?;

        // Validates uv project markers and that `uv` is discoverable.
        self.env
            .resolve(EnvKind::UvProject {
                project_dir: project_dir.clone(),
            })
            .map_err(map_env_error)?;

        let uv = self
            .env
            .find_tool("uv")
            .ok_or_else(|| TaskError::MissingTool { tool: "uv".into() })?;

        let mut cmd = Command::new(&uv);
        cmd.arg("run")
            .arg("--directory")
            .arg(&project_dir)
            .arg("--")
            .args(&manifest.entrypoint.command)
            .current_dir(&package_dir)
            .env("PATH", self.env.path_for_spawn())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // Own process group so timeout can kill the whole tree.
            cmd.process_group(0);
        }

        let mut child = cmd.spawn().map_err(|source| TaskError::Io {
            path: package_dir.clone(),
            source,
        })?;

        let mut stdout_pipe = child.stdout.take().ok_or_else(|| TaskError::Io {
            path: package_dir.clone(),
            source: std::io::Error::new(std::io::ErrorKind::Other, "missing stdout pipe"),
        })?;
        let mut stderr_pipe = child.stderr.take().ok_or_else(|| TaskError::Io {
            path: package_dir.clone(),
            source: std::io::Error::new(std::io::ErrorKind::Other, "missing stderr pipe"),
        })?;

        let stdout_handle = thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stdout_pipe.read_to_end(&mut buf);
            buf
        });
        let stderr_handle = thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stderr_pipe.read_to_end(&mut buf);
            buf
        });

        let timeout = Duration::from_secs(manifest.limits.timeout_seconds);
        match wait_child_with_timeout(&mut child, timeout) {
            Ok(status) => {
                let stdout =
                    String::from_utf8_lossy(&stdout_handle.join().unwrap_or_default()).into_owned();
                let stderr =
                    String::from_utf8_lossy(&stderr_handle.join().unwrap_or_default()).into_owned();
                let exit_code = status.code().unwrap_or_else(|| {
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        // Convention: signal death as 128+signal when code() is None.
                        status.signal().map(|s| 128 + s).unwrap_or(1)
                    }
                    #[cfg(not(unix))]
                    {
                        1
                    }
                });
                Ok(TaskRunOutput {
                    exit_code,
                    stdout,
                    stderr,
                })
            }
            Err(()) => {
                let _ = kill_child_tree(&mut child);
                let _ = child.wait();
                let stdout =
                    String::from_utf8_lossy(&stdout_handle.join().unwrap_or_default()).into_owned();
                let stderr =
                    String::from_utf8_lossy(&stderr_handle.join().unwrap_or_default()).into_owned();
                Err(TaskError::TimedOut {
                    timeout_seconds: manifest.limits.timeout_seconds,
                    stdout,
                    stderr,
                })
            }
        }
    }
}

fn map_env_error(err: EnvError) -> TaskError {
    match err {
        EnvError::MissingTool { tool } => TaskError::MissingTool { tool },
        other => TaskError::Env(other),
    }
}

/// Accept a `.task/` directory or a path to `task.yaml`.
fn resolve_task_paths(path: &Path) -> TaskResult<(PathBuf, PathBuf)> {
    let meta = std::fs::metadata(path).map_err(|source| TaskError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if meta.is_dir() {
        let manifest = path.join(TASK_MANIFEST_FILENAME);
        if !manifest.is_file() {
            return Err(TaskError::InvalidManifest {
                path: path.to_path_buf(),
                message: format!("missing {TASK_MANIFEST_FILENAME}"),
            });
        }
        return Ok((path.to_path_buf(), manifest));
    }
    if meta.is_file() {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name != TASK_MANIFEST_FILENAME {
            return Err(TaskError::InvalidManifest {
                path: path.to_path_buf(),
                message: format!("expected {TASK_MANIFEST_FILENAME}, found {name:?}"),
            });
        }
        let package_dir =
            path.parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| TaskError::InvalidManifest {
                    path: path.to_path_buf(),
                    message: "task.yaml has no parent directory".into(),
                })?;
        return Ok((package_dir, path.to_path_buf()));
    }
    Err(TaskError::InvalidManifest {
        path: path.to_path_buf(),
        message: "path is neither a task directory nor task.yaml".into(),
    })
}

fn resolve_project_dir(package_dir: &Path, project: &str) -> TaskResult<PathBuf> {
    let project_dir = if project.is_empty() || project == "." {
        package_dir.to_path_buf()
    } else {
        package_dir.join(project)
    };
    if !project_dir.is_dir() {
        return Err(TaskError::InvalidManifest {
            path: package_dir.to_path_buf(),
            message: format!(
                "runtime.project {:?} is not a directory under the task package",
                project
            ),
        });
    }
    Ok(project_dir)
}

fn wait_child_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<std::process::ExitStatus, ()> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    return Err(());
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(_) => return Err(()),
        }
    }
}

fn kill_child_tree(child: &mut std::process::Child) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        // Negative PID: signal the process group started with process_group(0).
        let pid = child.id() as i32;
        let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
        Ok(())
    }
    #[cfg(not(unix))]
    {
        child.kill()
    }
}

/// Convenience: run with the default [`TaskRunner`].
pub fn run_task(path: &Path) -> TaskResult<TaskRunOutput> {
    TaskRunner::new().run(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn write_executable(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    fn sample_yaml(provider: &str, timeout: u64) -> String {
        format!(
            r#"format: lattice-task
version: 1
runtime:
  type: python
  provider: {provider}
  project: .
entrypoint:
  command: [python, main.py]
limits:
  timeout_seconds: {timeout}
"#
        )
    }

    #[test]
    fn parses_valid_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task.yaml");
        fs::write(&path, sample_yaml("uv", 60)).unwrap();
        let m = TaskManifest::load(&path).unwrap();
        assert_eq!(m.format, TASK_FORMAT);
        assert_eq!(m.runtime.provider, "uv");
        assert_eq!(m.entrypoint.command, vec!["python", "main.py"]);
        assert_eq!(m.limits.timeout_seconds, 60);
    }

    #[test]
    fn rejects_non_uv_provider() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task.yaml");
        fs::write(&path, sample_yaml("nix", 300)).unwrap();
        let err = TaskManifest::load(&path).unwrap_err();
        match err {
            TaskError::UnsupportedProvider { provider } => assert_eq!(provider, "nix"),
            other => panic!("expected UnsupportedProvider, got {other:?}"),
        }
    }

    #[test]
    fn rejects_wrong_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task.yaml");
        fs::write(
            &path,
            "format: other\nversion: 1\nruntime:\n  type: python\n  provider: uv\nentrypoint:\n  command: [python, main.py]\n",
        )
        .unwrap();
        let err = TaskManifest::load(&path).unwrap_err();
        assert!(matches!(err, TaskError::InvalidManifest { .. }));
    }

    #[test]
    fn defaults_timeout_when_limits_omitted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task.yaml");
        fs::write(
            &path,
            r#"format: lattice-task
version: 1
runtime:
  type: python
  provider: uv
entrypoint:
  command: [python, main.py]
"#,
        )
        .unwrap();
        let m = TaskManifest::load(&path).unwrap();
        assert_eq!(m.limits.timeout_seconds, DEFAULT_TIMEOUT_SECONDS);
        assert_eq!(m.runtime.project, ".");
    }

    #[test]
    fn resolve_paths_accepts_dir_and_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let pkg = dir.path().join("Hello.task");
        fs::create_dir_all(&pkg).unwrap();
        let yaml = pkg.join("task.yaml");
        fs::write(&yaml, sample_yaml("uv", 30)).unwrap();

        let (d1, m1) = resolve_task_paths(&pkg).unwrap();
        assert_eq!(d1, pkg);
        assert_eq!(m1, yaml);

        let (d2, m2) = resolve_task_paths(&yaml).unwrap();
        assert_eq!(d2, pkg);
        assert_eq!(m2, yaml);
    }

    #[test]
    fn missing_uv_returns_missing_tool() {
        let dir = tempfile::tempdir().unwrap();
        let pkg = dir.path().join("Hello.task");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("task.yaml"), sample_yaml("uv", 30)).unwrap();
        fs::write(
            pkg.join("pyproject.toml"),
            "[project]\nname = \"hello-task\"\nversion = \"0.0.0\"\nrequires-python = \">=3.11\"\n",
        )
        .unwrap();
        fs::write(pkg.join("main.py"), "print('ok')\n").unwrap();

        let empty = tempfile::tempdir().unwrap();
        let runner = TaskRunner::with_env(EnvProvider::with_path(empty.path()));
        let err = runner.run(&pkg).unwrap_err();
        match err {
            TaskError::MissingTool { tool } => assert_eq!(tool, "uv"),
            other => panic!("expected MissingTool, got {other:?}"),
        }
    }

    #[test]
    fn timeout_kills_long_running_fake_uv() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        fs::create_dir_all(&bin).unwrap();

        let pkg = dir.path().join("Slow.task");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("task.yaml"), sample_yaml("uv", 1)).unwrap();
        fs::write(
            pkg.join("pyproject.toml"),
            "[project]\nname = \"slow-task\"\nversion = \"0.0.0\"\nrequires-python = \">=3.11\"\n",
        )
        .unwrap();
        fs::write(pkg.join("main.py"), "print('never')\n").unwrap();

        // EnvProvider.resolve runs `uv python find`; afterwards `uv run` sleeps.
        let python_path = bin.join("python");
        let uv_script = format!(
            r#"#!/bin/sh
if [ "$1" = "python" ] && [ "$2" = "find" ]; then
  printf '%s\n' '{python}'
  exit 0
fi
exec sleep 30
"#,
            python = python_path.display()
        );
        write_executable(&bin.join("uv"), &uv_script);
        write_executable(&python_path, "#!/bin/sh\nexit 0\n");

        // Keep system utilities (`sleep`, `sh`) discoverable alongside the fake `uv`.
        let path = std::env::join_paths([bin.as_path(), Path::new("/bin"), Path::new("/usr/bin")])
            .unwrap();
        let runner = TaskRunner::with_env(EnvProvider::with_path(path));
        let err = runner.run(&pkg).unwrap_err();
        match err {
            TaskError::TimedOut {
                timeout_seconds, ..
            } => assert_eq!(timeout_seconds, 1),
            other => panic!("expected TimedOut, got {other:?}"),
        }
    }

    /// Integration: real `uv` runs the Hello.task fixture (skipped when absent).
    #[test]
    fn runs_fixture_with_real_uv_when_available() {
        let host_path = match std::env::var_os("PATH") {
            Some(p) => p,
            None => return,
        };
        if EnvProvider::with_path(host_path.clone())
            .find_tool("uv")
            .is_none()
        {
            return;
        }

        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/Hello.task");
        assert!(
            fixture.join("task.yaml").is_file(),
            "missing fixture at {}",
            fixture.display()
        );

        let runner = TaskRunner::with_env(EnvProvider::with_path(host_path));
        let out = runner.run(&fixture).expect("Hello.task should run");
        assert_eq!(out.exit_code, 0, "stderr={}", out.stderr);
        assert!(
            out.stdout.contains("ok"),
            "stdout={:?} stderr={:?}",
            out.stdout,
            out.stderr
        );
    }
}

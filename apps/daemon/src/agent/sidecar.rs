//! Supervised `agentd` sidecar over JSONL stdio pipes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::task::JoinHandle;
use tracing::{info, warn};

use super::backend::{AgentEventSink, AgentRuntimeBackend};
use super::protocol::{
    AgentCommand, AgentEvent, AgentRunHandle, AgentRunId, AgentRuntimeError, AgentRuntimeHealth,
    ProviderKind, StartAgentRunRequest, PROTOCOL_VERSION,
};

/// Path to the Node/tsx entry or packaged `agentd` executable.
pub const ENV_AGENTD_BIN: &str = "LATTICE_AGENTD_BIN";
/// Prefer the in-process fake backend (tests / CI).
pub const ENV_AGENT_FAKE: &str = "LATTICE_AGENT_FAKE";
/// Provider kind passed through to agentd (`pioneer` / `openai` / `fake`).
pub const ENV_AGENT_PROVIDER: &str = "LATTICE_AGENT_PROVIDER";
/// Model id passed through to agentd.
pub const ENV_AGENT_MODEL: &str = "LATTICE_AGENT_MODEL";

const MAX_RESTARTS: u32 = 5;
const INITIAL_BACKOFF_MS: u64 = 200;
const MAX_BACKOFF_MS: u64 = 5_000;
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);

struct ActiveRun {
    events: AgentEventSink,
}

struct ChildSession {
    child: Child,
    stdin: ChildStdin,
}

struct SidecarState {
    session: Option<ChildSession>,
    reader: Option<JoinHandle<()>>,
    restarts: AtomicU32,
}

/// Spawns and supervises `agentd`, speaking Phase A JSONL over pipes.
pub struct SidecarAgentBackend {
    binary: PathBuf,
    args: Vec<String>,
    provider: Option<String>,
    model: Option<String>,
    state: Mutex<SidecarState>,
    sinks: Arc<Mutex<HashMap<String, ActiveRun>>>,
    degraded: AtomicBool,
    stop: AtomicBool,
    ready: Notify,
    supervisor: Mutex<Option<JoinHandle<()>>>,
}

impl SidecarAgentBackend {
    /// Construct a sidecar backend for `binary` (optionally with argv after the exe).
    pub fn new(binary: PathBuf, args: Vec<String>) -> Arc<Self> {
        let provider = std::env::var(ENV_AGENT_PROVIDER)
            .ok()
            .filter(|s| !s.is_empty());
        let model = std::env::var(ENV_AGENT_MODEL)
            .ok()
            .filter(|s| !s.is_empty());
        Arc::new(Self {
            binary,
            args,
            provider,
            model,
            state: Mutex::new(SidecarState {
                session: None,
                reader: None,
                restarts: AtomicU32::new(0),
            }),
            sinks: Arc::new(Mutex::new(HashMap::new())),
            degraded: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            ready: Notify::new(),
            supervisor: Mutex::new(None),
        })
    }

    /// Spawn the child, perform hello handshake, and start the restart supervisor.
    pub async fn start(self: &Arc<Self>) -> Result<(), AgentRuntimeError> {
        self.spawn_child().await?;
        let backend = Arc::clone(self);
        let join = tokio::spawn(async move {
            backend.supervise_loop().await;
        });
        *self.supervisor.lock().await = Some(join);
        Ok(())
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded.load(Ordering::SeqCst)
    }

    pub fn mark_degraded(&self, degraded: bool) {
        self.degraded.store(degraded, Ordering::SeqCst);
    }

    pub async fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.supervisor.try_lock() {
            if let Some(join) = guard.take() {
                join.abort();
            }
        }
        let mut state = self.state.lock().await;
        if let Some(mut session) = state.session.take() {
            let _ = write_command(&mut session.stdin, &AgentCommand::Shutdown).await;
            let _ = session.child.kill().await;
            let _ = session.child.wait().await;
        }
        if let Some(reader) = state.reader.take() {
            reader.abort();
        }
        self.fail_active_runs("agentd shutting down").await;
    }

    async fn spawn_child(&self) -> Result<(), AgentRuntimeError> {
        let mut cmd = build_agentd_command(
            &self.binary,
            &self.args,
            self.provider.as_deref(),
            self.model.as_deref(),
        );
        let mut child = cmd.spawn().map_err(|err| {
            AgentRuntimeError::Spawn(format!(
                "failed to spawn agentd ({}): {err}",
                self.binary.display()
            ))
        })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentRuntimeError::Spawn("agentd stdout was not captured".into()))?;
        let stderr = child.stderr.take();
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentRuntimeError::Spawn("agentd stdin was not captured".into()))?;

        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    warn!(target: "lattice_agentd", "{line}");
                }
            });
        }

        let (hello_tx, mut hello_rx) = mpsc::channel::<Result<(), AgentRuntimeError>>(1);
        let sinks = Arc::clone(&self.sinks);
        let reader = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            let mut handed_hello = false;
            while let Ok(Some(line)) = lines.next_line().await {
                match AgentEvent::from_line(&line) {
                    Ok(AgentEvent::HelloAck { protocol_version }) => {
                        if !handed_hello {
                            handed_hello = true;
                            let result = if protocol_version == PROTOCOL_VERSION {
                                Ok(())
                            } else {
                                Err(AgentRuntimeError::Protocol(format!(
                                    "agentd hello_ack version mismatch: got {protocol_version}, expected {PROTOCOL_VERSION}"
                                )))
                            };
                            let _ = hello_tx.send(result).await;
                        }
                    }
                    Ok(AgentEvent::Health { .. }) => {
                        // Health replies are request/response; Phase A ignores unsolicited.
                    }
                    Ok(event) => {
                        let run_id = event.run_id().map(str::to_string);
                        let terminal = matches!(
                            event,
                            AgentEvent::RunCompleted { .. } | AgentEvent::RunFailed { .. }
                        );
                        if let Some(run_id) = run_id {
                            let sink = {
                                let guard = sinks.lock().await;
                                guard.get(&run_id).map(|r| r.events.clone())
                            };
                            if let Some(tx) = sink {
                                let _ = tx.send(event).await;
                                if terminal {
                                    sinks.lock().await.remove(&run_id);
                                }
                            }
                        }
                    }
                    Err(err) => {
                        warn!(error = %err, line = %line, "agentd stdout parse error");
                    }
                }
            }
            if !handed_hello {
                let _ = hello_tx
                    .send(Err(AgentRuntimeError::Protocol(
                        "agentd exited before hello_ack".into(),
                    )))
                    .await;
            }
        });

        write_command(
            &mut stdin,
            &AgentCommand::Hello {
                protocol_version: PROTOCOL_VERSION,
            },
        )
        .await?;

        match tokio::time::timeout(HELLO_TIMEOUT, hello_rx.recv()).await {
            Ok(Some(Ok(()))) => {}
            Ok(Some(Err(err))) => {
                reader.abort();
                let _ = child.kill().await;
                return Err(err);
            }
            Ok(None) => {
                reader.abort();
                let _ = child.kill().await;
                return Err(AgentRuntimeError::Protocol(
                    "agentd hello channel closed".into(),
                ));
            }
            Err(_) => {
                reader.abort();
                let _ = child.kill().await;
                return Err(AgentRuntimeError::Protocol(
                    "timed out waiting for agentd hello_ack".into(),
                ));
            }
        }

        let mut state = self.state.lock().await;
        if let Some(mut old) = state.session.take() {
            let _ = old.child.kill().await;
        }
        if let Some(old_reader) = state.reader.take() {
            old_reader.abort();
        }
        state.session = Some(ChildSession { child, stdin });
        state.reader = Some(reader);
        self.mark_degraded(false);
        self.ready.notify_waiters();
        info!(binary = %self.binary.display(), "agentd sidecar ready");
        Ok(())
    }

    async fn supervise_loop(self: Arc<Self>) {
        let mut backoff_ms = INITIAL_BACKOFF_MS;
        while !self.stop.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(250)).await;
            let exited = {
                let mut state = self.state.lock().await;
                let Some(session) = state.session.as_mut() else {
                    continue;
                };
                match session.child.try_wait() {
                    Ok(Some(status)) => Some(status),
                    Ok(None) => None,
                    Err(err) => {
                        warn!(error = %err, "agentd wait failed");
                        None
                    }
                }
            };
            let Some(status) = exited else {
                continue;
            };

            warn!(?status, "agentd exited; marking agent plane degraded");
            self.mark_degraded(true);
            self.fail_active_runs("agentd crashed").await;

            {
                let mut state = self.state.lock().await;
                state.session = None;
                if let Some(reader) = state.reader.take() {
                    reader.abort();
                }
                let restarts = state.restarts.fetch_add(1, Ordering::SeqCst) + 1;
                if restarts > MAX_RESTARTS {
                    warn!(restarts, "agentd restart budget exhausted");
                    continue;
                }
            }

            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
            backoff_ms = (backoff_ms.saturating_mul(2)).min(MAX_BACKOFF_MS);

            match self.spawn_child().await {
                Ok(()) => {
                    backoff_ms = INITIAL_BACKOFF_MS;
                    info!("agentd restarted");
                }
                Err(err) => {
                    warn!(error = %err, "failed to restart agentd");
                }
            }
        }
    }

    async fn fail_active_runs(&self, message: &str) {
        let mut sinks = self.sinks.lock().await;
        for (run_id, active) in sinks.drain() {
            let _ = active
                .events
                .send(AgentEvent::RunFailed {
                    run_id,
                    message: message.into(),
                    retryable: true,
                })
                .await;
        }
    }

    async fn write_to_child(&self, command: &AgentCommand) -> Result<(), AgentRuntimeError> {
        let mut state = self.state.lock().await;
        let Some(session) = state.session.as_mut() else {
            return Err(AgentRuntimeError::Unavailable(
                "agentd is not running".into(),
            ));
        };
        write_command(&mut session.stdin, command).await
    }
}

#[async_trait]
impl AgentRuntimeBackend for SidecarAgentBackend {
    async fn start_run(
        &self,
        request: StartAgentRunRequest,
        events: AgentEventSink,
    ) -> Result<AgentRunHandle, AgentRuntimeError> {
        request.validate()?;
        if self.is_degraded() {
            return Err(AgentRuntimeError::Unavailable(
                "agentd is degraded after crashes".into(),
            ));
        }

        {
            let mut sinks = self.sinks.lock().await;
            if sinks.contains_key(request.run_id.as_str()) {
                return Err(AgentRuntimeError::InvalidRequest(format!(
                    "run already active: {}",
                    request.run_id
                )));
            }
            sinks.insert(request.run_id.as_str().to_string(), ActiveRun { events });
        }

        if let Err(err) = self.write_to_child(&request.to_command()).await {
            self.sinks.lock().await.remove(request.run_id.as_str());
            return Err(err);
        }

        Ok(AgentRunHandle {
            run_id: request.run_id,
            thread_id: request.thread_id,
        })
    }

    async fn cancel_run(&self, run_id: AgentRunId) -> Result<(), AgentRuntimeError> {
        {
            let sinks = self.sinks.lock().await;
            if !sinks.contains_key(run_id.as_str()) {
                return Err(AgentRuntimeError::RunNotFound(run_id.to_string()));
            }
        }
        self.write_to_child(&AgentCommand::CancelRun {
            run_id: run_id.0.clone(),
        })
        .await
    }

    async fn health(&self) -> Result<AgentRuntimeHealth, AgentRuntimeError> {
        let degraded = self.is_degraded();
        let running = self.state.lock().await.session.is_some();
        Ok(AgentRuntimeHealth {
            ok: running && !degraded,
            backend: "sidecar".into(),
            degraded,
        })
    }
}

fn build_agentd_command(
    binary: &Path,
    args: &[String],
    provider: Option<&str>,
    model: Option<&str>,
) -> Command {
    let mut cmd = Command::new(binary);
    if !args.is_empty() {
        cmd.args(args);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    // Pass through provider secrets and Lattice HTTP tool credentials at spawn
    // only — never log values.
    for key in [
        "PIONEER_API_KEY",
        "OPENAI_API_KEY",
        "LATTICE_AUTH_TOKEN",
        "LATTICE_API_BASE_URL",
    ] {
        if let Ok(value) = std::env::var(key) {
            if !value.is_empty() {
                cmd.env(key, value);
            }
        }
    }
    if let Some(provider) = provider {
        cmd.env(ENV_AGENT_PROVIDER, provider);
    }
    if let Some(model) = model {
        cmd.env(ENV_AGENT_MODEL, model);
    }
    cmd
}

async fn write_command(
    stdin: &mut ChildStdin,
    command: &AgentCommand,
) -> Result<(), AgentRuntimeError> {
    let line = command.to_line()?;
    stdin.write_all(line.as_bytes()).await?;
    stdin.flush().await?;
    Ok(())
}

/// Resolve `LATTICE_AGENTD_BIN` into `(executable, trailing args)`.
///
/// Values may be a direct executable or a launcher form such as
/// `npx tsx apps/agentd/src/index.ts`. When unset, prefer the Rust
/// `lattice-agentd` binary (release/debug/next to latticed). Node
/// `run.sh` only when `LATTICE_AGENTD_PREFER_NODE` is set.
pub fn resolve_agentd_bin() -> Option<(PathBuf, Vec<String>)> {
    if let Ok(raw) = std::env::var(ENV_AGENTD_BIN) {
        if !raw.is_empty() {
            let mut parts = shell_split(&raw);
            if parts.is_empty() {
                return None;
            }
            let binary = PathBuf::from(parts.remove(0));
            return Some((binary, parts));
        }
    }
    discover_default_agentd_bin()
}

fn discover_default_agentd_bin() -> Option<(PathBuf, Vec<String>)> {
    // Relative to latticed crate (apps/daemon) → workspace root.
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for candidate in [
        workspace_root.join("target/release/lattice-agentd"),
        workspace_root.join("target/debug/lattice-agentd"),
    ] {
        let candidate = std::fs::canonicalize(&candidate).unwrap_or(candidate);
        if candidate.is_file() {
            return Some((candidate, Vec::new()));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sidecar = dir.join("lattice-agentd");
            if sidecar.is_file() {
                return Some((sidecar, Vec::new()));
            }
        }
    }

    // Node is opt-in only (`LATTICE_AGENTD_PREFER_NODE=1` or explicit LATTICE_AGENTD_BIN).
    if env_truthy("LATTICE_AGENTD_PREFER_NODE") {
        let run_sh = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../agentd/scripts/run.sh");
        let run_sh = std::fs::canonicalize(&run_sh).unwrap_or(run_sh);
        if run_sh.is_file() {
            return Some((run_sh, Vec::new()));
        }
    }
    None
}

fn shell_split(input: &str) -> Vec<String> {
    // Minimal whitespace split — enough for `node path/to/entry.js` style values.
    input.split_whitespace().map(str::to_string).collect()
}

pub fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

pub fn default_provider_from_env() -> ProviderKind {
    std::env::var(ENV_AGENT_PROVIDER)
        .ok()
        .as_deref()
        .and_then(ProviderKind::parse)
        .unwrap_or_else(|| {
            // Prefer OpenAI whenever OPENAI_API_KEY is set. Do not pick Pioneer over
            // OpenAI when both keys are present (desktop-dev / secrets/ai.env often
            // injects both). Fake/local paths are selected separately.
            if std::env::var("OPENAI_API_KEY")
                .ok()
                .filter(|s| !s.is_empty())
                .is_some()
            {
                ProviderKind::Openai
            } else {
                // Pioneer remains available only via explicit LATTICE_AGENT_PROVIDER
                // or when solely PIONEER_API_KEY is set (non-primary / dogfood).
                ProviderKind::Pioneer
            }
        })
}

pub fn default_model_from_env() -> String {
    if let Some(model) = std::env::var(ENV_AGENT_MODEL)
        .ok()
        .filter(|s| !s.is_empty())
    {
        return model;
    }
    match default_provider_from_env() {
        ProviderKind::Openai => "gpt-5-nano".into(),
        ProviderKind::Pioneer | ProviderKind::Fake => "gpt-5.6-luna".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command as StdCommand, Stdio};
    use std::time::Duration;

    async fn collect_events_until_terminal(
        rx: &mut mpsc::Receiver<AgentEvent>,
        limit: usize,
    ) -> Vec<AgentEvent> {
        let mut out = Vec::new();
        while out.len() < limit {
            match tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
                Ok(Some(event)) => {
                    let terminal = matches!(
                        event,
                        AgentEvent::RunCompleted { .. } | AgentEvent::RunFailed { .. }
                    );
                    out.push(event);
                    if terminal {
                        break;
                    }
                }
                Ok(None) | Err(_) => break,
            }
        }
        out
    }

    fn mock_agentd_script() -> String {
        r#"#!/usr/bin/env python3
import json, sys

def emit(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    msg = json.loads(line)
    t = msg.get("type")
    if t == "hello":
        emit({"type": "hello_ack", "protocolVersion": msg.get("protocolVersion", 1)})
    elif t == "start_run":
        run_id = msg["runId"]
        emit({"type": "run_started", "runId": run_id, "threadId": msg["threadId"]})
        emit({"type": "message_chunk", "runId": run_id, "chunk": {"type": "text-delta", "id": "1", "delta": "sidecar"}})
        emit({"type": "run_completed", "runId": run_id})
    elif t == "cancel_run":
        pass
    elif t == "health":
        emit({"type": "health", "ok": True})
    elif t == "shutdown":
        break
"#
        .into()
    }

    #[tokio::test]
    async fn sidecar_handshake_and_run_when_bin_set() {
        let dir = tempfile::tempdir().expect("tempdir");
        let script = dir.path().join("mock-agentd.py");
        std::fs::write(&script, mock_agentd_script()).expect("write script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script, perms).unwrap();
        }

        // Skip if python3 is unavailable.
        let python_ok = StdCommand::new("python3")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !python_ok {
            eprintln!("skipping sidecar test: python3 unavailable");
            return;
        }

        let backend = SidecarAgentBackend::new(script, Vec::new());
        backend.start().await.expect("start sidecar");

        let health = backend.health().await.expect("health");
        assert!(health.ok);
        assert_eq!(health.backend, "sidecar");

        let (tx, mut rx) = mpsc::channel(16);
        let handle = backend
            .start_run(
                StartAgentRunRequest {
                    thread_id: "thread-s".into(),
                    run_id: AgentRunId::new("run-s"),
                    provider: ProviderKind::Fake,
                    model: "mock".into(),
                    messages: None,
                    prompt: Some("hi".into()),
                    workspace_id: "ws".into(),
                    workspace_root: None,
                },
                tx,
            )
            .await
            .expect("start_run");
        assert_eq!(handle.run_id.as_str(), "run-s");

        let events = collect_events_until_terminal(&mut rx, 16).await;
        assert!(matches!(
            events.first(),
            Some(AgentEvent::RunStarted { .. })
        ));
        assert!(matches!(
            events.last(),
            Some(AgentEvent::RunCompleted { .. })
        ));

        backend.shutdown().await;
    }

    fn env_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    struct EnvGuard {
        key: String,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &str, value: Option<&str>) -> Self {
            let previous = std::env::var(key).ok();
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
            Self {
                key: key.to_string(),
                previous,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var(&self.key, value),
                None => std::env::remove_var(&self.key),
            }
        }
    }

    #[test]
    fn default_provider_prefers_openai_when_both_keys_set() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _provider = EnvGuard::set(ENV_AGENT_PROVIDER, None);
        let _openai = EnvGuard::set("OPENAI_API_KEY", Some("sk-test"));
        let _pioneer = EnvGuard::set("PIONEER_API_KEY", Some("pioneer-test"));
        assert_eq!(default_provider_from_env(), ProviderKind::Openai);
    }

    #[test]
    fn default_provider_uses_openai_when_only_openai_key_set() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _provider = EnvGuard::set(ENV_AGENT_PROVIDER, None);
        let _openai = EnvGuard::set("OPENAI_API_KEY", Some("sk-test"));
        let _pioneer = EnvGuard::set("PIONEER_API_KEY", None);
        assert_eq!(default_provider_from_env(), ProviderKind::Openai);
    }

    #[test]
    fn default_provider_falls_back_to_pioneer_without_openai() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _provider = EnvGuard::set(ENV_AGENT_PROVIDER, None);
        let _openai = EnvGuard::set("OPENAI_API_KEY", None);
        let _pioneer = EnvGuard::set("PIONEER_API_KEY", Some("pioneer-test"));
        assert_eq!(default_provider_from_env(), ProviderKind::Pioneer);
    }

    #[test]
    fn default_provider_respects_explicit_env_override() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _provider = EnvGuard::set(ENV_AGENT_PROVIDER, Some("pioneer"));
        let _openai = EnvGuard::set("OPENAI_API_KEY", Some("sk-test"));
        let _pioneer = EnvGuard::set("PIONEER_API_KEY", Some("pioneer-test"));
        assert_eq!(default_provider_from_env(), ProviderKind::Pioneer);
    }
}

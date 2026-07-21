//! Live bridge child process: stdio JSON-lines + kill-on-drop.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use uuid::Uuid;

use crate::cwd::resolve_cwd_under_workspace;
use crate::discover::PythonLauncher;
use crate::error::KernelError;
use crate::protocol::{BridgeRequest, BridgeResponse, ExecuteResult, KernelOutput};

const READY_TIMEOUT: Duration = Duration::from_secs(90);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Options for starting a kernel bridge session.
#[derive(Debug, Clone)]
pub struct StartOptions {
    /// Workspace root used for the cwd capability gate.
    pub workspace_root: PathBuf,
    /// Working directory for the bridge/kernel (must resolve under `workspace_root`).
    pub cwd: PathBuf,
    /// Optional override of the bridge script path (tests / packaging).
    pub bridge_script: Option<PathBuf>,
    /// Optional launcher override.
    pub launcher: Option<PythonLauncher>,
}

impl StartOptions {
    pub fn new(workspace_root: impl Into<PathBuf>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            cwd: cwd.into(),
            bridge_script: None,
            launcher: None,
        }
    }
}

type WaiterMap = Arc<Mutex<HashMap<String, Sender<BridgeResponse>>>>;

struct SessionInner {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    waiters: WaiterMap,
    alive: AtomicBool,
    reader_join: Mutex<Option<JoinHandle<()>>>,
}

/// A live out-of-process kernel bridge.
///
/// Cheap to clone: clones share the same child. `execute` and `interrupt` may
/// run concurrently (interrupt only needs stdin; responses are demuxed by id).
#[derive(Clone)]
pub struct KernelSession {
    inner: Arc<SessionInner>,
}

impl KernelSession {
    /// Discover Python, spawn the shipped bridge, and wait for `ready`.
    pub fn start(opts: StartOptions) -> Result<Self, KernelError> {
        let cwd = resolve_cwd_under_workspace(&opts.workspace_root, &opts.cwd)?;
        let bridge = opts
            .bridge_script
            .unwrap_or_else(default_bridge_script_path);
        if !bridge.is_file() {
            return Err(KernelError::spawn(format!(
                "bridge script not found: {}",
                bridge.display()
            )));
        }
        let launcher = match opts.launcher {
            Some(launcher) => launcher,
            None => PythonLauncher::discover()?,
        };
        let mut cmd = launcher.command_for(&bridge, &opts.workspace_root);
        cmd.current_dir(&cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        Self::spawn_command(cmd)
    }

    /// Spawn an arbitrary command as a bridge (used by unit tests with a mock).
    pub fn spawn_command(mut cmd: Command) -> Result<Self, KernelError> {
        let mut child = cmd.spawn().map_err(KernelError::spawn)?;
        let stdin = child.stdin.take().ok_or_else(|| {
            KernelError::spawn("bridge stdin was not captured")
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            KernelError::spawn("bridge stdout was not captured")
        })?;
        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    eprintln!("lattice-kernel bridge: {line}");
                }
            });
        }

        let waiters: WaiterMap = Arc::new(Mutex::new(HashMap::new()));
        let (ready_tx, ready_rx) = mpsc::channel::<BridgeResponse>();
        {
            let mut map = waiters.lock().map_err(|_| KernelError::DeadSession)?;
            map.insert("__ready__".into(), ready_tx);
        }

        let waiters_reader = Arc::clone(&waiters);
        let reader_join = thread::spawn(move || read_bridge_stdout(stdout, waiters_reader));

        let session = Self {
            inner: Arc::new(SessionInner {
                child: Mutex::new(child),
                stdin: Mutex::new(stdin),
                waiters,
                alive: AtomicBool::new(true),
                reader_join: Mutex::new(Some(reader_join)),
            }),
        };

        session.wait_for_ready(ready_rx)?;
        Ok(session)
    }

    fn wait_for_ready(&self, ready_rx: Receiver<BridgeResponse>) -> Result<(), KernelError> {
        let deadline = std::time::Instant::now() + READY_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(KernelError::Timeout);
            }
            match ready_rx.recv_timeout(remaining) {
                Ok(BridgeResponse::Ready) => {
                    let mut map = self
                        .inner
                        .waiters
                        .lock()
                        .map_err(|_| KernelError::DeadSession)?;
                    map.remove("__ready__");
                    return Ok(());
                }
                Ok(BridgeResponse::BridgeError { message, .. }) => {
                    return Err(KernelError::spawn(message));
                }
                Ok(other) => {
                    return Err(KernelError::protocol(format!(
                        "expected ready, got {other:?}"
                    )));
                }
                Err(RecvTimeoutError::Timeout) => return Err(KernelError::Timeout),
                Err(RecvTimeoutError::Disconnected) => {
                    self.inner.alive.store(false, Ordering::SeqCst);
                    return Err(KernelError::DeadSession);
                }
            }
        }
    }

    /// Execute `code` and collect outputs until the matching `done` response.
    pub fn execute(&self, code: impl Into<String>) -> Result<ExecuteResult, KernelError> {
        self.ensure_alive()?;
        let request_id = Uuid::now_v7().to_string();
        let rx = self.register_waiter(&request_id)?;
        self.write_request(BridgeRequest::Execute {
            id: request_id.clone(),
            code: code.into(),
        })?;

        let mut outputs = Vec::new();
        let deadline = std::time::Instant::now() + REQUEST_TIMEOUT;
        let result = (|| {
            loop {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() {
                    return Err(KernelError::Timeout);
                }
                let response = recv_response(&rx, remaining)?;
                match response {
                    BridgeResponse::Stream { name, text, .. } => {
                        outputs.push(KernelOutput::Stream { name, text });
                    }
                    BridgeResponse::ExecuteResult { data, .. } => {
                        outputs.push(KernelOutput::ExecuteResult { data });
                    }
                    BridgeResponse::DisplayData { data, .. } => {
                        outputs.push(KernelOutput::DisplayData { data });
                    }
                    BridgeResponse::Error {
                        ename,
                        evalue,
                        traceback,
                        ..
                    } => {
                        outputs.push(KernelOutput::Error {
                            ename,
                            evalue,
                            traceback,
                        });
                    }
                    BridgeResponse::Done { status, .. } => {
                        return Ok(ExecuteResult {
                            request_id: request_id.clone(),
                            status,
                            outputs,
                        });
                    }
                    BridgeResponse::BridgeError { message, .. } => {
                        return Err(KernelError::protocol(message));
                    }
                    BridgeResponse::Ready => continue,
                }
            }
        })();

        self.unregister_waiter(&request_id);
        result
    }

    /// Ask the bridge to interrupt the kernel (safe while `execute` waits).
    pub fn interrupt(&self) -> Result<(), KernelError> {
        self.ensure_alive()?;
        let request_id = Uuid::now_v7().to_string();
        let rx = self.register_waiter(&request_id)?;
        self.write_request(BridgeRequest::Interrupt {
            id: request_id.clone(),
        })?;
        let result = self.wait_for_done_on(&rx, &request_id, Duration::from_secs(15));
        self.unregister_waiter(&request_id);
        result
    }

    /// Request a graceful shutdown, then kill if needed.
    pub fn shutdown(&self) -> Result<(), KernelError> {
        if !self.inner.alive.load(Ordering::SeqCst) {
            return Ok(());
        }
        let request_id = Uuid::now_v7().to_string();
        if let Ok(rx) = self.register_waiter(&request_id) {
            if self
                .write_request(BridgeRequest::Shutdown {
                    id: request_id.clone(),
                })
                .is_ok()
            {
                let _ = self.wait_for_done_on(&rx, &request_id, Duration::from_secs(5));
            }
            self.unregister_waiter(&request_id);
        }
        self.kill()
    }

    /// Force-kill the bridge child.
    pub fn kill(&self) -> Result<(), KernelError> {
        if !self.inner.alive.swap(false, Ordering::SeqCst) {
            return Ok(());
        }
        if let Ok(mut child) = self.inner.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    pub fn is_alive(&self) -> bool {
        self.refresh_alive();
        self.inner.alive.load(Ordering::SeqCst)
    }

    fn register_waiter(&self, request_id: &str) -> Result<Receiver<BridgeResponse>, KernelError> {
        let (tx, rx) = mpsc::channel();
        let mut map = self
            .inner
            .waiters
            .lock()
            .map_err(|_| KernelError::DeadSession)?;
        map.insert(request_id.to_string(), tx);
        Ok(rx)
    }

    fn unregister_waiter(&self, request_id: &str) {
        if let Ok(mut map) = self.inner.waiters.lock() {
            map.remove(request_id);
        }
    }

    fn write_request(&self, request: BridgeRequest) -> Result<(), KernelError> {
        let line = request.to_line().map_err(KernelError::protocol)?;
        let mut stdin = self
            .inner
            .stdin
            .lock()
            .map_err(|_| KernelError::DeadSession)?;
        stdin.write_all(line.as_bytes()).map_err(KernelError::io)?;
        stdin.flush().map_err(KernelError::io)?;
        Ok(())
    }

    fn wait_for_done_on(
        &self,
        rx: &Receiver<BridgeResponse>,
        request_id: &str,
        timeout: Duration,
    ) -> Result<(), KernelError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(KernelError::Timeout);
            }
            match recv_response(rx, remaining)? {
                BridgeResponse::Done { status, .. } => {
                    if status == "ok" {
                        return Ok(());
                    }
                    return Err(KernelError::protocol(format!(
                        "request {request_id} finished with status {status}"
                    )));
                }
                BridgeResponse::BridgeError { message, .. } => {
                    return Err(KernelError::protocol(message));
                }
                _ => continue,
            }
        }
    }

    fn ensure_alive(&self) -> Result<(), KernelError> {
        self.refresh_alive();
        if self.inner.alive.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(KernelError::DeadSession)
        }
    }

    fn refresh_alive(&self) {
        if !self.inner.alive.load(Ordering::SeqCst) {
            return;
        }
        let Ok(mut child) = self.inner.child.lock() else {
            self.inner.alive.store(false, Ordering::SeqCst);
            return;
        };
        match child.try_wait() {
            Ok(Some(_)) => self.inner.alive.store(false, Ordering::SeqCst),
            Ok(None) => {}
            Err(_) => self.inner.alive.store(false, Ordering::SeqCst),
        }
    }
}

impl Drop for KernelSession {
    fn drop(&mut self) {
        // Only the last clone tears down the child.
        if Arc::strong_count(&self.inner) > 1 {
            return;
        }
        let _ = self.kill();
        if let Ok(mut join) = self.inner.reader_join.lock() {
            if let Some(handle) = join.take() {
                let _ = handle.join();
            }
        }
    }
}

fn recv_response(
    rx: &Receiver<BridgeResponse>,
    timeout: Duration,
) -> Result<BridgeResponse, KernelError> {
    match rx.recv_timeout(timeout) {
        Ok(response) => Ok(response),
        Err(RecvTimeoutError::Timeout) => Err(KernelError::Timeout),
        Err(RecvTimeoutError::Disconnected) => Err(KernelError::DeadSession),
    }
}

fn default_bridge_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bridge/lattice_ipykernel_bridge.py")
}

fn read_bridge_stdout<R: std::io::Read>(stdout: R, waiters: WaiterMap) {
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let Ok(line) = line else {
            break;
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let response = match BridgeResponse::from_line(trimmed) {
            Ok(response) => response,
            Err(err) => BridgeResponse::BridgeError {
                id: None,
                message: format!("invalid bridge JSON: {err}: {trimmed}"),
            },
        };

        let Ok(map) = waiters.lock() else {
            break;
        };
        match response.request_id() {
            Some(id) => {
                if let Some(tx) = map.get(id) {
                    let _ = tx.send(response);
                }
            }
            None => {
                // `ready` and anonymous bridge errors go to the ready waiter.
                if let Some(tx) = map.get("__ready__") {
                    let _ = tx.send(response);
                }
            }
        }
    }
}

/// Path to the shipped bridge script (for docs / diagnostics).
pub fn shipped_bridge_script() -> PathBuf {
    default_bridge_script_path()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn find_python3() -> Option<PathBuf> {
        std::env::var_os("PATH").and_then(|path| {
            for dir in std::env::split_paths(&path) {
                let candidate = dir.join("python3");
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            None
        })
    }

    fn write_mock_bridge(dir: &Path) -> PathBuf {
        let path = dir.join("mock_bridge.py");
        fs::write(
            &path,
            r#"#!/usr/bin/env python3
import json, sys
def emit(o):
    sys.stdout.write(json.dumps(o) + "\n")
    sys.stdout.flush()
emit({"type": "ready"})
for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    req = json.loads(line)
    t = req["type"]
    rid = req["id"]
    if t == "execute":
        code = req.get("code", "")
        emit({"type": "stream", "id": rid, "name": "stdout", "text": code + "\n"})
        if "raise" in code:
            emit({"type": "error", "id": rid, "ename": "RuntimeError", "evalue": "boom", "traceback": ["boom"]})
            emit({"type": "done", "id": rid, "status": "error"})
        else:
            emit({"type": "execute_result", "id": rid, "data": {"text/plain": "ok"}})
            emit({"type": "done", "id": rid, "status": "ok"})
    elif t == "interrupt":
        emit({"type": "done", "id": rid, "status": "ok"})
    elif t == "shutdown":
        emit({"type": "done", "id": rid, "status": "ok"})
        break
"#,
        )
        .expect("write mock");
        path
    }

    #[test]
    fn mock_bridge_execute_interrupt_shutdown() {
        let Some(python) = find_python3() else {
            eprintln!("skip: no python3 on PATH");
            return;
        };
        let dir = tempfile::tempdir().expect("tempdir");
        let script = write_mock_bridge(dir.path());
        let mut cmd = Command::new(&python);
        cmd.arg(&script)
            .current_dir(dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let session = KernelSession::spawn_command(cmd).expect("spawn mock");
        let result = session.execute("print(1)").expect("execute");
        assert_eq!(result.status, "ok");
        assert!(result.outputs.iter().any(|o| matches!(
            o,
            KernelOutput::Stream { text, .. } if text.contains("print(1)")
        )));
        assert!(result
            .outputs
            .iter()
            .any(|o| matches!(o, KernelOutput::ExecuteResult { .. })));

        session.interrupt().expect("interrupt");
        session.shutdown().expect("shutdown");
        assert!(!session.is_alive());
    }

    #[test]
    fn start_rejects_cwd_outside_workspace() {
        let root = tempfile::tempdir().expect("root");
        let outside = tempfile::tempdir().expect("outside");
        let err = match KernelSession::start(StartOptions::new(root.path(), outside.path())) {
            Err(err) => err,
            Ok(_) => panic!("start should deny cwd outside workspace"),
        };
        assert!(matches!(err, KernelError::CwdNotAllowed { .. }));
    }

    #[test]
    #[ignore = "requires ipykernel; set LATTICE_KERNEL_LIVE=1 and run with --ignored"]
    fn live_ipykernel_execute_print() {
        if std::env::var_os("LATTICE_KERNEL_LIVE").is_none() {
            eprintln!("skip live test: LATTICE_KERNEL_LIVE not set");
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let session =
            KernelSession::start(StartOptions::new(dir.path(), dir.path())).expect("start");
        let result = session
            .execute("print('lattice-kernel-live')")
            .expect("exec");
        assert_eq!(result.status, "ok");
        let joined: String = result
            .outputs
            .iter()
            .filter_map(|o| match o {
                KernelOutput::Stream { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            joined.contains("lattice-kernel-live"),
            "stdout missing marker: {result:?}"
        );
        session.shutdown().expect("shutdown");
    }
}

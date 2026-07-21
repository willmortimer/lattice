//! Kill-on-drop map of live kernel sessions.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::KernelError;
use crate::protocol::ExecuteResult;
use crate::session::{KernelSession, StartOptions};

/// Supervises multiple [`KernelSession`] values keyed by opaque session ids.
///
/// Dropping the map (or individual shutdown) kills remaining children.
#[derive(Default)]
pub struct KernelSessionMap {
    sessions: HashMap<String, KernelSession>,
    next_id: AtomicU64,
}

impl KernelSessionMap {
    pub fn new() -> Self {
        Self::default()
    }

    fn allocate_id(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed);
        format!("kernel-{n}")
    }

    /// Start a new bridge session under the workspace cwd gate.
    pub fn start(&mut self, opts: StartOptions) -> Result<String, KernelError> {
        let session = KernelSession::start(opts)?;
        let session_id = self.allocate_id();
        self.sessions.insert(session_id.clone(), session);
        Ok(session_id)
    }

    /// Insert an already-spawned session (tests / alternate hosts).
    pub fn insert_session(&mut self, session: KernelSession) -> String {
        let session_id = self.allocate_id();
        self.sessions.insert(session_id.clone(), session);
        session_id
    }

    /// Clone a session handle so callers can drop the map lock before blocking.
    pub fn get(&self, session_id: &str) -> Result<KernelSession, KernelError> {
        self.sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| KernelError::UnknownSession {
                session_id: session_id.to_string(),
            })
    }

    pub fn execute(
        &mut self,
        session_id: &str,
        code: impl Into<String>,
    ) -> Result<ExecuteResult, KernelError> {
        self.get(session_id)?.execute(code)
    }

    pub fn interrupt(&mut self, session_id: &str) -> Result<(), KernelError> {
        self.get(session_id)?.interrupt()
    }

    pub fn shutdown(&mut self, session_id: &str) -> Result<(), KernelError> {
        let session = self
            .sessions
            .remove(session_id)
            .ok_or_else(|| KernelError::UnknownSession {
                session_id: session_id.to_string(),
            })?;
        session.shutdown()
    }

    pub fn contains(&self, session_id: &str) -> bool {
        self.sessions.contains_key(session_id)
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

impl Drop for KernelSessionMap {
    fn drop(&mut self) {
        for (_, session) in self.sessions.drain() {
            let _ = session.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::KernelOutput;
    use crate::session::KernelSession;
    use std::fs;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

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

    fn write_mock_bridge(dir: &std::path::Path) -> PathBuf {
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
    t, rid = req["type"], req["id"]
    if t == "execute":
        emit({"type": "stream", "id": rid, "name": "stdout", "text": "mapped\n"})
        emit({"type": "done", "id": rid, "status": "ok"})
    elif t in ("interrupt", "shutdown"):
        emit({"type": "done", "id": rid, "status": "ok"})
        if t == "shutdown":
            break
"#,
        )
        .expect("write mock");
        path
    }

    #[test]
    fn map_unknown_session_errors() {
        let mut map = KernelSessionMap::new();
        let err = map.execute("missing", "1").expect_err("unknown");
        assert!(matches!(err, KernelError::UnknownSession { .. }));
        let err = map.interrupt("missing").expect_err("unknown");
        assert!(matches!(err, KernelError::UnknownSession { .. }));
        let err = map.shutdown("missing").expect_err("unknown");
        assert!(matches!(err, KernelError::UnknownSession { .. }));
    }

    #[test]
    fn map_start_execute_shutdown_with_mock() {
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

        let session = KernelSession::spawn_command(cmd).expect("spawn");
        let mut map = KernelSessionMap::new();
        let id = map.insert_session(session);
        assert!(map.contains(&id));
        assert_eq!(map.len(), 1);

        let result = map.execute(&id, "x").expect("execute");
        assert_eq!(result.status, "ok");
        assert!(result.outputs.iter().any(|o| matches!(
            o,
            KernelOutput::Stream { text, .. } if text.contains("mapped")
        )));

        map.shutdown(&id).expect("shutdown");
        assert!(!map.contains(&id));
        assert!(map.is_empty());
    }
}

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
#[cfg(test)]
use std::time::Duration;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use crate::error::TerminalError;
use crate::shell::default_shell;

/// Options for spawning a new PTY-backed shell session.
#[derive(Debug, Clone)]
pub struct SpawnOptions {
    /// Working directory; must already exist as a directory.
    pub cwd: PathBuf,
    /// Terminal columns.
    pub cols: u16,
    /// Terminal rows.
    pub rows: u16,
    /// Optional shell executable; defaults to [`default_shell`].
    pub shell: Option<PathBuf>,
}

impl SpawnOptions {
    pub fn new(cwd: impl Into<PathBuf>, cols: u16, rows: u16) -> Self {
        Self {
            cwd: cwd.into(),
            cols,
            rows,
            shell: None,
        }
    }
}

/// A live PTY session owning a child shell process.
///
/// Output bytes are delivered on the [`Receiver`] returned from [`Self::spawn`].
/// Dropping the session (or calling [`Self::kill`]) terminates the child.
pub struct TerminalSession {
    child: Box<dyn Child + Send + Sync>,
    master: Box<dyn MasterPty + Send>,
    writer: Option<Box<dyn Write + Send>>,
    reader_join: Option<JoinHandle<()>>,
    alive: bool,
}

impl TerminalSession {
    /// Spawn a shell in `opts.cwd` attached to a new PTY.
    ///
    /// Returns the session and a channel that receives raw PTY output chunks.
    /// The channel closes when the reader hits EOF (child exit / kill / writer
    /// drop).
    pub fn spawn(opts: SpawnOptions) -> Result<(Self, Receiver<Vec<u8>>), TerminalError> {
        validate_cwd(&opts.cwd)?;

        let shell = opts.shell.unwrap_or_else(default_shell);
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: opts.rows,
                cols: opts.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(TerminalError::spawn)?;

        let mut cmd = CommandBuilder::new(&shell);
        cmd.cwd(&opts.cwd);
        // Prefer a known TERM so interactive shells behave predictably under tests.
        cmd.env("TERM", "xterm-256color");

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(TerminalError::spawn)?;
        // Slave handle is only needed for spawn; drop so the child owns the tty.
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(TerminalError::spawn)?;
        let writer = pair.master.take_writer().map_err(TerminalError::spawn)?;

        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let reader_join = thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let session = Self {
            child,
            master: pair.master,
            writer: Some(writer),
            reader_join: Some(reader_join),
            alive: true,
        };
        Ok((session, rx))
    }

    /// Write raw bytes to the PTY (typically keyboard input).
    pub fn write(&mut self, data: &[u8]) -> Result<(), TerminalError> {
        self.refresh_alive()?;
        if !self.alive {
            return Err(TerminalError::DeadSession);
        }
        let writer = self.writer.as_mut().ok_or(TerminalError::DeadSession)?;
        writer.write_all(data).map_err(TerminalError::io)?;
        writer.flush().map_err(TerminalError::io)?;
        Ok(())
    }

    /// Resize the PTY window; does not panic on a live session.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), TerminalError> {
        self.refresh_alive()?;
        if !self.alive {
            return Err(TerminalError::DeadSession);
        }
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(TerminalError::io)?;
        Ok(())
    }

    /// Force-kill the child process and mark the session dead.
    pub fn kill(&mut self) -> Result<(), TerminalError> {
        if !self.alive {
            return Ok(());
        }
        // Dropping the writer signals EOF to the slave.
        self.writer.take();
        if let Err(err) = self.child.kill() {
            // Already-exited children can surface as errors; treat as cleaned up.
            let _ = err;
        }
        self.alive = false;
        // Brief wait so the OS reaps the child and the reader thread can EOF.
        let _ = self.child.try_wait();
        Ok(())
    }

    /// Whether the child is still running (polls without blocking).
    pub fn is_alive(&mut self) -> bool {
        self.refresh_alive().is_ok() && self.alive
    }

    fn refresh_alive(&mut self) -> Result<(), TerminalError> {
        if !self.alive {
            return Ok(());
        }
        match self.child.try_wait() {
            Ok(Some(_)) => {
                self.alive = false;
                self.writer.take();
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(err) => Err(TerminalError::io(err)),
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.kill();
        if let Some(handle) = self.reader_join.take() {
            // Reader should exit promptly after kill + writer drop; don't hang forever.
            let _ = handle.join();
        }
        // Keep master until after child teardown (portable-pty platform quirk).
        let _ = &self.master;
    }
}

fn validate_cwd(cwd: &Path) -> Result<(), TerminalError> {
    let meta = std::fs::metadata(cwd).map_err(|_| TerminalError::InvalidCwd {
        path: cwd.to_path_buf(),
    })?;
    if !meta.is_dir() {
        return Err(TerminalError::InvalidCwd {
            path: cwd.to_path_buf(),
        });
    }
    Ok(())
}

/// Collect PTY output until `predicate` matches or `timeout` elapses.
#[cfg(test)]
pub(crate) fn wait_for_output(
    rx: &Receiver<Vec<u8>>,
    timeout: Duration,
    mut predicate: impl FnMut(&[u8]) -> bool,
) -> Vec<u8> {
    let deadline = std::time::Instant::now() + timeout;
    let mut collected = Vec::new();
    while std::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        match rx.recv_timeout(remaining.min(Duration::from_millis(50))) {
            Ok(chunk) => {
                collected.extend_from_slice(&chunk);
                if predicate(&collected) {
                    return collected;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    collected
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    fn temp_cwd() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn spawn_rejects_nonexistent_cwd() {
        let missing = std::env::temp_dir().join(format!(
            "lattice-terminal-missing-{}",
            std::process::id()
        ));
        let err = match TerminalSession::spawn(SpawnOptions::new(&missing, 80, 24)) {
            Err(err) => err,
            Ok(_) => panic!("spawn should fail for missing cwd"),
        };
        match err {
            TerminalError::InvalidCwd { path } => assert_eq!(path, missing),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn spawn_rejects_file_as_cwd() {
        let dir = temp_cwd();
        let file_path = dir.path().join("not-a-dir");
        fs::write(&file_path, b"x").expect("write file");
        let err = match TerminalSession::spawn(SpawnOptions::new(&file_path, 80, 24)) {
            Err(err) => err,
            Ok(_) => panic!("spawn should fail for file cwd"),
        };
        assert!(matches!(err, TerminalError::InvalidCwd { .. }));
    }

    #[test]
    fn spawn_write_echo_and_read_output() {
        let dir = temp_cwd();
        let (mut session, rx) =
            TerminalSession::spawn(SpawnOptions::new(dir.path(), 80, 24)).expect("spawn");

        // Give the shell a moment to start before sending input (macOS PTY quirk).
        thread::sleep(Duration::from_millis(50));
        session
            .write(b"printf 'lattice-pty-ok\\n'\n")
            .expect("write");

        let output = wait_for_output(&rx, Duration::from_secs(5), |buf| {
            String::from_utf8_lossy(buf).contains("lattice-pty-ok")
        });
        let text = String::from_utf8_lossy(&output);
        assert!(
            text.contains("lattice-pty-ok"),
            "expected marker in output, got: {text:?}"
        );
        session.kill().expect("kill");
    }

    #[test]
    fn resize_does_not_panic_on_live_session() {
        let dir = temp_cwd();
        let (mut session, _rx) =
            TerminalSession::spawn(SpawnOptions::new(dir.path(), 80, 24)).expect("spawn");
        session.resize(120, 40).expect("resize");
        session.resize(40, 12).expect("resize again");
        assert!(session.is_alive());
        session.kill().expect("kill");
    }

    #[test]
    fn kill_marks_session_dead_and_rejects_write() {
        let dir = temp_cwd();
        let (mut session, _rx) =
            TerminalSession::spawn(SpawnOptions::new(dir.path(), 80, 24)).expect("spawn");
        session.kill().expect("kill");
        assert!(!session.is_alive());
        let err = session.write(b"echo after-kill\n").expect_err("write");
        assert!(matches!(err, TerminalError::DeadSession));
    }

    #[test]
    fn drop_cleans_up_without_panic() {
        let dir = temp_cwd();
        let (session, _rx) =
            TerminalSession::spawn(SpawnOptions::new(dir.path(), 80, 24)).expect("spawn");
        drop(session);
    }
}

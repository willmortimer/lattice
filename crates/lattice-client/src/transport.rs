//! Transport-neutral daemon IPC connect helpers.
//!
//! Handshake and framed envelopes stay above this layer. Platform endpoints:
//! - Unix: UDS at `{data}/Lattice/run/latticed.sock`
//! - Windows: named pipe `\\.\pipe\lattice-latticed-<user>` (no TCP)

use std::io;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use tokio::net::UnixStream;

/// Connected daemon stream; [`tokio::io::split`] into read/write halves.
#[cfg(unix)]
pub type DaemonStream = UnixStream;

/// Connected daemon stream; [`tokio::io::split`] into read/write halves.
#[cfg(windows)]
pub type DaemonStream = tokio::net::windows::named_pipe::NamedPipeClient;

/// Format the default Unix UDS path under a platform data directory.
///
/// Result: `{data_dir}/Lattice/run/latticed.sock`.
pub fn format_unix_socket_endpoint(data_dir: impl AsRef<Path>) -> PathBuf {
    data_dir
        .as_ref()
        .join("Lattice")
        .join("run")
        .join("latticed.sock")
}

/// Format the Windows named-pipe endpoint for `username`.
///
/// Result: `\\.\pipe\lattice-latticed-<username>`.
pub fn format_windows_pipe_endpoint(username: &str) -> PathBuf {
    PathBuf::from(format!(r"\\.\pipe\lattice-latticed-{username}"))
}

/// Format the Windows named-pipe endpoint for `lattice-embed-host`.
///
/// Result: `\\.\pipe\lattice-embed-host-<username>`. Distinct from
/// [`format_windows_pipe_endpoint`] so latticed and embed-host never share a pipe.
pub fn format_windows_embed_host_pipe_endpoint(username: &str) -> PathBuf {
    PathBuf::from(format!(r"\\.\pipe\lattice-embed-host-{username}"))
}

/// Default daemon IPC endpoint for this platform.
pub fn default_endpoint() -> PathBuf {
    #[cfg(unix)]
    {
        let data_dir = std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| {
                if cfg!(target_os = "macos") {
                    home.join("Library").join("Application Support")
                } else {
                    home.join(".local").join("share")
                }
            })
            .unwrap_or_else(std::env::temp_dir);
        format_unix_socket_endpoint(data_dir)
    }
    #[cfg(windows)]
    {
        let username = std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_else(|_| "unknown".into());
        format_windows_pipe_endpoint(&username)
    }
}

/// Connect to a daemon endpoint (UDS path or Windows named-pipe name).
pub async fn connect(endpoint: impl AsRef<Path>) -> io::Result<DaemonStream> {
    #[cfg(unix)]
    {
        UnixStream::connect(endpoint.as_ref()).await
    }
    #[cfg(windows)]
    {
        connect_windows_pipe(endpoint.as_ref()).await
    }
}

#[cfg(windows)]
async fn connect_windows_pipe(endpoint: &Path) -> io::Result<DaemonStream> {
    use std::time::Duration;

    use tokio::net::windows::named_pipe::ClientOptions;
    use tokio::time;

    // ERROR_PIPE_BUSY — server exists but has no free instance yet.
    const ERROR_PIPE_BUSY: i32 = 231;

    loop {
        match ClientOptions::new().open(endpoint) {
            Ok(client) => return Ok(client),
            Err(err) if err.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                time::sleep(Duration::from_millis(50)).await;
            }
            Err(err) => return Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_socket_path_uses_lattice_run_latticed_sock() {
        let path = format_unix_socket_endpoint("/var/data");
        assert_eq!(path, PathBuf::from("/var/data/Lattice/run/latticed.sock"));
    }

    #[test]
    fn windows_pipe_name_includes_username() {
        let path = format_windows_pipe_endpoint("alice");
        assert_eq!(path, PathBuf::from(r"\\.\pipe\lattice-latticed-alice"));
    }

    #[test]
    fn windows_pipe_name_preserves_spaces_in_username() {
        let path = format_windows_pipe_endpoint("Will Mortimer");
        assert_eq!(
            path,
            PathBuf::from(r"\\.\pipe\lattice-latticed-Will Mortimer")
        );
    }

    #[test]
    fn windows_embed_host_pipe_is_distinct_from_latticed() {
        let path = format_windows_embed_host_pipe_endpoint("Will Mortimer");
        assert_eq!(
            path,
            PathBuf::from(r"\\.\pipe\lattice-embed-host-Will Mortimer")
        );
        assert_ne!(path, format_windows_pipe_endpoint("Will Mortimer"));
    }

    #[cfg(unix)]
    #[test]
    fn default_endpoint_is_unix_socket() {
        let path = default_endpoint();
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("latticed.sock")
        );
    }

    #[cfg(windows)]
    #[test]
    fn default_endpoint_is_named_pipe() {
        let path = default_endpoint();
        let s = path.to_string_lossy();
        assert!(s.starts_with(r"\\.\pipe\lattice-latticed-"));
    }
}

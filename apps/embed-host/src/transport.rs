//! Platform IPC for embed-host (UDS on Unix, named pipes on Windows).

use std::io;
use std::path::Path;

#[cfg(unix)]
use tokio::net::UnixStream;

/// Connected embed-host stream (`AsyncRead + AsyncWrite`).
#[cfg(unix)]
pub type EmbedHostStream = UnixStream;

/// Connected embed-host stream (`AsyncRead + AsyncWrite`).
#[cfg(windows)]
pub type EmbedHostStream = tokio::net::windows::named_pipe::NamedPipeClient;

/// Connect to an embed-host endpoint.
pub async fn connect(endpoint: impl AsRef<Path>) -> io::Result<EmbedHostStream> {
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
async fn connect_windows_pipe(endpoint: &Path) -> io::Result<EmbedHostStream> {
    use std::time::Duration;

    use tokio::net::windows::named_pipe::ClientOptions;
    use tokio::time;

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

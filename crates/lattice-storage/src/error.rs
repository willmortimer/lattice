use std::path::PathBuf;

/// Errors from the storage layer. Follows `lattice-core`'s thiserror style:
/// I/O carries the offending path, and revision conflicts are their own
/// variant so callers can react to lost-update races specifically.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// A workspace-relative path resolved outside the store root
    /// (an absolute path, or one that climbs past the root with `..`).
    #[error("path {path} escapes the workspace root")]
    OutsideWorkspace { path: PathBuf },

    /// The on-disk revision did not match what the caller expected to be
    /// present. `expected`/`found` are content hashes (`None` = absent).
    #[error("revision mismatch at {path}: expected {expected:?}, found {found:?}")]
    RevisionMismatch {
        path: PathBuf,
        expected: Option<String>,
        found: Option<String>,
    },

    #[error("recovery journal error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }

    /// A not-found I/O error for `path`, used by stores that model absence
    /// in memory rather than via the filesystem.
    pub(crate) fn not_found(path: impl Into<PathBuf>) -> Self {
        Error::io(path, std::io::Error::from(std::io::ErrorKind::NotFound))
    }
}

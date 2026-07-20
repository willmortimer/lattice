use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("path {path} is outside workspace root {root}")]
    PathNotAllowed { path: PathBuf, root: PathBuf },

    #[error("duckdb error: {0}")]
    DuckDb(#[from] duckdb::Error),

    #[error("{0}")]
    Message(String),
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn path_not_allowed(path: impl Into<PathBuf>, root: impl Into<PathBuf>) -> Self {
        Error::PathNotAllowed {
            path: path.into(),
            root: root.into(),
        }
    }

    pub(crate) fn message(message: impl Into<String>) -> Self {
        Error::Message(message.into())
    }
}

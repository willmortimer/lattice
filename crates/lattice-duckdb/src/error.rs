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

    #[error("query cancelled")]
    Cancelled,

    #[error("sqlite error at {path}: {source}")]
    Sqlite {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },

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

    pub(crate) fn sqlite(path: impl Into<PathBuf>, source: rusqlite::Error) -> Self {
        Error::Sqlite {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn message(message: impl Into<String>) -> Self {
        Error::Message(message.into())
    }

    /// True when DuckDB reported an interrupt / abort for the running query.
    pub fn is_cancelled(&self) -> bool {
        match self {
            Error::Cancelled => true,
            Error::DuckDb(err) => duckdb_error_is_interrupt(err),
            Error::Message(message) => message_looks_cancelled(message),
            _ => false,
        }
    }
}

fn duckdb_error_is_interrupt(err: &duckdb::Error) -> bool {
    message_looks_cancelled(&err.to_string())
}

fn message_looks_cancelled(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("interrupt") || lower.contains("cancelled") || lower.contains("canceled")
}

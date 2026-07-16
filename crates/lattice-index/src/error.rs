use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("workspace error: {0}")]
    Workspace(#[from] lattice_core::Error),

    #[error("index database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("path {path} is not valid UTF-8")]
    NonUtf8Path { path: PathBuf },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}

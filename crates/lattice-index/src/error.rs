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

    #[error("resource runtime error: {0}")]
    Runtime(#[from] lattice_core::ResourceRuntimeError),

    #[error("index database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("vector index error: {0}")]
    Vector(#[from] crate::vector::VectorIndexError),

    #[error("embedding error: {0}")]
    Embedding(#[from] lattice_embedding::EmbeddingError),

    #[error("path {path} is not valid UTF-8")]
    NonUtf8Path { path: PathBuf },

    #[error("json serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("index operation cancelled")]
    Cancelled,

    #[error("embedding namespace not found: {0}")]
    NamespaceNotFound(i64),
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

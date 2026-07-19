use std::io;
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Workspace(#[from] lattice_core::Error),
    #[error(transparent)]
    Commands(#[from] lattice_commands::Error),
    #[error(transparent)]
    Index(#[from] lattice_index::Error),
    #[error("workspace session not found for id {0}")]
    SessionNotFound(String),
}

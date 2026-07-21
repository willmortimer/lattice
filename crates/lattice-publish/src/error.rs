use std::path::PathBuf;

/// Errors from static publish / export.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{0}")]
    Message(String),

    #[error(transparent)]
    Core(#[from] lattice_core::Error),

    #[error(transparent)]
    Data(#[from] lattice_data::Error),

    #[error(transparent)]
    Artifact(#[from] lattice_commands::ArtifactError),

    #[error(transparent)]
    Theme(#[from] lattice_theme::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

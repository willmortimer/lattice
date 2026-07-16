use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("no Lattice workspace found at or above {0} (missing lattice.yaml)")]
    WorkspaceNotFound(PathBuf),

    #[error("{path} already contains a Lattice workspace")]
    WorkspaceExists { path: PathBuf },

    #[error("invalid manifest {path}: {message}")]
    InvalidManifest { path: PathBuf, message: String },

    #[error("failed to parse {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    /// A [`crate::WorkspaceWatcher`] failed to start or attach a filesystem
    /// watch at `path`.
    #[error("failed to watch {path}: {source}")]
    Watch {
        path: PathBuf,
        #[source]
        source: notify::Error,
    },
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}

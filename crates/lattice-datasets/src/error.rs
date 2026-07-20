use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid dataset package at {path}: {message}")]
    InvalidPackage { path: PathBuf, message: String },

    #[error("failed to parse {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    #[error("parquet error at {path}: {source}")]
    Parquet {
        path: PathBuf,
        #[source]
        source: parquet::errors::ParquetError,
    },

    #[error("arrow error at {path}: {message}")]
    Arrow { path: PathBuf, message: String },

    #[error("csv error at {path}: {message}")]
    Csv { path: PathBuf, message: String },

    #[error("sqlite error at {path}: {source}")]
    Sqlite {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },

    #[error("{message}")]
    InvalidArgument { message: String },
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn invalid_package(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Error::InvalidPackage {
            path: path.into(),
            message: message.into(),
        }
    }

    pub(crate) fn parquet(
        path: impl Into<PathBuf>,
        source: parquet::errors::ParquetError,
    ) -> Self {
        Error::Parquet {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn arrow(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Error::Arrow {
            path: path.into(),
            message: message.into(),
        }
    }

    pub(crate) fn csv(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Error::Csv {
            path: path.into(),
            message: message.into(),
        }
    }

    pub(crate) fn sqlite(path: impl Into<PathBuf>, source: rusqlite::Error) -> Self {
        Error::Sqlite {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn invalid_argument(message: impl Into<String>) -> Self {
        Error::InvalidArgument {
            message: message.into(),
        }
    }
}

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid data package at {path}: {message}")]
    InvalidPackage { path: PathBuf, message: String },

    #[error("failed to parse {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("table {table}: {message}")]
    Table { table: String, message: String },
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

    pub(crate) fn table(table: impl Into<String>, message: impl Into<String>) -> Self {
        Error::Table {
            table: table.into(),
            message: message.into(),
        }
    }
}

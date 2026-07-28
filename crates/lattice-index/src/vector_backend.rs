//! Vector storage backend selection via [`ENV_VECTOR_BACKEND`].

use std::env;
use std::fmt;
use std::str::FromStr;

/// Environment variable selecting the vector index backend.
pub const ENV_VECTOR_BACKEND: &str = "LATTICE_VECTOR_BACKEND";

/// Supported vector index backends for semantic search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorBackend {
    /// SQLite `chunk_vectors` BLOB exact-scan (default).
    Sqlite,
    /// LanceDB search-elements dataset via [`lattice_lance::EmbeddedLanceStore`].
    Lance,
}

impl fmt::Display for VectorBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite => write!(f, "sqlite"),
            Self::Lance => write!(f, "lance"),
        }
    }
}

impl FromStr for VectorBackend {
    type Err = VectorBackendParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        parse_vector_backend(value)
    }
}

/// Error returned when [`ENV_VECTOR_BACKEND`] is set to an unsupported value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorBackendParseError {
    pub value: String,
}

impl fmt::Display for VectorBackendParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid {ENV_VECTOR_BACKEND} value {:?}: expected \"sqlite\" or \"lance\"",
            self.value
        )
    }
}

impl std::error::Error for VectorBackendParseError {}

/// Read the vector backend from [`ENV_VECTOR_BACKEND`].
///
/// Unset, empty, or `sqlite` (case-insensitive) selects [`VectorBackend::Sqlite`].
/// `lance` selects [`VectorBackend::Lance`]. Any other value is an error.
pub fn vector_backend_from_env() -> Result<VectorBackend, VectorBackendParseError> {
    match env::var(ENV_VECTOR_BACKEND) {
        Ok(value) => parse_vector_backend(&value),
        Err(env::VarError::NotPresent) => Ok(VectorBackend::Sqlite),
        Err(env::VarError::NotUnicode(_)) => Err(VectorBackendParseError {
            value: String::from("<non-unicode>"),
        }),
    }
}

fn parse_vector_backend(value: &str) -> Result<VectorBackend, VectorBackendParseError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "sqlite" => Ok(VectorBackend::Sqlite),
        "lance" => Ok(VectorBackend::Lance),
        _ => Err(VectorBackendParseError {
            value: value.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sqlite_variants() {
        assert_eq!(parse_vector_backend("").unwrap(), VectorBackend::Sqlite);
        assert_eq!(parse_vector_backend("sqlite").unwrap(), VectorBackend::Sqlite);
        assert_eq!(parse_vector_backend("SQLITE").unwrap(), VectorBackend::Sqlite);
        assert_eq!(parse_vector_backend("  Sqlite  ").unwrap(), VectorBackend::Sqlite);
    }

    #[test]
    fn parse_lance() {
        assert_eq!(parse_vector_backend("lance").unwrap(), VectorBackend::Lance);
        assert_eq!(parse_vector_backend("LANCE").unwrap(), VectorBackend::Lance);
    }

    #[test]
    fn rejects_unknown_backend() {
        let err = parse_vector_backend("qdrant").unwrap_err();
        assert_eq!(err.value, "qdrant");
    }
}

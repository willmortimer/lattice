use std::path::Path;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("http: {0}")]
    Http(String),
    #[error("git: {0}")]
    Git(String),
    #[error("auth: {0}")]
    Auth(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("sandbox: {0}")]
    Sandbox(String),
    #[error("credential store: {0}")]
    Credentials(String),
}

impl Error {
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }

    pub fn http(msg: impl Into<String>) -> Self {
        Self::Http(msg.into())
    }

    pub fn git(msg: impl Into<String>) -> Self {
        Self::Git(msg.into())
    }

    pub fn auth(msg: impl Into<String>) -> Self {
        Self::Auth(msg.into())
    }

    pub fn sandbox(msg: impl Into<String>) -> Self {
        Self::Sandbox(msg.into())
    }

    pub fn credentials(msg: impl Into<String>) -> Self {
        Self::Credentials(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn path_display(path: &Path) -> String {
    path.display().to_string()
}

pub(crate) fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub(crate) fn remove_dir_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    ensure_parent(path)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

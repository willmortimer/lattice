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
    #[error("lease json error at {path}: {source}")]
    LeaseJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "workspace lease held by {owner} (pid={pid}, process_start={process_start}, instance_id={instance_id})"
    )]
    LeaseHeld {
        owner: String,
        pid: u32,
        process_start: u64,
        instance_id: String,
    },
    #[error("workspace lease not held: {detail}")]
    LeaseNotHeld { detail: String },
    #[error(transparent)]
    Workspace(#[from] lattice_core::Error),
    #[error(transparent)]
    Commands(#[from] lattice_commands::Error),
    #[error(transparent)]
    Index(#[from] lattice_index::Error),
    #[error("workspace session not found for id {0}")]
    SessionNotFound(String),
    #[error("failed to start workspace watcher at {path}: {source}")]
    Watch {
        path: PathBuf,
        #[source]
        source: lattice_core::Error,
    },
}

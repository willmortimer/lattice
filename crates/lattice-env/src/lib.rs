//! Shared environment resolution for native kernels and `*.task/` runs.
//!
//! # Resolution order and providers
//!
//! | Request | Behavior |
//! |---|---|
//! | [`EnvKind::System`] | Find `python3`, then `python`, on `PATH`. |
//! | [`EnvKind::UvProject`] | Require `pyproject.toml` or `uv.lock` in `project_dir`. |
//! | [`EnvKind::Nix`] | Stub: always [`EnvError::Unavailable`] (filled in by J6). |
//!
//! ## `uv-project` approach
//!
//! Resolve the interpreter with `uv python find --directory <project_dir>`.
//! Prefer this over `uv run which python`: it returns the interpreter path
//! without syncing or executing the project environment. Missing `uv` is
//! [`EnvError::MissingTool`]. Requesting [`EnvKind::Nix`] never silently
//! falls back to system Python.

mod error;
mod path_util;
mod provider;

pub use error::EnvError;
pub use provider::{resolve, EnvKind, EnvProvider, ResolvedEnv};

pub type Result<T> = std::result::Result<T, EnvError>;

//! Shared environment resolution for native kernels and `*.task/` runs.
//!
//! # Resolution order and providers
//!
//! | Request | Behavior |
//! |---|---|
//! | [`EnvKind::System`] | Find `python3`, then `python`, on `PATH`. |
//! | [`EnvKind::UvProject`] | Require `pyproject.toml` or `uv.lock` in `project_dir`. |
//! | [`EnvKind::Nix`] | Require `nix` on `PATH` and `flake.nix` or `shell.nix` under `root`. |
//!
//! ## `uv-project` approach
//!
//! Resolve the interpreter with `uv python find --directory <project_dir>`.
//! Prefer this over `uv run which python`: it returns the interpreter path
//! without syncing or executing the project environment. Missing `uv` is
//! [`EnvError::MissingTool`].
//!
//! ## `nix` approach
//!
//! Missing `nix` is [`EnvError::MissingTool`]. Missing both `flake.nix` and
//! `shell.nix` is [`EnvError::Unavailable`]. When a nix file is present, run
//! `nix print-dev-env --json` (flake: `path:<root>#`; classic:
//! `-f shell.nix`), take `variables.PATH.value`, and find `python3` then
//! `python` **only** on that PATH. Failures from nix or a PATH without
//! Python are [`EnvError::ToolFailed`] / [`EnvError::Unavailable`] — never a
//! silent fallback to system Python when [`EnvKind::Nix`] was requested.

mod error;
mod path_util;
mod provider;

pub use error::EnvError;
pub use provider::{resolve, EnvKind, EnvProvider, ResolvedEnv};

pub type Result<T> = std::result::Result<T, EnvError>;

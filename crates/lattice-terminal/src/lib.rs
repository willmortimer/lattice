//! Tauri-free PTY session core for Lattice's embedded terminal (ADR 0039).
//!
//! This crate wraps [`portable_pty`] for spawn, write, resize, kill, and a
//! background read loop. Capability policy and Tauri IPC live in callers.

mod error;
mod session;
mod shell;

pub use error::TerminalError;
pub use session::{SpawnOptions, TerminalSession};
pub use shell::default_shell;

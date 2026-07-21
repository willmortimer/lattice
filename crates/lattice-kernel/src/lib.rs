//! Out-of-process Jupyter/`ipykernel` supervisor for Lattice (ADR 0009).
//!
//! Spawns a Python stdio JSON-lines bridge that owns `jupyter_client` +
//! `ipykernel`. The trusted Rust process never embeds CPython and never speaks
//! ZMQ. See the crate `README.md` for the wire protocol.

mod cwd;
mod discover;
mod error;
mod map;
mod protocol;
mod session;

pub use cwd::resolve_cwd_under_workspace;
pub use discover::{inject_lattice_python_sdk, shipped_lattice_py_dir, PythonLauncher};
pub use error::KernelError;
pub use map::KernelSessionMap;
pub use protocol::{
    BridgeRequest, BridgeResponse, ExecuteResult, KernelOutput,
};
pub use session::{shipped_bridge_script, KernelSession, StartOptions};

//! Semantic command and transaction core for Lattice (ADR 0007).
//!
//! Every mutation in the product — desktop GUI, CLI, future local API and
//! MCP — is expressed as a [`Command`], grouped into an atomic
//! [`Transaction`], and applied through the command engine.

mod command;
mod error;
mod trash;

pub use command::{
    Command, CommandOutcome, HistoryEntry, Transaction, TransactionReceipt, UndoReport,
};
pub use error::Error;
pub use trash::TrashPolicy;

pub type Result<T> = std::result::Result<T, Error>;

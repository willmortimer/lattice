//! Isolated voice inference host for Lattice (ADR 0043 / D5).
//!
//! The host listens on a private Unix-domain socket and speaks length-delimited
//! [`lattice_protocol::Envelope`] frames — the same framing and voice message
//! shapes as the daemon control plane. Host-admin Status / Unload RPCs live on
//! reserved request fields so they stay out of the workspace request surface.
//!
//! The default `fake` backend wraps [`lattice_voice::NullSpeechProvider`] and
//! never downloads Parakeet models. Enable `--features fluidaudio` for the
//! real FluidAudio path via `lattice-voice-macos`.

mod backend;
mod client;
mod convert;
mod error;
mod server;

pub use backend::BackendKind;
pub use client::{
    collect_transcript_texts, socket_path_in, wait_for_final_transcript, VoiceHostClient,
};
pub use error::VoiceHostError;
pub use server::{run_server, HostConfig, HostState};

/// Re-export the shared wire protocol version.
pub use lattice_protocol::PROTOCOL_VERSION;

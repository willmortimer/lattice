//! Provider-neutral audio capture types for Lattice voice.
//!
//! Canonical PCM for FluidAudio handoff is 16 kHz mono Float32
//! ([docs/voice/audio-capture.md](../../docs/voice/audio-capture.md)).
//! This crate owns ring/pre-roll contracts and the `CaptureProvider` trait;
//! platform capture lives in `lattice-audio-macos` (and future backends).

mod error;
mod event;
mod format;
mod frame;
mod provider;
mod ring;

pub use error::CaptureError;
pub use event::{CaptureEvent, GapEvent};
pub use format::{AudioFormat, SampleFormat, CANONICAL_AUDIO_FORMAT, DEFAULT_PRE_ROLL_MS};
pub use frame::{AudioDiagnostics, AudioFrame};
pub use provider::{CaptureEventSender, CaptureProvider, SyntheticCaptureProvider};
pub use ring::{PreRollBuffer, RingBuffer};

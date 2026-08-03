//! Non-macOS stub: FluidAudio voice-host is unavailable.

use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use lattice_protocol::{Event, Request, Response};
use tokio::sync::broadcast;

use crate::error::{Error, Result};

pub const ENV_VOICE_HOST_SOCKET: &str = "LATTICE_VOICE_HOST_SOCKET";
pub const ENV_VOICE_FAKE: &str = "LATTICE_VOICE_FAKE";
pub const ENV_VOICE_HOST_BIN: &str = "LATTICE_VOICE_HOST_BIN";
pub const ENV_VOICE_MODEL_CACHE: &str = "LATTICE_VOICE_MODEL_CACHE";

#[derive(Debug, Clone)]
pub enum VoiceProviderMode {
    ExternalSocket { socket: PathBuf },
    SpawnHost {
        binary: PathBuf,
        socket: PathBuf,
        fake: bool,
    },
}

impl VoiceProviderMode {
    pub fn from_env() -> Option<Self> {
        None
    }
}

pub fn resolve_voice_host_bin() -> Option<PathBuf> {
    None
}

pub struct VoiceController {
    _next_event_seq: Arc<AtomicU64>,
}

impl VoiceController {
    pub async fn start(_mode: VoiceProviderMode) -> Result<Arc<Self>> {
        Err(Error::Message(
            "voice-host is macOS-only in this build".into(),
        ))
    }

    pub fn attach_event_fanout(
        &self,
        _event_tx: broadcast::Sender<Event>,
        _next_event_seq: Arc<AtomicU64>,
    ) {
    }

    pub fn shutdown(&self) {}

    pub async fn handle_request(
        &self,
        _req: Request,
    ) -> std::result::Result<Response, lattice_protocol::Error> {
        Err(lattice_protocol::Error {
            code: "voice_unavailable".into(),
            message: "voice-host is macOS-only in this build".into(),
            details: None,
        })
    }
}

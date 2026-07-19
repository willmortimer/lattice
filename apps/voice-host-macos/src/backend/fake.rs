use std::sync::Arc;

use lattice_voice::{NullSpeechProvider, SpeechProvider};

/// Deterministic in-process backend for CI and protocol tests.
pub struct FakeBackend;

impl FakeBackend {
    pub fn new() -> Arc<dyn SpeechProvider> {
        Arc::new(NullSpeechProvider::new())
    }
}

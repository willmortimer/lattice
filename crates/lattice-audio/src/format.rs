//! Canonical capture sample format.

/// PCM sample encoding produced by capture adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleFormat {
    /// IEEE Float32 little-endian (FluidAudio bridge contract).
    F32Le,
    /// Signed 16-bit little-endian (protocol-neutral; not the macOS capture default).
    I16Le,
}

/// Packed PCM layout for a capture stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioFormat {
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub sample_format: SampleFormat,
}

impl AudioFormat {
    /// Bytes per interleaved frame (one sample per channel).
    #[must_use]
    pub const fn bytes_per_frame(self) -> usize {
        let sample_bytes = match self.sample_format {
            SampleFormat::F32Le => 4,
            SampleFormat::I16Le => 2,
        };
        sample_bytes * self.channels as usize
    }

    /// Samples (across all channels) for `duration_ms` of audio.
    #[must_use]
    pub const fn sample_count_for_ms(self, duration_ms: u32) -> usize {
        (self.sample_rate_hz as usize * duration_ms as usize * self.channels as usize) / 1000
    }
}

/// Canonical FluidAudio handoff format: 16 kHz mono Float32.
pub const CANONICAL_AUDIO_FORMAT: AudioFormat = AudioFormat {
    sample_rate_hz: 16_000,
    channels: 1,
    sample_format: SampleFormat::F32Le,
};

/// Default pre-roll duration while dictation is armed (~300 ms).
pub const DEFAULT_PRE_ROLL_MS: u32 = 300;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_layout() {
        assert_eq!(CANONICAL_AUDIO_FORMAT.sample_rate_hz, 16_000);
        assert_eq!(CANONICAL_AUDIO_FORMAT.channels, 1);
        assert_eq!(CANONICAL_AUDIO_FORMAT.sample_format, SampleFormat::F32Le);
        assert_eq!(CANONICAL_AUDIO_FORMAT.bytes_per_frame(), 4);
        assert_eq!(CANONICAL_AUDIO_FORMAT.sample_count_for_ms(300), 4_800);
        assert_eq!(CANONICAL_AUDIO_FORMAT.sample_count_for_ms(20), 320);
    }
}

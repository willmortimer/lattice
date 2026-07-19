//! Full-utterance PCM buffer for optional offline re-decode (ADR 0007).

use bytes::Bytes;

use crate::error::SpeechError;
use crate::protocol::{AudioChunk, AudioSampleFormat};

/// Accumulates mono PCM for one utterance while a speech session is active.
///
/// Production finals still come from streaming flush by default; this buffer
/// enables an independent/same-family offline path when one is implemented.
#[derive(Debug, Clone, Default)]
pub struct UtteranceAudioBuffer {
    samples: Vec<f32>,
    sample_rate_hz: Option<u32>,
    channels: Option<u8>,
    /// Number of non-empty PCM frames accepted (for tests / diagnostics).
    frames_accepted: u64,
}

/// Immutable snapshot used at finalization.
#[derive(Debug, Clone, PartialEq)]
pub struct FrozenUtteranceAudio {
    samples: Vec<f32>,
    sample_rate_hz: u32,
    channels: u8,
    frames_accepted: u64,
}

impl UtteranceAudioBuffer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    #[must_use]
    pub fn len_samples(&self) -> usize {
        self.samples.len()
    }

    #[must_use]
    pub fn frames_accepted(&self) -> u64 {
        self.frames_accepted
    }

    #[must_use]
    pub fn sample_rate_hz(&self) -> Option<u32> {
        self.sample_rate_hz
    }

    #[must_use]
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// Append Float32 mono samples, enforcing a consistent format for the utterance.
    pub fn push_f32(
        &mut self,
        samples: &[f32],
        sample_rate_hz: u32,
        channels: u8,
    ) -> Result<(), SpeechError> {
        if channels == 0 {
            return Err(SpeechError::provider("utterance buffer requires channels > 0"));
        }
        if sample_rate_hz == 0 {
            return Err(SpeechError::provider(
                "utterance buffer requires sample_rate_hz > 0",
            ));
        }
        if let Some(existing) = self.sample_rate_hz {
            if existing != sample_rate_hz {
                return Err(SpeechError::provider(format!(
                    "utterance sample rate changed mid-session ({existing} -> {sample_rate_hz})"
                )));
            }
        }
        if let Some(existing) = self.channels {
            if existing != channels {
                return Err(SpeechError::provider(format!(
                    "utterance channel count changed mid-session ({existing} -> {channels})"
                )));
            }
        }

        self.sample_rate_hz = Some(sample_rate_hz);
        self.channels = Some(channels);
        if samples.is_empty() {
            return Ok(());
        }
        self.samples.extend_from_slice(samples);
        self.frames_accepted = self.frames_accepted.saturating_add(1);
        Ok(())
    }

    /// Decode and append a wire [`AudioChunk`] (F32 or I16 LE mono).
    pub fn push_chunk(&mut self, chunk: &AudioChunk) -> Result<(), SpeechError> {
        let samples = decode_chunk_samples(chunk)?;
        self.push_f32(&samples, chunk.sample_rate_hz, chunk.channels)
    }

    /// Freeze the buffer for offline re-decode. Empty audio is allowed.
    #[must_use]
    pub fn freeze(self) -> FrozenUtteranceAudio {
        FrozenUtteranceAudio {
            samples: self.samples,
            sample_rate_hz: self.sample_rate_hz.unwrap_or(16_000),
            channels: self.channels.unwrap_or(1),
            frames_accepted: self.frames_accepted,
        }
    }

    pub fn clear(&mut self) {
        self.samples.clear();
        self.sample_rate_hz = None;
        self.channels = None;
        self.frames_accepted = 0;
    }
}

impl FrozenUtteranceAudio {
    #[must_use]
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    #[must_use]
    pub fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    #[must_use]
    pub fn channels(&self) -> u8 {
        self.channels
    }

    #[must_use]
    pub fn frames_accepted(&self) -> u64 {
        self.frames_accepted
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    #[must_use]
    pub fn len_samples(&self) -> usize {
        self.samples.len()
    }
}

fn decode_chunk_samples(chunk: &AudioChunk) -> Result<Vec<f32>, SpeechError> {
    match chunk.sample_format {
        AudioSampleFormat::F32 => decode_f32_payload(&chunk.payload),
        AudioSampleFormat::I16Le => decode_i16_le_payload(&chunk.payload),
    }
}

fn decode_f32_payload(payload: &Bytes) -> Result<Vec<f32>, SpeechError> {
    if payload.len() % 4 != 0 {
        return Err(SpeechError::provider("F32 payload length is not aligned"));
    }
    let mut samples = Vec::with_capacity(payload.len() / 4);
    for bytes in payload.chunks_exact(4) {
        samples.push(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
    }
    Ok(samples)
}

fn decode_i16_le_payload(payload: &Bytes) -> Result<Vec<f32>, SpeechError> {
    if payload.len() % 2 != 0 {
        return Err(SpeechError::provider("I16 LE payload length is not aligned"));
    }
    let mut samples = Vec::with_capacity(payload.len() / 2);
    for bytes in payload.chunks_exact(2) {
        let sample = i16::from_le_bytes([bytes[0], bytes[1]]);
        samples.push(f32::from(sample) / f32::from(i16::MAX));
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::AudioSampleFormat;

    fn f32_chunk(sequence: u64, samples: &[f32]) -> AudioChunk {
        let mut payload = Vec::with_capacity(samples.len() * 4);
        for sample in samples {
            payload.extend_from_slice(&sample.to_le_bytes());
        }
        AudioChunk {
            session_id: "s".into(),
            sequence,
            captured_at_ns: 0,
            sample_rate_hz: 16_000,
            channels: 1,
            sample_format: AudioSampleFormat::F32,
            payload: Bytes::from(payload),
        }
    }

    #[test]
    fn buffer_retains_frames_across_chunks() {
        let mut buffer = UtteranceAudioBuffer::new();
        buffer
            .push_chunk(&f32_chunk(0, &[0.0, 0.5]))
            .expect("chunk 0");
        buffer
            .push_chunk(&f32_chunk(1, &[-0.5, 1.0]))
            .expect("chunk 1");
        assert_eq!(buffer.frames_accepted(), 2);
        assert_eq!(buffer.len_samples(), 4);
        assert_eq!(buffer.samples(), &[0.0, 0.5, -0.5, 1.0]);

        let frozen = buffer.freeze();
        assert_eq!(frozen.frames_accepted(), 2);
        assert_eq!(frozen.sample_rate_hz(), 16_000);
        assert!(!frozen.is_empty());
    }

    #[test]
    fn empty_payload_does_not_count_as_frame() {
        let mut buffer = UtteranceAudioBuffer::new();
        buffer.push_chunk(&f32_chunk(0, &[])).expect("empty");
        assert_eq!(buffer.frames_accepted(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn rejects_sample_rate_change() {
        let mut buffer = UtteranceAudioBuffer::new();
        buffer.push_f32(&[0.1], 16_000, 1).unwrap();
        let err = buffer.push_f32(&[0.2], 8_000, 1).unwrap_err();
        assert!(err.to_string().contains("sample rate changed"));
    }
}

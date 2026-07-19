//! Packed PCM frames with monotonic sequence and capture timestamps.

use bytes::Bytes;

use crate::format::{AudioFormat, CANONICAL_AUDIO_FORMAT};

/// Optional clipping / level diagnostics computed at capture time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioDiagnostics {
    /// Peak absolute sample in the frame (`0.0`..=`1.0+` if clipped).
    pub peak_abs: f32,
    /// Root-mean-square level over the frame.
    pub rms: f32,
    /// True when any sample exceeded the clipping threshold.
    pub clipped: bool,
}

impl AudioDiagnostics {
    pub const CLIP_THRESHOLD: f32 = 0.999;

    /// Compute diagnostics over Float32 mono samples.
    #[must_use]
    pub fn from_f32_samples(samples: &[f32]) -> Self {
        if samples.is_empty() {
            return Self {
                peak_abs: 0.0,
                rms: 0.0,
                clipped: false,
            };
        }

        let mut peak = 0.0_f32;
        let mut sum_sq = 0.0_f64;
        let mut clipped = false;
        for &sample in samples {
            let abs = sample.abs();
            if abs > peak {
                peak = abs;
            }
            if abs >= Self::CLIP_THRESHOLD {
                clipped = true;
            }
            sum_sq += f64::from(sample) * f64::from(sample);
        }
        let rms = (sum_sq / samples.len() as f64).sqrt() as f32;
        Self {
            peak_abs: peak,
            rms,
            clipped,
        }
    }
}

/// One sequenced chunk of packed PCM from a capture session.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioFrame {
    pub sequence: u64,
    /// Monotonic capture clock nanoseconds (platform-defined epoch).
    pub captured_at_ns: u64,
    /// Number of interleaved frames (sample-frames) in `payload`.
    pub frame_count: u32,
    pub format: AudioFormat,
    /// Packed little-endian samples (`f32` for the canonical format).
    pub payload: Bytes,
    pub diagnostics: Option<AudioDiagnostics>,
}

impl AudioFrame {
    /// Build a frame from Float32 samples in the canonical format.
    #[must_use]
    pub fn from_f32_le(
        sequence: u64,
        captured_at_ns: u64,
        samples: &[f32],
        with_diagnostics: bool,
    ) -> Self {
        let mut bytes = Vec::with_capacity(samples.len() * 4);
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        let diagnostics = with_diagnostics.then(|| AudioDiagnostics::from_f32_samples(samples));
        Self {
            sequence,
            captured_at_ns,
            frame_count: samples.len() as u32,
            format: CANONICAL_AUDIO_FORMAT,
            payload: Bytes::from(bytes),
            diagnostics,
        }
    }

    /// Decode payload as Float32 LE samples when `format` is canonical F32.
    #[must_use]
    pub fn f32_samples(&self) -> Option<Vec<f32>> {
        if self.format.sample_format != crate::format::SampleFormat::F32Le {
            return None;
        }
        let raw = self.payload.as_ref();
        if raw.len() % 4 != 0 {
            return None;
        }
        let mut out = Vec::with_capacity(raw.len() / 4);
        for chunk in raw.chunks_exact(4) {
            out.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_f32_payload() {
        let samples = [0.0_f32, 0.5, -0.25, 1.0];
        let frame = AudioFrame::from_f32_le(7, 1_000, &samples, true);
        assert_eq!(frame.sequence, 7);
        assert_eq!(frame.frame_count, 4);
        assert_eq!(frame.payload.len(), 16);
        assert_eq!(frame.f32_samples().as_deref(), Some(samples.as_slice()));
        let diag = frame.diagnostics.expect("diagnostics");
        assert!(diag.peak_abs >= 0.999);
        assert!(diag.clipped);
    }

    #[test]
    fn empty_diagnostics_are_zero() {
        let d = AudioDiagnostics::from_f32_samples(&[]);
        assert_eq!(d.peak_abs, 0.0);
        assert_eq!(d.rms, 0.0);
        assert!(!d.clipped);
    }
}

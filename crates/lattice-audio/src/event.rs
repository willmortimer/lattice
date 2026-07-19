//! Capture stream events and gap notifications.

use crate::frame::AudioFrame;

/// Sequence discontinuity reported when frames are dropped under backpressure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GapEvent {
    /// Last contiguous sequence observed before the gap (exclusive of missing).
    pub from_sequence: u64,
    /// Next sequence after the gap (first received after drop).
    pub to_sequence: u64,
    pub captured_at_ns: u64,
}

impl GapEvent {
    /// Number of missing sequence numbers in `(from_sequence, to_sequence)`.
    #[must_use]
    pub fn missing_count(&self) -> u64 {
        self.to_sequence.saturating_sub(self.from_sequence).saturating_sub(1)
    }
}

/// Events emitted by a [`crate::CaptureProvider`].
#[derive(Debug, Clone, PartialEq)]
pub enum CaptureEvent {
    /// Capture pipeline started streaming (pre-roll flush may follow).
    Started { captured_at_ns: u64 },
    /// Packed PCM frame.
    Frame(AudioFrame),
    /// Dropped frames / sequence discontinuity.
    Gap(GapEvent),
    /// Capture stopped cleanly.
    Stopped { captured_at_ns: u64 },
    /// Fatal capture error; stream ends.
    Error {
        message: String,
        captured_at_ns: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gap_missing_count() {
        let gap = GapEvent {
            from_sequence: 3,
            to_sequence: 7,
            captured_at_ns: 0,
        };
        assert_eq!(gap.missing_count(), 3);
    }
}

//! Bounded PCM frame queues for capture → consumer backpressure.

use std::collections::VecDeque;

use crate::event::GapEvent;
use crate::frame::AudioFrame;

/// Default capacity: ~5 s of 20 ms frames (see `docs/voice/audio-capture.md`).
pub const DEFAULT_FRAME_QUEUE_CAPACITY: usize = 250;

/// Fixed-capacity queue of [`AudioFrame`] values.
///
/// When full, new frames are rejected and a [`GapEvent`] is returned so callers
/// can surface the discontinuity instead of growing without bound.
#[derive(Debug, Clone, Default)]
pub struct BoundedFrameQueue {
    capacity: usize,
    frames: VecDeque<AudioFrame>,
}

impl BoundedFrameQueue {
    /// Create an empty queue that holds at most `capacity` frames.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            frames: VecDeque::with_capacity(capacity.min(64)),
        }
    }

    /// Canonical default capacity ([`DEFAULT_FRAME_QUEUE_CAPACITY`]).
    #[must_use]
    pub fn canonical_default() -> Self {
        Self::with_capacity(DEFAULT_FRAME_QUEUE_CAPACITY)
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        self.capacity == 0 || self.frames.len() >= self.capacity
    }

    /// Enqueue `frame`, or return a gap event when the queue is full.
    ///
    /// Dropped frames are the *incoming* ones (newest), preserving already-queued
    /// audio continuity until the consumer drains.
    pub fn try_push(&mut self, frame: AudioFrame) -> Result<(), GapEvent> {
        if self.capacity == 0 || self.frames.len() >= self.capacity {
            let sequence = frame.sequence;
            return Err(GapEvent {
                from_sequence: sequence.saturating_sub(1),
                to_sequence: sequence.saturating_add(1),
                captured_at_ns: frame.captured_at_ns,
            });
        }
        self.frames.push_back(frame);
        Ok(())
    }

    /// Pop the oldest queued frame.
    pub fn pop_front(&mut self) -> Option<AudioFrame> {
        self.frames.pop_front()
    }

    pub fn clear(&mut self) {
        self.frames.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(sequence: u64) -> AudioFrame {
        AudioFrame::from_f32_le(sequence, sequence * 1_000, &[0.1, 0.2], false)
    }

    #[test]
    fn accepts_until_capacity() {
        let mut queue = BoundedFrameQueue::with_capacity(2);
        assert!(queue.try_push(frame(0)).is_ok());
        assert!(queue.try_push(frame(1)).is_ok());
        assert!(queue.is_full());

        let gap = queue.try_push(frame(2)).unwrap_err();
        assert_eq!(gap.from_sequence, 1);
        assert_eq!(gap.to_sequence, 3);
        assert_eq!(gap.missing_count(), 1);
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn drain_preserves_order_and_timestamps() {
        let mut queue = BoundedFrameQueue::with_capacity(4);
        queue.try_push(frame(0)).unwrap();
        queue.try_push(frame(1)).unwrap();

        let first = queue.pop_front().unwrap();
        assert_eq!(first.sequence, 0);
        assert_eq!(first.captured_at_ns, 0);
        assert_eq!(first.payload.len(), 8);

        let second = queue.pop_front().unwrap();
        assert_eq!(second.sequence, 1);
        assert_eq!(second.captured_at_ns, 1_000);
        assert!(queue.is_empty());
    }

    #[test]
    fn zero_capacity_always_gaps() {
        let mut queue = BoundedFrameQueue::with_capacity(0);
        let gap = queue.try_push(frame(5)).unwrap_err();
        assert_eq!(gap.from_sequence, 4);
        assert_eq!(gap.to_sequence, 6);
        assert!(queue.is_empty());
    }

    #[test]
    fn canonical_default_is_five_seconds() {
        assert_eq!(
            BoundedFrameQueue::canonical_default().capacity(),
            DEFAULT_FRAME_QUEUE_CAPACITY
        );
    }
}

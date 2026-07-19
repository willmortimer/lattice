//! Fixed-capacity sample rings and pre-roll buffers.

use crate::format::{AudioFormat, CANONICAL_AUDIO_FORMAT, DEFAULT_PRE_ROLL_MS};

/// Contiguous Float32 sample ring (overwrite-oldest when full).
#[derive(Debug, Clone)]
pub struct RingBuffer {
    buf: Vec<f32>,
    capacity: usize,
    /// Next write index.
    head: usize,
    len: usize,
}

impl RingBuffer {
    /// Create an empty ring that holds at most `capacity` samples.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: vec![0.0; capacity],
            capacity,
            head: 0,
            len: 0,
        }
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        self.len == self.capacity && self.capacity > 0
    }

    /// Push samples, dropping the oldest when over capacity.
    pub fn push_slice(&mut self, samples: &[f32]) {
        if self.capacity == 0 || samples.is_empty() {
            return;
        }

        for &sample in samples {
            if self.len < self.capacity {
                let idx = (self.head + self.len) % self.capacity;
                self.buf[idx] = sample;
                self.len += 1;
            } else {
                self.buf[self.head] = sample;
                self.head = (self.head + 1) % self.capacity;
            }
        }
    }

    /// Copy samples in chronological order into `out` (cleared first).
    pub fn copy_chronologically(&self, out: &mut Vec<f32>) {
        out.clear();
        out.reserve(self.len);
        for i in 0..self.len {
            let idx = (self.head + i) % self.capacity;
            out.push(self.buf[idx]);
        }
    }

    /// Drain all samples in chronological order and clear the ring.
    pub fn drain(&mut self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.len);
        self.copy_chronologically(&mut out);
        self.clear();
        out
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }
}

/// Rolling pre-roll window sized for a capture format and duration.
#[derive(Debug, Clone)]
pub struct PreRollBuffer {
    format: AudioFormat,
    duration_ms: u32,
    ring: RingBuffer,
    /// Monotonic timestamp of the newest sample written (ns), if any.
    newest_captured_at_ns: Option<u64>,
}

impl PreRollBuffer {
    /// Pre-roll for `duration_ms` of `format` audio (default 300 ms canonical).
    #[must_use]
    pub fn new(format: AudioFormat, duration_ms: u32) -> Self {
        let capacity = format.sample_count_for_ms(duration_ms);
        Self {
            format,
            duration_ms,
            ring: RingBuffer::with_capacity(capacity),
            newest_captured_at_ns: None,
        }
    }

    /// Canonical 16 kHz mono Float32 pre-roll at [`DEFAULT_PRE_ROLL_MS`].
    #[must_use]
    pub fn canonical_default() -> Self {
        Self::new(CANONICAL_AUDIO_FORMAT, DEFAULT_PRE_ROLL_MS)
    }

    #[must_use]
    pub fn format(&self) -> AudioFormat {
        self.format
    }

    #[must_use]
    pub fn duration_ms(&self) -> u32 {
        self.duration_ms
    }

    #[must_use]
    pub fn capacity_samples(&self) -> usize {
        self.ring.capacity()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    #[must_use]
    pub fn newest_captured_at_ns(&self) -> Option<u64> {
        self.newest_captured_at_ns
    }

    /// Append Float32 samples and record the chunk's capture timestamp.
    pub fn push_f32(&mut self, samples: &[f32], captured_at_ns: u64) {
        self.ring.push_slice(samples);
        if !samples.is_empty() {
            self.newest_captured_at_ns = Some(captured_at_ns);
        }
    }

    /// Take the full pre-roll window in chronological order and clear.
    pub fn take(&mut self) -> (Vec<f32>, Option<u64>) {
        let ts = self.newest_captured_at_ns.take();
        (self.ring.drain(), ts)
    }

    pub fn clear(&mut self) {
        self.ring.clear();
        self.newest_captured_at_ns = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_overwrites_oldest() {
        let mut ring = RingBuffer::with_capacity(4);
        ring.push_slice(&[1.0, 2.0, 3.0, 4.0]);
        assert!(ring.is_full());
        ring.push_slice(&[5.0, 6.0]);
        let drained = ring.drain();
        assert_eq!(drained, vec![3.0, 4.0, 5.0, 6.0]);
        assert!(ring.is_empty());
    }

    #[test]
    fn ring_partial_fill_preserves_order() {
        let mut ring = RingBuffer::with_capacity(8);
        ring.push_slice(&[0.1, 0.2, 0.3]);
        let mut out = Vec::new();
        ring.copy_chronologically(&mut out);
        assert_eq!(out, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn pre_roll_default_capacity_is_300ms() {
        let pre = PreRollBuffer::canonical_default();
        assert_eq!(pre.duration_ms(), 300);
        assert_eq!(pre.capacity_samples(), 4_800);
    }

    #[test]
    fn pre_roll_keeps_trailing_window() {
        let mut pre = PreRollBuffer::new(CANONICAL_AUDIO_FORMAT, 20); // 320 samples
        assert_eq!(pre.capacity_samples(), 320);

        let first: Vec<f32> = (0..320).map(|i| i as f32).collect();
        pre.push_f32(&first, 1_000);
        let second: Vec<f32> = (320..480).map(|i| i as f32).collect();
        pre.push_f32(&second, 2_000);

        let (samples, ts) = pre.take();
        assert_eq!(ts, Some(2_000));
        assert_eq!(samples.len(), 320);
        assert_eq!(samples[0], 160.0);
        assert_eq!(samples[319], 479.0);
        assert!(pre.is_empty());
    }

    #[test]
    fn zero_capacity_ring_is_noop() {
        let mut ring = RingBuffer::with_capacity(0);
        ring.push_slice(&[1.0, 2.0]);
        assert!(ring.is_empty());
        assert!(ring.drain().is_empty());
    }
}

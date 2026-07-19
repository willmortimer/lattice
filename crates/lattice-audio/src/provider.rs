//! Capture provider trait and event fan-out.

use tokio::sync::mpsc;

use crate::error::CaptureError;
use crate::event::CaptureEvent;

/// Delivers capture events to a local subscriber.
#[derive(Clone, Debug)]
pub struct CaptureEventSender {
    tx: mpsc::UnboundedSender<CaptureEvent>,
}

impl CaptureEventSender {
    pub fn new(tx: mpsc::UnboundedSender<CaptureEvent>) -> Self {
        Self { tx }
    }

    pub fn pair() -> (Self, mpsc::UnboundedReceiver<CaptureEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self::new(tx), rx)
    }

    pub fn send(&self, event: CaptureEvent) -> Result<(), CaptureError> {
        self.tx
            .send(event)
            .map_err(|_| CaptureError::EventSubscriberDisconnected)
    }
}

/// Client-owned microphone capture (not `latticed`).
///
/// Typical lifecycle: [`CaptureProvider::subscribe`], optional
/// [`CaptureProvider::arm`] (fill pre-roll), [`CaptureProvider::start`]
/// (flush pre-roll then live frames), [`CaptureProvider::stop`].
pub trait CaptureProvider: Send {
    /// Begin filling the pre-roll ring without emitting stream frames.
    fn arm(&mut self) -> Result<(), CaptureError>;

    /// Start streaming. Emits [`CaptureEvent::Started`], any armed pre-roll as
    /// frames, then live [`CaptureEvent::Frame`] values.
    fn start(&mut self) -> Result<(), CaptureError>;

    /// Stop capture and emit [`CaptureEvent::Stopped`] when a subscriber exists.
    fn stop(&mut self) -> Result<(), CaptureError>;

    /// Subscribe to capture events. Call before [`CaptureProvider::start`].
    fn subscribe(&mut self) -> mpsc::UnboundedReceiver<CaptureEvent>;
}

/// In-memory provider that feeds synthetic Float32 frames (unit tests / demos).
#[derive(Debug)]
pub struct SyntheticCaptureProvider {
    events: Option<CaptureEventSender>,
    armed: bool,
    running: bool,
    pre_roll: crate::ring::PreRollBuffer,
    next_sequence: u64,
    clock_ns: u64,
}

impl Default for SyntheticCaptureProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntheticCaptureProvider {
    pub fn new() -> Self {
        Self {
            events: None,
            armed: false,
            running: false,
            pre_roll: crate::ring::PreRollBuffer::canonical_default(),
            next_sequence: 0,
            clock_ns: 0,
        }
    }

    fn tick(&mut self, delta_ns: u64) -> u64 {
        self.clock_ns = self.clock_ns.saturating_add(delta_ns);
        self.clock_ns
    }

    /// Push synthetic samples into the armed pre-roll (no-op unless armed).
    pub fn push_pre_roll_samples(&mut self, samples: &[f32]) {
        if self.armed && !self.running {
            let ts = self.tick(samples.len() as u64 * 62_500); // ~1/16k s per sample
            self.pre_roll.push_f32(samples, ts);
        }
    }

    /// Emit one live frame while running (test helper).
    pub fn emit_live_frame(&mut self, samples: &[f32]) -> Result<(), CaptureError> {
        if !self.running {
            return Err(CaptureError::NotRunning);
        }
        let events = self
            .events
            .clone()
            .ok_or(CaptureError::EventSubscriberDisconnected)?;
        let ts = self.tick(samples.len() as u64 * 62_500);
        let seq = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        let frame = crate::frame::AudioFrame::from_f32_le(seq, ts, samples, true);
        events.send(CaptureEvent::Frame(frame))
    }
}

impl CaptureProvider for SyntheticCaptureProvider {
    fn arm(&mut self) -> Result<(), CaptureError> {
        if self.running {
            return Err(CaptureError::AlreadyRunning);
        }
        self.armed = true;
        self.pre_roll.clear();
        Ok(())
    }

    fn start(&mut self) -> Result<(), CaptureError> {
        if self.running {
            return Err(CaptureError::AlreadyRunning);
        }
        let Some(events) = self.events.clone() else {
            return Err(CaptureError::invalid_argument(
                "subscribe before start",
            ));
        };

        let started_at = self.tick(1);
        events.send(CaptureEvent::Started {
            captured_at_ns: started_at,
        })?;

        let (pre_samples, pre_ts) = self.pre_roll.take();
        if !pre_samples.is_empty() {
            let ts = pre_ts.unwrap_or(started_at);
            let seq = self.next_sequence;
            self.next_sequence = self.next_sequence.saturating_add(1);
            let frame = crate::frame::AudioFrame::from_f32_le(seq, ts, &pre_samples, true);
            events.send(CaptureEvent::Frame(frame))?;
        }

        self.armed = false;
        self.running = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        if !self.running && !self.armed {
            return Err(CaptureError::NotRunning);
        }
        let stopped_at = self.tick(1);
        if let Some(events) = &self.events {
            let _ = events.send(CaptureEvent::Stopped {
                captured_at_ns: stopped_at,
            });
        }
        self.running = false;
        self.armed = false;
        self.pre_roll.clear();
        Ok(())
    }

    fn subscribe(&mut self) -> mpsc::UnboundedReceiver<CaptureEvent> {
        let (tx, rx) = CaptureEventSender::pair();
        self.events = Some(tx);
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::CaptureEvent;

    #[test]
    fn synthetic_pre_roll_flushes_on_start() {
        let mut provider = SyntheticCaptureProvider::new();
        let mut rx = provider.subscribe();
        provider.arm().unwrap();
        provider.push_pre_roll_samples(&[0.1, 0.2, 0.3, 0.4]);
        provider.start().unwrap();

        match rx.try_recv().unwrap() {
            CaptureEvent::Started { .. } => {}
            other => panic!("expected Started, got {other:?}"),
        }
        match rx.try_recv().unwrap() {
            CaptureEvent::Frame(frame) => {
                assert_eq!(frame.sequence, 0);
                assert_eq!(frame.frame_count, 4);
                assert_eq!(
                    frame.f32_samples().as_deref(),
                    Some([0.1_f32, 0.2, 0.3, 0.4].as_slice())
                );
            }
            other => panic!("expected Frame, got {other:?}"),
        }

        provider
            .emit_live_frame(&[0.5, 0.6])
            .unwrap();
        match rx.try_recv().unwrap() {
            CaptureEvent::Frame(frame) => {
                assert_eq!(frame.sequence, 1);
                assert_eq!(frame.frame_count, 2);
            }
            other => panic!("expected Frame, got {other:?}"),
        }

        provider.stop().unwrap();
        assert!(matches!(
            rx.try_recv().unwrap(),
            CaptureEvent::Stopped { .. }
        ));
    }

    #[test]
    fn start_without_subscribe_fails() {
        let mut provider = SyntheticCaptureProvider::new();
        let err = provider.start().unwrap_err();
        assert!(matches!(err, CaptureError::InvalidArgument(_)));
    }
}

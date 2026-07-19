//! [`CaptureProvider`] backed by LatticeAudioBridge (when linked).

use std::ffi::c_void;

use lattice_audio::{
    AudioDiagnostics, AudioFrame, CaptureError, CaptureEvent, CaptureEventSender, CaptureProvider,
    GapEvent, DEFAULT_PRE_ROLL_MS,
};
use tokio::sync::mpsc;

use crate::bridge::NativeCapture;
use crate::ffi::{
    copy_error_message, copy_frame_samples, LatticeAudioEvent, LatticeAudioEventKind,
};

struct CallbackState {
    events: CaptureEventSender,
}

/// macOS `AVAudioEngine` capture provider.
///
/// Without the `link-bridge` feature this type still compiles but all capture
/// calls return [`CaptureError::Unsupported`].
pub struct MacOsCaptureProvider {
    pre_roll_ms: u32,
    enable_diagnostics: bool,
    capture: Option<NativeCapture>,
    events: Option<CaptureEventSender>,
    /// Keeps the callback context alive while the native bridge holds its pointer.
    callback_state: Option<Box<CallbackState>>,
    running: bool,
    armed: bool,
}

impl MacOsCaptureProvider {
    pub fn new() -> Self {
        Self::with_options(DEFAULT_PRE_ROLL_MS, true)
    }

    pub fn with_options(pre_roll_ms: u32, enable_diagnostics: bool) -> Self {
        Self {
            pre_roll_ms,
            enable_diagnostics,
            capture: None,
            events: None,
            callback_state: None,
            running: false,
            armed: false,
        }
    }

    fn ensure_capture(&mut self) -> Result<(), CaptureError> {
        if self.capture.is_none() {
            self.capture = Some(NativeCapture::create(
                self.pre_roll_ms,
                self.enable_diagnostics,
            )?);
        }
        Ok(())
    }
}

impl Default for MacOsCaptureProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureProvider for MacOsCaptureProvider {
    fn arm(&mut self) -> Result<(), CaptureError> {
        if self.running {
            return Err(CaptureError::AlreadyRunning);
        }
        self.ensure_capture()?;
        let capture = self.capture.as_ref().expect("capture created");
        capture.arm()?;
        self.armed = true;
        Ok(())
    }

    fn start(&mut self) -> Result<(), CaptureError> {
        if self.running {
            return Err(CaptureError::AlreadyRunning);
        }
        let Some(events) = self.events.clone() else {
            return Err(CaptureError::invalid_argument("subscribe before start"));
        };
        self.ensure_capture()?;
        let capture = self.capture.as_ref().expect("capture created");

        let state = Box::new(CallbackState { events });
        let context = (&*state as *const CallbackState).cast::<c_void>().cast_mut();
        self.callback_state = Some(state);

        match capture.start(Some(on_bridge_event), context) {
            Ok(()) => {
                self.running = true;
                self.armed = false;
                Ok(())
            }
            Err(err) => {
                self.callback_state = None;
                Err(err)
            }
        }
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        if let Some(capture) = &self.capture {
            capture.stop()?;
        } else if !self.armed && !self.running {
            return Err(CaptureError::NotRunning);
        }
        self.running = false;
        self.armed = false;
        // Drop callback state only after the bridge has stopped invoking it.
        self.callback_state = None;
        Ok(())
    }

    fn subscribe(&mut self) -> mpsc::UnboundedReceiver<CaptureEvent> {
        let (tx, rx) = CaptureEventSender::pair();
        self.events = Some(tx);
        rx
    }
}

unsafe extern "C" fn on_bridge_event(event: *const LatticeAudioEvent, context: *mut c_void) {
    if event.is_null() || context.is_null() {
        return;
    }
    let state = &*(context as *const CallbackState);
    let event = &*event;
    let mapped = map_event(event);
    let _ = state.events.send(mapped);
}

fn map_event(event: &LatticeAudioEvent) -> CaptureEvent {
    match event.kind {
        k if k == LatticeAudioEventKind::Started as u32 => CaptureEvent::Started {
            captured_at_ns: event.captured_at_ns,
        },
        k if k == LatticeAudioEventKind::Stopped as u32 => CaptureEvent::Stopped {
            captured_at_ns: event.captured_at_ns,
        },
        k if k == LatticeAudioEventKind::Frame as u32 => {
            let samples = copy_frame_samples(&event.frame);
            let diagnostics = if event.frame.peak_abs.is_nan() {
                None
            } else {
                Some(AudioDiagnostics {
                    peak_abs: event.frame.peak_abs,
                    rms: event.frame.rms,
                    clipped: event.frame.clipped != 0,
                })
            };
            let mut frame = AudioFrame::from_f32_le(
                event.frame.sequence,
                event.frame.captured_at_ns,
                &samples,
                false,
            );
            frame.diagnostics = diagnostics;
            CaptureEvent::Frame(frame)
        }
        k if k == LatticeAudioEventKind::Gap as u32 => CaptureEvent::Gap(GapEvent {
            from_sequence: event.gap.from_sequence,
            to_sequence: event.gap.to_sequence,
            captured_at_ns: event.gap.captured_at_ns,
        }),
        k if k == LatticeAudioEventKind::Error as u32 => CaptureEvent::Error {
            message: copy_error_message(event),
            captured_at_ns: event.captured_at_ns,
        },
        _ => CaptureEvent::Error {
            message: format!("unknown capture event kind {}", event.kind),
            captured_at_ns: event.captured_at_ns,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribe_without_bridge_still_works() {
        let mut provider = MacOsCaptureProvider::new();
        let mut rx = provider.subscribe();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn start_without_subscribe_fails() {
        let mut provider = MacOsCaptureProvider::new();
        let err = provider.start().unwrap_err();
        assert!(matches!(err, CaptureError::InvalidArgument(_)));
    }

    #[test]
    #[cfg(not(link_bridge))]
    fn arm_without_bridge_is_unsupported() {
        let mut provider = MacOsCaptureProvider::new();
        let _ = provider.subscribe();
        let err = provider.arm().unwrap_err();
        assert!(matches!(err, CaptureError::Unsupported(_)));
    }
}

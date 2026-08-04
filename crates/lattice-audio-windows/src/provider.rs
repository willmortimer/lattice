//! [`CaptureProvider`] for Windows WASAPI (cpal), with unsupported stubs elsewhere.

use lattice_audio::{CaptureError, CaptureEvent, CaptureEventSender, CaptureProvider};
use tokio::sync::mpsc;

#[cfg(windows)]
use crate::stream::{self, ActiveCapture};

#[cfg(not(windows))]
const UNSUPPORTED_HOST_MSG: &str =
    "Windows WASAPI mic capture requires a Windows host (lattice-audio-windows)";

/// Windows microphone capture provider backed by cpal/WASAPI on `cfg(windows)`.
pub struct WindowsCaptureProvider {
    events: Option<CaptureEventSender>,
    armed: bool,
    running: bool,
    #[cfg(windows)]
    active: Option<ActiveCapture>,
}

impl WindowsCaptureProvider {
    pub fn new() -> Self {
        Self {
            events: None,
            armed: false,
            running: false,
            #[cfg(windows)]
            active: None,
        }
    }
}

impl Default for WindowsCaptureProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureProvider for WindowsCaptureProvider {
    fn arm(&mut self) -> Result<(), CaptureError> {
        if self.running {
            return Err(CaptureError::AlreadyRunning);
        }
        #[cfg(windows)]
        {
            self.ensure_active()?;
            let active = self.active.as_mut().expect("active after ensure");
            active.set_armed()?;
            self.armed = true;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err(CaptureError::Unsupported(UNSUPPORTED_HOST_MSG.into()))
        }
    }

    fn start(&mut self) -> Result<(), CaptureError> {
        if self.running {
            return Err(CaptureError::AlreadyRunning);
        }
        let Some(events) = self.events.clone() else {
            return Err(CaptureError::invalid_argument("subscribe before start"));
        };
        #[cfg(windows)]
        {
            self.ensure_active()?;
            let active = self.active.as_mut().expect("active after ensure");
            active.start_streaming(events)?;
            self.armed = false;
            self.running = true;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = events;
            Err(CaptureError::Unsupported(UNSUPPORTED_HOST_MSG.into()))
        }
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        #[cfg(windows)]
        {
            if !self.running && !self.armed && self.active.is_none() {
                return Err(CaptureError::NotRunning);
            }
            if let Some(mut active) = self.active.take() {
                active.stop(self.events.clone())?;
            }
            self.running = false;
            self.armed = false;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            if !self.running && !self.armed {
                return Err(CaptureError::NotRunning);
            }
            self.running = false;
            self.armed = false;
            Err(CaptureError::Unsupported(UNSUPPORTED_HOST_MSG.into()))
        }
    }

    fn subscribe(&mut self) -> mpsc::UnboundedReceiver<CaptureEvent> {
        let (tx, rx) = CaptureEventSender::pair();
        self.events = Some(tx);
        #[cfg(windows)]
        if let Some(active) = self.active.as_mut() {
            active.set_events(self.events.clone());
        }
        rx
    }
}

#[cfg(windows)]
impl WindowsCaptureProvider {
    fn ensure_active(&mut self) -> Result<(), CaptureError> {
        if self.active.is_none() {
            self.active = Some(stream::ActiveCapture::start(self.events.clone())?);
        }
        Ok(())
    }
}

/// True when the host default input device can be opened (Windows only).
///
/// Off Windows this always returns `false`. Used by desktop `voice_status` to
/// report mic-ready without starting a capture session.
#[must_use]
pub fn default_input_available() -> bool {
    #[cfg(windows)]
    {
        stream::default_input_available()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribe_works_everywhere() {
        let mut provider = WindowsCaptureProvider::new();
        let mut rx = provider.subscribe();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn start_without_subscribe_fails() {
        let mut provider = WindowsCaptureProvider::new();
        let err = provider.start().unwrap_err();
        assert!(matches!(err, CaptureError::InvalidArgument(_)));
    }

    #[test]
    #[cfg(not(windows))]
    fn arm_is_unsupported_off_windows() {
        let mut provider = WindowsCaptureProvider::new();
        let _ = provider.subscribe();
        let err = provider.arm().unwrap_err();
        assert!(matches!(err, CaptureError::Unsupported(_)));
        assert!(err.to_string().contains("WASAPI"));
        assert!(!default_input_available());
    }

    #[test]
    #[cfg(not(windows))]
    fn start_is_unsupported_off_windows() {
        let mut provider = WindowsCaptureProvider::new();
        let _ = provider.subscribe();
        let err = provider.start().unwrap_err();
        assert!(matches!(err, CaptureError::Unsupported(_)));
    }

    #[test]
    #[cfg(windows)]
    fn provider_constructs_on_windows() {
        // Compile/shape guard: public API stays available; device may be absent
        // in headless CI so we only assert construction + subscribe.
        let mut provider = WindowsCaptureProvider::new();
        let _ = provider.subscribe();
        let _ = default_input_available();
    }
}

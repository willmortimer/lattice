//! Live microphone capture smoke test (ignored by default).
//!
//! Requires `--features live-capture` (links LatticeAudioBridge) and mic access.
//!
//! ```sh
//! cargo test -p lattice-audio-macos --features live-capture --test live_capture -- --ignored --nocapture
//! ```

#![cfg(all(target_os = "macos", feature = "live-capture"))]

use std::time::Duration;

use lattice_audio::{CaptureEvent, CaptureProvider};
use lattice_audio_macos::MacOsCaptureProvider;

#[test]
#[ignore = "requires microphone permission and LatticeAudioBridge dylib"]
fn live_capture_emits_frames() {
    let mut provider = MacOsCaptureProvider::with_options(300, true);
    let mut rx = provider.subscribe();

    provider.arm().expect("arm");
    std::thread::sleep(Duration::from_millis(350));
    provider.start().expect("start");
    std::thread::sleep(Duration::from_millis(500));
    provider.stop().expect("stop");

    let mut saw_started = false;
    let mut frame_count = 0u32;
    let mut saw_stopped = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            CaptureEvent::Started { .. } => saw_started = true,
            CaptureEvent::Frame(frame) => {
                frame_count += 1;
                assert_eq!(frame.format.sample_rate_hz, 16_000);
                assert_eq!(frame.format.channels, 1);
                assert!(frame.frame_count > 0);
            }
            CaptureEvent::Stopped { .. } => saw_stopped = true,
            CaptureEvent::Gap(_) | CaptureEvent::Error { .. } => {}
        }
    }

    assert!(saw_started, "expected Started event");
    assert!(
        frame_count > 0,
        "expected at least one PCM frame (speak or allow ambient noise)"
    );
    assert!(saw_stopped, "expected Stopped event");
}

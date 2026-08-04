//! cpal/WASAPI input stream owned on a background thread.

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, StreamConfig};
use lattice_audio::{
    AudioFrame, CaptureError, CaptureEvent, CaptureEventSender, PreRollBuffer,
    CANONICAL_AUDIO_FORMAT, DEFAULT_PRE_ROLL_MS,
};

/// Idle / filling pre-roll / emitting live frames.
const MODE_IDLE: u8 = 0;
const MODE_ARMED: u8 = 1;
const MODE_RUNNING: u8 = 2;

/// ~20 ms at 16 kHz mono.
const FRAME_SAMPLES: usize = 320;

struct SharedState {
    mode: AtomicU8,
    events: Mutex<Option<CaptureEventSender>>,
    pre_roll: Mutex<PreRollBuffer>,
    next_sequence: AtomicU64,
    converter: Mutex<FormatConverter>,
    started_at: Instant,
}

impl SharedState {
    fn new(events: Option<CaptureEventSender>, in_rate: u32, in_channels: u16) -> Self {
        Self {
            mode: AtomicU8::new(MODE_IDLE),
            events: Mutex::new(events),
            pre_roll: Mutex::new(PreRollBuffer::new(
                CANONICAL_AUDIO_FORMAT,
                DEFAULT_PRE_ROLL_MS,
            )),
            next_sequence: AtomicU64::new(0),
            converter: Mutex::new(FormatConverter::new(in_rate, in_channels)),
            started_at: Instant::now(),
        }
    }

    fn now_ns(&self) -> u64 {
        self.started_at.elapsed().as_nanos() as u64
    }
}

/// Owns the capture thread + stop signal.
pub struct ActiveCapture {
    shared: Arc<SharedState>,
    stop_tx: Option<mpsc::Sender<()>>,
    join: Option<JoinHandle<()>>,
}

impl ActiveCapture {
    pub fn start(events: Option<CaptureEventSender>) -> Result<Self, CaptureError> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<Arc<SharedState>, CaptureError>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let join = thread::Builder::new()
            .name("lattice-audio-wasapi".into())
            .spawn(move || {
                if let Err(err) = run_capture_thread(events, &ready_tx, stop_rx) {
                    let _ = ready_tx.send(Err(err));
                }
            })
            .map_err(|err| CaptureError::provider(format!("spawn capture thread: {err}")))?;

        let shared = ready_rx
            .recv()
            .map_err(|_| CaptureError::provider("capture thread exited before ready"))??;

        Ok(Self {
            shared,
            stop_tx: Some(stop_tx),
            join: Some(join),
        })
    }

    pub fn set_events(&self, events: Option<CaptureEventSender>) {
        if let Ok(mut slot) = self.shared.events.lock() {
            *slot = events;
        }
    }

    pub fn set_armed(&self) -> Result<(), CaptureError> {
        if let Ok(mut pre) = self.shared.pre_roll.lock() {
            pre.clear();
        }
        self.shared.mode.store(MODE_ARMED, Ordering::SeqCst);
        Ok(())
    }

    pub fn start_streaming(&self, events: CaptureEventSender) -> Result<(), CaptureError> {
        {
            let mut slot = self
                .shared
                .events
                .lock()
                .map_err(|_| CaptureError::provider("events lock poisoned"))?;
            *slot = Some(events.clone());
        }

        let ts = self.shared.now_ns();
        events.send(CaptureEvent::Started {
            captured_at_ns: ts,
        })?;

        let (pre_samples, pre_ts) = {
            let mut pre = self
                .shared
                .pre_roll
                .lock()
                .map_err(|_| CaptureError::provider("pre-roll lock poisoned"))?;
            pre.take()
        };
        if !pre_samples.is_empty() {
            let seq = self.shared.next_sequence.fetch_add(1, Ordering::SeqCst);
            let frame = AudioFrame::from_f32_le(seq, pre_ts.unwrap_or(ts), &pre_samples, true);
            events.send(CaptureEvent::Frame(frame))?;
        }

        if let Ok(mut conv) = self.shared.converter.lock() {
            conv.clear_pending();
        }

        self.shared.mode.store(MODE_RUNNING, Ordering::SeqCst);
        Ok(())
    }

    pub fn stop(&mut self, events: Option<CaptureEventSender>) -> Result<(), CaptureError> {
        self.shared.mode.store(MODE_IDLE, Ordering::SeqCst);
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        let ts = self.shared.now_ns();
        let sender = events.or_else(|| {
            self.shared
                .events
                .lock()
                .ok()
                .and_then(|mut slot| slot.take())
        });
        if let Some(events) = sender {
            let _ = events.send(CaptureEvent::Stopped {
                captured_at_ns: ts,
            });
        }
        Ok(())
    }
}

impl Drop for ActiveCapture {
    fn drop(&mut self) {
        let _ = self.stop(None);
    }
}

pub fn default_input_available() -> bool {
    cpal::default_host().default_input_device().is_some()
}

fn map_device_error(err: impl std::fmt::Display) -> CaptureError {
    let message = err.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("denied")
        || lower.contains("access")
        || lower.contains("permission")
        || lower.contains("not authorized")
    {
        CaptureError::PermissionDenied
    } else {
        CaptureError::Device(message)
    }
}

fn open_default_config() -> Result<(cpal::Device, StreamConfig, SampleFormat), CaptureError> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| CaptureError::Device("no default input device".into()))?;
    let supported = device.default_input_config().map_err(map_device_error)?;
    let sample_format = supported.sample_format();
    let config: StreamConfig = supported.into();
    Ok((device, config, sample_format))
}

fn run_capture_thread(
    events: Option<CaptureEventSender>,
    ready_tx: &mpsc::Sender<Result<Arc<SharedState>, CaptureError>>,
    stop_rx: mpsc::Receiver<()>,
) -> Result<(), CaptureError> {
    let (device, config, sample_format) = match open_default_config() {
        Ok(v) => v,
        Err(err) => {
            let _ = ready_tx.send(Err(err.clone()));
            return Err(err);
        }
    };

    let shared = Arc::new(SharedState::new(
        events,
        config.sample_rate.0,
        config.channels,
    ));
    let shared_cb = Arc::clone(&shared);
    let err_shared = Arc::clone(&shared);

    let stream = match sample_format {
        SampleFormat::F32 => build_stream::<f32>(&device, &config, shared_cb),
        SampleFormat::I16 => build_stream::<i16>(&device, &config, shared_cb),
        SampleFormat::U16 => build_stream::<u16>(&device, &config, shared_cb),
        other => {
            let err =
                CaptureError::Unsupported(format!("unsupported input sample format: {other:?}"));
            let _ = ready_tx.send(Err(err.clone()));
            return Err(err);
        }
    };

    let stream = match stream {
        Ok(s) => s,
        Err(err) => {
            let _ = ready_tx.send(Err(err.clone()));
            return Err(err);
        }
    };

    if let Err(err) = stream.play() {
        let mapped = map_device_error(err);
        let _ = ready_tx.send(Err(mapped.clone()));
        return Err(mapped);
    }

    if ready_tx.send(Ok(Arc::clone(&shared))).is_err() {
        return Ok(());
    }

    let _ = stop_rx.recv();
    drop(stream);
    let _ = err_shared;
    Ok(())
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    shared: Arc<SharedState>,
) -> Result<cpal::Stream, CaptureError>
where
    T: Sample + cpal::SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    let err_shared = Arc::clone(&shared);
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                on_input(data, &shared);
            },
            move |err| {
                let message = err.to_string();
                let ts = err_shared.now_ns();
                if let Ok(slot) = err_shared.events.lock() {
                    if let Some(events) = slot.as_ref() {
                        let _ = events.send(CaptureEvent::Error {
                            message,
                            captured_at_ns: ts,
                        });
                    }
                }
                err_shared.mode.store(MODE_IDLE, Ordering::SeqCst);
            },
            None,
        )
        .map_err(map_device_error)
}

fn on_input<T>(data: &[T], shared: &SharedState)
where
    T: Sample,
    f32: FromSample<T>,
{
    let mode = shared.mode.load(Ordering::Relaxed);
    if mode == MODE_IDLE {
        return;
    }

    let Ok(mut converter) = shared.converter.lock() else {
        return;
    };

    let mut converted = Vec::new();
    converter.push_interleaved(data, &mut converted);
    if converted.is_empty() {
        return;
    }

    let ts = shared.now_ns();
    match mode {
        MODE_ARMED => {
            drop(converter);
            if let Ok(mut pre) = shared.pre_roll.lock() {
                pre.push_f32(&converted, ts);
            }
        }
        MODE_RUNNING => {
            converter.stage_pending(&converted);
            let mut frames = Vec::new();
            while let Some(frame_samples) = converter.take_frame(FRAME_SAMPLES) {
                let seq = shared.next_sequence.fetch_add(1, Ordering::Relaxed);
                frames.push(AudioFrame::from_f32_le(seq, ts, &frame_samples, true));
            }
            drop(converter);
            if frames.is_empty() {
                return;
            }
            let Ok(events_guard) = shared.events.lock() else {
                return;
            };
            let Some(events) = events_guard.as_ref() else {
                return;
            };
            for frame in frames {
                if events.send(CaptureEvent::Frame(frame)).is_err() {
                    shared.mode.store(MODE_IDLE, Ordering::SeqCst);
                    return;
                }
            }
        }
        _ => {}
    }
}

/// Mono downmix + linear resample to [`CANONICAL_AUDIO_FORMAT`].
struct FormatConverter {
    in_rate: u32,
    in_channels: u16,
    /// Fractional read cursor relative to the start of the next mono chunk.
    phase: f64,
    prev: f32,
    has_prev: bool,
    /// Pending canonical samples awaiting frame emission.
    pending: Vec<f32>,
}

impl FormatConverter {
    fn new(in_rate: u32, in_channels: u16) -> Self {
        Self {
            in_rate: in_rate.max(1),
            in_channels: in_channels.max(1),
            phase: 0.0,
            prev: 0.0,
            has_prev: false,
            pending: Vec::new(),
        }
    }

    fn clear_pending(&mut self) {
        self.pending.clear();
    }

    fn stage_pending(&mut self, samples: &[f32]) {
        self.pending.extend_from_slice(samples);
    }

    fn take_frame(&mut self, frame_samples: usize) -> Option<Vec<f32>> {
        if self.pending.len() < frame_samples {
            return None;
        }
        Some(self.pending.drain(..frame_samples).collect())
    }

    fn push_interleaved<T>(&mut self, data: &[T], out: &mut Vec<f32>)
    where
        T: Sample,
        f32: FromSample<T>,
    {
        let channels = self.in_channels as usize;
        if channels == 0 || data.is_empty() {
            return;
        }

        let mut mono = Vec::with_capacity(data.len() / channels + 1);
        for frame in data.chunks_exact(channels) {
            let mut sum = 0.0_f32;
            for sample in frame {
                sum += f32::from_sample(*sample);
            }
            mono.push(sum / channels as f32);
        }

        if self.in_rate == CANONICAL_AUDIO_FORMAT.sample_rate_hz {
            out.extend_from_slice(&mono);
            if let Some(&last) = mono.last() {
                self.prev = last;
                self.has_prev = true;
            }
            return;
        }

        let step = self.in_rate as f64 / f64::from(CANONICAL_AUDIO_FORMAT.sample_rate_hz);
        let mut pos = self.phase;
        let len = mono.len();
        while (pos as usize) < len {
            let idx = pos.floor() as usize;
            let frac = (pos - idx as f64) as f32;
            // Across chunk boundaries, lerp from the previous chunk's last sample.
            let left = if idx == 0 && self.has_prev {
                self.prev
            } else {
                mono[idx.min(len - 1)]
            };
            let right = if idx + 1 < len {
                mono[idx + 1]
            } else {
                mono[idx.min(len - 1)]
            };
            out.push(left + (right - left) * frac);
            pos += step;
        }
        self.phase = pos - len as f64;
        if let Some(&last) = mono.last() {
            self.prev = last;
            self.has_prev = true;
        }
    }
}

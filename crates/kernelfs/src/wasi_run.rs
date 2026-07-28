//! Host-facing WASI guest runner with epoch tick, cancel, and stdio capture.
//!
//! Addresses the Lattice kernelfs issue: epoch ticker + cancel-friendly run
//! helper so agent hosts do not reinvent interrupt logic.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use wasmtime::{Engine, Linker, Module, Store, UpdateDeadline};
use wasmtime_wasi::pipe::MemoryOutputPipe;
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::{I32Exit, WasiCtxBuilder};

use crate::wasi_preopens::{configure_wasi_preopens, WasiPreopenError, WasiPreopenSpec};
use crate::wasi_runtime::{configure_store, engine_with_limits, WasmtimeLimits};

/// Default interval between [`Engine::increment_epoch`] ticks while a guest runs.
pub const DEFAULT_EPOCH_TICK_INTERVAL: Duration = Duration::from_millis(10);

/// Default capacity for captured stdout/stderr memory pipes.
pub const DEFAULT_STDIO_CAPTURE_CAPACITY: usize = 256 * 1024;

/// Options for [`run_wasi_guest`].
#[derive(Debug, Clone)]
pub struct WasiRunOptions {
    pub limits: WasmtimeLimits,
    /// How often a background thread calls [`Engine::increment_epoch`].
    /// Ignored when epoch interruption is disabled in [`WasmtimeLimits`].
    pub epoch_tick_interval: Duration,
    /// Wall-clock budget checked on each epoch callback. When elapsed, the
    /// guest traps with [`WasiRunError::EpochDeadline`].
    pub max_wall_time: Option<Duration>,
    /// When set and becomes `true`, the guest traps with [`WasiRunError::Cancelled`].
    pub cancel: Option<Arc<AtomicBool>>,
    /// Capture guest stdout/stderr into [`WasiRunResult`] (always on for the helper).
    pub stdio_capture_capacity: usize,
}

impl Default for WasiRunOptions {
    fn default() -> Self {
        Self {
            limits: WasmtimeLimits::default(),
            epoch_tick_interval: DEFAULT_EPOCH_TICK_INTERVAL,
            max_wall_time: Some(Duration::from_secs(5)),
            cancel: None,
            stdio_capture_capacity: DEFAULT_STDIO_CAPTURE_CAPACITY,
        }
    }
}

/// Successful guest invocation (including non-zero WASI `proc_exit` codes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasiRunResult {
    /// `0` when `_start` returned normally or via `proc_exit(0)`.
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Structured failures from [`run_wasi_guest`].
#[derive(Debug, thiserror::Error)]
pub enum WasiRunError {
    #[error("guest exhausted fuel")]
    FuelExhausted { stdout: Vec<u8>, stderr: Vec<u8> },
    #[error("guest hit epoch / wall-time deadline")]
    EpochDeadline { stdout: Vec<u8>, stderr: Vec<u8> },
    #[error("guest cancelled by host")]
    Cancelled { stdout: Vec<u8>, stderr: Vec<u8> },
    #[error("module missing `_start` export")]
    MissingStart,
    #[error("wasm trap: {message}")]
    Trap {
        message: String,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    #[error("failed to build wasm engine/module: {0}")]
    Engine(#[source] wasmtime::Error),
    #[error(transparent)]
    Preopen(#[from] WasiPreopenError),
}

/// Run a WASIp1 `_start` guest against a materialized KernelFS run root.
///
/// Configures preopens for `/input`, `/work`, `/output`, `/tmp`, applies
/// [`WasmtimeLimits`], captures stdout/stderr, and (when epoch interruption is
/// enabled) runs a background epoch ticker plus an epoch callback that honors
/// [`WasiRunOptions::cancel`] and [`WasiRunOptions::max_wall_time`].
pub fn run_wasi_guest(
    run_root: &Path,
    wasm: &[u8],
    options: &WasiRunOptions,
) -> Result<WasiRunResult, WasiRunError> {
    let spec = WasiPreopenSpec::from_run_root(run_root);
    let engine = engine_with_limits(&options.limits).map_err(WasiRunError::Engine)?;
    let module = Module::from_binary(&engine, wasm).map_err(WasiRunError::Engine)?;

    let mut linker: Linker<WasiP1Ctx> = Linker::new(&engine);
    preview1::add_to_linker_sync(&mut linker, |ctx| ctx).map_err(WasiRunError::Engine)?;
    let pre = linker
        .instantiate_pre(&module)
        .map_err(WasiRunError::Engine)?;

    let stdout_pipe = MemoryOutputPipe::new(options.stdio_capture_capacity);
    let stderr_pipe = MemoryOutputPipe::new(options.stdio_capture_capacity);
    // Clone handles so we can read contents after the store drops the WASI ctx.
    let stdout_reader = stdout_pipe.clone();
    let stderr_reader = stderr_pipe.clone();

    let mut builder = WasiCtxBuilder::new();
    configure_wasi_preopens(&mut builder, &spec)?;
    builder.stdout(stdout_pipe).stderr(stderr_pipe);
    let wasi = builder.build_p1();

    let mut store = Store::new(&engine, wasi);
    configure_store(&mut store, &options.limits).map_err(WasiRunError::Engine)?;

    let ticker_stop = Arc::new(AtomicBool::new(false));
    let _ticker = start_epoch_ticker(
        &engine,
        &options.limits,
        options.epoch_tick_interval,
        ticker_stop.clone(),
    );

    if options.limits.epoch_deadline_ticks.is_some() {
        let cancel = options.cancel.clone();
        let max_wall_time = options.max_wall_time;
        let started = Instant::now();
        store.epoch_deadline_callback(move |_store| {
            if cancel
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::SeqCst))
            {
                return Err(wasmtime::Error::msg("kernelfs: guest cancelled"));
            }
            if let Some(max) = max_wall_time {
                if started.elapsed() >= max {
                    return Err(wasmtime::Error::msg("kernelfs: epoch deadline"));
                }
            }
            Ok(UpdateDeadline::Continue(1))
        });
        // Deadline of 1 means the next host increment invokes the callback.
        store.set_epoch_deadline(1);
    }

    let instance = pre.instantiate(&mut store).map_err(WasiRunError::Engine)?;
    let start = match instance.get_typed_func::<(), ()>(&mut store, "_start") {
        Ok(func) => func,
        Err(_) => {
            ticker_stop.store(true, Ordering::SeqCst);
            return Err(WasiRunError::MissingStart);
        }
    };

    let call_result = start.call(&mut store, ());
    ticker_stop.store(true, Ordering::SeqCst);

    let stdout = stdout_reader.contents().to_vec();
    let stderr = stderr_reader.contents().to_vec();

    match call_result {
        Ok(()) => Ok(WasiRunResult {
            exit_code: 0,
            stdout,
            stderr,
        }),
        Err(err) => classify_guest_error(err, stdout, stderr, options.cancel.as_ref()),
    }
}

fn start_epoch_ticker(
    engine: &Engine,
    limits: &WasmtimeLimits,
    interval: Duration,
    stop: Arc<AtomicBool>,
) -> Option<thread::JoinHandle<()>> {
    if limits.epoch_deadline_ticks.is_none() {
        return None;
    }
    let engine = engine.clone();
    Some(thread::spawn(move || {
        while !stop.load(Ordering::SeqCst) {
            thread::sleep(interval);
            if stop.load(Ordering::SeqCst) {
                break;
            }
            engine.increment_epoch();
        }
    }))
}

fn classify_guest_error(
    err: wasmtime::Error,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    cancel: Option<&Arc<AtomicBool>>,
) -> Result<WasiRunResult, WasiRunError> {
    if let Some(exit) = err.downcast_ref::<I32Exit>() {
        return Ok(WasiRunResult {
            exit_code: exit.0,
            stdout,
            stderr,
        });
    }

    let message = format!("{err:#}");
    let lower = message.to_ascii_lowercase();

    if cancel.is_some_and(|flag| flag.load(Ordering::SeqCst))
        || lower.contains("kernelfs: guest cancelled")
    {
        return Err(WasiRunError::Cancelled { stdout, stderr });
    }
    if lower.contains("kernelfs: epoch deadline") || lower.contains("epoch") {
        return Err(WasiRunError::EpochDeadline { stdout, stderr });
    }
    if lower.contains("fuel") {
        return Err(WasiRunError::FuelExhausted { stdout, stderr });
    }

    Err(WasiRunError::Trap {
        message,
        stdout,
        stderr,
    })
}

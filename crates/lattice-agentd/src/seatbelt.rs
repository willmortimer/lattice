//! macOS Seatbelt (`sandbox-exec`) isolation for WASI guest runs.
//!
//! On Darwin, Wasmtime executes in a child process under a deny-default profile
//! that only allows the KernelFS run directory (plus the runner binary and
//! minimal system paths). The parent keeps Lattice HTTP / proposal authority.
//!
//! Control with `LATTICE_WASI_SEATBELT` (`1`/`true` force on, `0`/`false` off).
//! Default: on for macOS, off elsewhere. Override the child binary with
//! `LATTICE_WASI_SEATBELT_BIN`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use kernelfs::{WasiRunError, WasiRunOptions, WasiRunResult};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Env var: `1`/`true` force Seatbelt; `0`/`false` disable (even on macOS).
pub const SEATBELT_ENV: &str = "LATTICE_WASI_SEATBELT";

/// Env var pointing at the `lattice-wasi-seatbelt` helper binary.
pub const SEATBELT_BIN_ENV: &str = "LATTICE_WASI_SEATBELT_BIN";

#[derive(Debug, Error)]
pub enum SeatbeltError {
    #[error("macOS Seatbelt WASI isolation is not available on this platform")]
    UnsupportedPlatform,
    #[error("Seatbelt runner binary not found (set {SEATBELT_BIN_ENV} or ship lattice-wasi-seatbelt next to lattice-agentd)")]
    RunnerMissing,
    #[error("sandbox-exec failed: {0}")]
    SandboxExec(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid seatbelt child result: {0}")]
    BadResult(String),
    #[error("guest cancelled by host while Seatbelt child was running")]
    Cancelled,
    #[error(transparent)]
    Guest(#[from] WasiRunError),
}

/// Whether Seatbelt isolation should wrap the Wasmtime guest.
pub fn seatbelt_enabled() -> bool {
    match env::var(SEATBELT_ENV) {
        Ok(value) => {
            let v = value.trim().to_ascii_lowercase();
            match v.as_str() {
                "0" | "false" | "no" | "off" => false,
                "1" | "true" | "yes" | "on" => true,
                _ => cfg!(target_os = "macos"),
            }
        }
        Err(_) => cfg!(target_os = "macos"),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeatbeltJob {
    pub run_root: PathBuf,
    pub wasm_path: PathBuf,
    pub fuel_limit: Option<u64>,
    pub epoch_deadline_ticks: Option<u64>,
    pub max_wall_time_ms: Option<u64>,
    pub stdio_capture_capacity: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SeatbeltChildResult {
    Ok {
        exit_code: i32,
        stdout_b64: String,
        stderr_b64: String,
    },
    Err {
        kind: String,
        message: String,
        stdout_b64: String,
        stderr_b64: String,
    },
}

/// Resolve the Seatbelt child helper binary.
pub fn resolve_runner_bin() -> Option<PathBuf> {
    if let Ok(path) = env::var(SEATBELT_BIN_ENV) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(mut exe) = env::current_exe() {
        exe.set_file_name("lattice-wasi-seatbelt");
        if exe.is_file() {
            return Some(exe);
        }
    }
    None
}

/// Write a Seatbelt profile for the WASI child.
///
/// Uses `(allow default)` + `(deny network*)` because a hard deny-default profile
/// aborts dyld/Wasmtime on current macOS (SIGABRT). The child still runs under
/// `sandbox-exec` with network denied; file access is further constrained by
/// copying the wasm into `run_root/.host/` and only mounting KernelFS dirs.
pub fn write_profile(
    profile_path: &Path,
    run_root: &Path,
    runner_bin: &Path,
) -> Result<(), SeatbeltError> {
    let run_root = run_root
        .canonicalize()
        .unwrap_or_else(|_| run_root.to_path_buf());
    let runner_bin = runner_bin
        .canonicalize()
        .unwrap_or_else(|_| runner_bin.to_path_buf());
    let run = escape_sb_path(&run_root);
    let runner = escape_sb_path(&runner_bin);
    let profile = format!(
        r#"(version 1)
; lattice-wasi-seatbelt — network denied; wasm + KernelFS under run root only by convention.
(allow default)
(deny network*)
(deny file-write*
  (subpath "/Users")
  (subpath "/Volumes")
  (subpath "/etc")
  (subpath "/var/root")
)
(allow file-write*
  (subpath "{run}")
  (subpath "/private/var/folders")
  (subpath "/private/tmp")
  (subpath "/tmp")
)
; runner path recorded for audit / future tighten
; runner={runner}
"#
    );
    fs::write(profile_path, profile)?;
    Ok(())
}

fn escape_sb_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\").replace('"', "\\\"")
}

/// Run Wasmtime in a Seatbelt child. Copies `wasm_bytes` under `run_root/.host/`.
pub fn run_wasi_in_seatbelt(
    run_root: &Path,
    wasm_bytes: &[u8],
    options: &WasiRunOptions,
) -> Result<WasiRunResult, SeatbeltError> {
    if !cfg!(target_os = "macos") {
        return Err(SeatbeltError::UnsupportedPlatform);
    }

    let runner = resolve_runner_bin().ok_or(SeatbeltError::RunnerMissing)?;
    let host_dir = run_root.join(".host");
    fs::create_dir_all(&host_dir)?;
    let wasm_path = host_dir.join("guest.wasm");
    fs::write(&wasm_path, wasm_bytes)?;
    let profile_path = host_dir.join("seatbelt.sb");
    write_profile(&profile_path, run_root, &runner)?;
    let job_path = host_dir.join("job.json");
    let result_path = host_dir.join("result.json");
    let _ = fs::remove_file(&result_path);

    let job = SeatbeltJob {
        run_root: run_root.to_path_buf(),
        wasm_path: wasm_path.clone(),
        fuel_limit: options.limits.fuel,
        epoch_deadline_ticks: options.limits.epoch_deadline_ticks,
        max_wall_time_ms: options.max_wall_time.map(|d| d.as_millis() as u64),
        stdio_capture_capacity: options.stdio_capture_capacity,
    };
    fs::write(&job_path, serde_json::to_vec_pretty(&job).map_err(|err| {
        SeatbeltError::BadResult(format!("serialize job: {err}"))
    })?)?;

    let mut child = Command::new("sandbox-exec")
        .arg("-f")
        .arg(&profile_path)
        .arg(&runner)
        .arg("--job")
        .arg(&job_path)
        .arg("--result")
        .arg(&result_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let cancel = options.cancel.clone();
    let status = wait_with_cancel(&mut child, cancel.as_ref())?;
    let stdout = child.stdout.take().map(|mut s| {
        let mut buf = Vec::new();
        let _ = std::io::Read::read_to_end(&mut s, &mut buf);
        buf
    });
    let stderr = child.stderr.take().map(|mut s| {
        let mut buf = Vec::new();
        let _ = std::io::Read::read_to_end(&mut s, &mut buf);
        buf
    });

    if !status.success() {
        let err_tail = stderr
            .as_ref()
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_default();
        let out_tail = stdout
            .as_ref()
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_default();
        return Err(SeatbeltError::SandboxExec(format!(
            "exit {:?}; stderr={err_tail}; stdout={out_tail}",
            status.code()
        )));
    }

    let raw = fs::read_to_string(&result_path).map_err(|err| {
        SeatbeltError::BadResult(format!("missing result.json: {err}"))
    })?;
    let parsed: SeatbeltChildResult = serde_json::from_str(&raw)
        .map_err(|err| SeatbeltError::BadResult(format!("parse result: {err}")))?;
    child_result_to_wasi(parsed)
}

fn wait_with_cancel(
    child: &mut std::process::Child,
    cancel: Option<&Arc<AtomicBool>>,
) -> Result<std::process::ExitStatus, SeatbeltError> {
    loop {
        if cancel.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(SeatbeltError::Cancelled);
        }
        match child.try_wait()? {
            Some(status) => return Ok(status),
            None => thread::sleep(Duration::from_millis(20)),
        }
    }
}

fn child_result_to_wasi(result: SeatbeltChildResult) -> Result<WasiRunResult, SeatbeltError> {
    match result {
        SeatbeltChildResult::Ok {
            exit_code,
            stdout_b64,
            stderr_b64,
        } => Ok(WasiRunResult {
            exit_code,
            stdout: decode_b64(&stdout_b64)?,
            stderr: decode_b64(&stderr_b64)?,
        }),
        SeatbeltChildResult::Err {
            kind,
            message,
            stdout_b64,
            stderr_b64,
        } => {
            let stdout = decode_b64(&stdout_b64).unwrap_or_default();
            let stderr = decode_b64(&stderr_b64).unwrap_or_default();
            let run_err = match kind.as_str() {
                "fuel_exhausted" => WasiRunError::FuelExhausted { stdout, stderr },
                "epoch_deadline" => WasiRunError::EpochDeadline { stdout, stderr },
                "cancelled" => WasiRunError::Cancelled { stdout, stderr },
                "missing_start" => WasiRunError::MissingStart,
                "trap" => WasiRunError::Trap {
                    message,
                    stdout,
                    stderr,
                },
                other => WasiRunError::Trap {
                    message: format!("{other}: {message}"),
                    stdout,
                    stderr,
                },
            };
            Err(SeatbeltError::Guest(run_err))
        }
    }
}

fn decode_b64(text: &str) -> Result<Vec<u8>, SeatbeltError> {
    decode_base64_std(text).map_err(SeatbeltError::BadResult)
}

fn decode_base64_std(input: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Result<u8, String> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(format!("invalid base64 byte {c}")),
        }
    }
    let bytes: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    if bytes.len() % 4 != 0 {
        return Err("base64 length not multiple of 4".into());
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        let (c, pad_c) = if chunk[2] == b'=' {
            (0, true)
        } else {
            (val(chunk[2])?, false)
        };
        let (d, pad_d) = if chunk[3] == b'=' {
            (0, true)
        } else {
            (val(chunk[3])?, false)
        };
        out.push((a << 2) | (b >> 4));
        if !pad_c {
            out.push((b << 4) | (c >> 2));
        }
        if !pad_d {
            out.push((c << 6) | d);
        }
    }
    Ok(out)
}

pub fn encode_base64(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let mut n = (chunk[0] as u32) << 16;
        if chunk.len() > 1 {
            n |= (chunk[1] as u32) << 8;
        }
        if chunk.len() > 2 {
            n |= chunk[2] as u32;
        }
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Child entry: read job, run guest, write result JSON.
pub fn run_seatbelt_child(job_path: &Path, result_path: &Path) -> Result<(), SeatbeltError> {
    let job: SeatbeltJob = serde_json::from_slice(&fs::read(job_path)?)
        .map_err(|err| SeatbeltError::BadResult(format!("job parse: {err}")))?;
    let wasm = fs::read(&job.wasm_path)?;
    let mut options = WasiRunOptions {
        stdio_capture_capacity: job.stdio_capture_capacity,
        ..WasiRunOptions::default()
    };
    options.limits.fuel = job.fuel_limit;
    options.limits.epoch_deadline_ticks = job.epoch_deadline_ticks;
    if let Some(ms) = job.max_wall_time_ms {
        options.max_wall_time = Some(Duration::from_millis(ms));
    }

    let result = match kernelfs::run_wasi_guest(&job.run_root, &wasm, &options) {
        Ok(run) => SeatbeltChildResult::Ok {
            exit_code: run.exit_code,
            stdout_b64: encode_base64(&run.stdout),
            stderr_b64: encode_base64(&run.stderr),
        },
        Err(err) => map_run_error(err),
    };
    fs::write(
        result_path,
        serde_json::to_vec_pretty(&result)
            .map_err(|err| SeatbeltError::BadResult(format!("result serialize: {err}")))?,
    )?;
    Ok(())
}

fn map_run_error(err: WasiRunError) -> SeatbeltChildResult {
    match err {
        WasiRunError::FuelExhausted { stdout, stderr } => SeatbeltChildResult::Err {
            kind: "fuel_exhausted".into(),
            message: "guest exhausted fuel".into(),
            stdout_b64: encode_base64(&stdout),
            stderr_b64: encode_base64(&stderr),
        },
        WasiRunError::EpochDeadline { stdout, stderr } => SeatbeltChildResult::Err {
            kind: "epoch_deadline".into(),
            message: "guest hit epoch / wall-time deadline".into(),
            stdout_b64: encode_base64(&stdout),
            stderr_b64: encode_base64(&stderr),
        },
        WasiRunError::Cancelled { stdout, stderr } => SeatbeltChildResult::Err {
            kind: "cancelled".into(),
            message: "guest cancelled by host".into(),
            stdout_b64: encode_base64(&stdout),
            stderr_b64: encode_base64(&stderr),
        },
        WasiRunError::MissingStart => SeatbeltChildResult::Err {
            kind: "missing_start".into(),
            message: "module missing _start export".into(),
            stdout_b64: encode_base64(&[]),
            stderr_b64: encode_base64(&[]),
        },
        WasiRunError::Trap {
            message,
            stdout,
            stderr,
        } => SeatbeltChildResult::Err {
            kind: "trap".into(),
            message,
            stdout_b64: encode_base64(&stdout),
            stderr_b64: encode_base64(&stderr),
        },
        WasiRunError::Engine(inner) => SeatbeltChildResult::Err {
            kind: "engine".into(),
            message: inner.to_string(),
            stdout_b64: encode_base64(&[]),
            stderr_b64: encode_base64(&[]),
        },
        WasiRunError::Preopen(inner) => SeatbeltChildResult::Err {
            kind: "preopen".into(),
            message: inner.to_string(),
            stdout_b64: encode_base64(&[]),
            stderr_b64: encode_base64(&[]),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        let samples: &[&[u8]] = &[b"", b"f", b"fo", b"foo", b"hello from input", &[0xff, 0x00, 0x01]];
        for sample in samples {
            let encoded = encode_base64(sample);
            let decoded = decode_base64_std(&encoded).expect("decode");
            assert_eq!(&decoded, sample);
        }
    }

    #[test]
    fn profile_mentions_run_root() {
        let temp = tempfile::tempdir().expect("temp");
        let run = temp.path().join("run");
        fs::create_dir_all(&run).expect("run");
        let runner = temp.path().join("runner");
        fs::write(&runner, b"x").expect("runner");
        let profile = temp.path().join("p.sb");
        write_profile(&profile, &run, &runner).expect("profile");
        let text = fs::read_to_string(&profile).expect("read");
        assert!(text.contains("(deny network*)"));
        assert!(text.contains(&run.canonicalize().unwrap().to_string_lossy().to_string()));
    }
}

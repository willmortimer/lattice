//! macOS Seatbelt (`sandbox-exec`) isolation for WASI guest runs.
//!
//! On Darwin, Wasmtime executes in a child process under a **deny-default**
//! profile that only allows the KernelFS run directory, the runner binary, and
//! the minimum dyld/Wasmtime system paths. Network is denied. The parent keeps
//! Lattice HTTP / proposal authority.
//!
//! Missing `lattice-wasi-seatbelt` is a hard error (`SeatbeltError::RunnerMissing`);
//! the host never falls back to in-process Wasmtime while Seatbelt is enabled.
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
#[cfg(test)]
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use kernelfs::{WasiRunError, WasiRunOptions, WasiRunResult};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Env var: `1`/`true` force Seatbelt; `0`/`false` disable (even on macOS).
pub const SEATBELT_ENV: &str = "LATTICE_WASI_SEATBELT";

/// Env var pointing at the `lattice-wasi-seatbelt` helper binary.
pub const SEATBELT_BIN_ENV: &str = "LATTICE_WASI_SEATBELT_BIN";

/// Serializes tests that mutate Seatbelt env vars (lib tests share one process).
#[cfg(test)]
pub(crate) static SEATBELT_ENV_LOCK: Mutex<()> = Mutex::new(());

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
///
/// An explicit `LATTICE_WASI_SEATBELT_BIN` that is not a file fails closed
/// (no search next to `current_exe`). Unset env still looks beside the agentd
/// binary so a shipped sidecar install works without extra config.
pub fn resolve_runner_bin() -> Option<PathBuf> {
    match env::var(SEATBELT_BIN_ENV) {
        Ok(path) => {
            let path = PathBuf::from(path.trim());
            if path.is_file() {
                Some(path)
            } else {
                None
            }
        }
        Err(_) => {
            if let Ok(mut exe) = env::current_exe() {
                exe.set_file_name("lattice-wasi-seatbelt");
                if exe.is_file() {
                    return Some(exe);
                }
            }
            None
        }
    }
}

/// Write a deny-default Seatbelt profile for the WASI child.
///
/// Each allow is the measured minimum for dyld + Wasmtime + KernelFS on current
/// macOS. `(allow default)` SIGABRTs are not an acceptable substitute.
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
; lattice-wasi-seatbelt — deny-default; network denied; KernelFS run dir + dyld/Wasmtime.
(deny default)
(deny network*)
; dyld stats the filesystem root inode. (subpath "/usr") does not match "/";
; without this literal, even /usr/bin/true SIGABRTs under deny-default.
(allow file-read* (literal "/"))
; System libraries, frameworks, and the dyld shared cache.
(allow file-read*
  (subpath "/usr")
  (subpath "/System")
  (subpath "/Library")
  (subpath "/private/var/db/dyld")
)
; Runner binary (often a user-owned cargo/target path, not under /usr).
(allow file-read* (literal "{runner}"))
; KernelFS run dir: job, wasm, preopens, output.
(allow file-read* (subpath "{run}"))
; Guest output plus OS temp (tempfile / Cranelift scratch).
(allow file-write*
  (subpath "{run}")
  (subpath "/private/tmp")
  (subpath "/private/var/folders")
  (subpath "/tmp")
)
; sandbox-exec applies the profile then execs the helper.
(allow process-exec (literal "{runner}"))
; libsystem/Wasmtime may fork worker threads' backing processes.
(allow process-fork)
; Rust/Wasmtime thread stack guard pages: omitting this SIGABRTs
; ("failed to allocate a guard page") when the helper runs a WASI guest.
(allow sysctl-read)
; libsystem abort and signal plumbing inside the sandboxed helper.
(allow signal)
; dyld/libsystem mach bootstrap; deny can SIGABRT the helper on some macOS versions.
(allow mach-lookup)
"#
    );
    fs::write(profile_path, profile)?;
    Ok(())
}

fn escape_sb_path(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
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
    // Seatbelt `(subpath)` matches real paths; tempfile's `/var/folders` is a
    // symlink to `/private/var/folders` and is denied unless canonicalized.
    let run_root = run_root
        .canonicalize()
        .unwrap_or_else(|_| run_root.to_path_buf());
    let runner = runner.canonicalize().unwrap_or_else(|_| runner);
    let host_dir = run_root.join(".host");
    fs::create_dir_all(&host_dir)?;
    let wasm_path = host_dir.join("guest.wasm");
    fs::write(&wasm_path, wasm_bytes)?;
    let profile_path = host_dir.join("seatbelt.sb");
    write_profile(&profile_path, &run_root, &runner)?;
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
    fs::write(
        &job_path,
        serde_json::to_vec_pretty(&job)
            .map_err(|err| SeatbeltError::BadResult(format!("serialize job: {err}")))?,
    )?;

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

    let raw = fs::read_to_string(&result_path)
        .map_err(|err| SeatbeltError::BadResult(format!("missing result.json: {err}")))?;
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
    let bytes: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
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
        let samples: &[&[u8]] = &[
            b"",
            b"f",
            b"fo",
            b"foo",
            b"hello from input",
            &[0xff, 0x00, 0x01],
        ];
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
        assert!(
            text.contains("(deny default)"),
            "expected deny-default profile, got:\n{text}"
        );
        assert!(
            !text.contains("(allow default)"),
            "deny-default profile must not include (allow default):\n{text}"
        );
        assert!(text.contains("(deny network*)"));
        let run_canon = run.canonicalize().unwrap().to_string_lossy().into_owned();
        assert!(
            text.contains(&run_canon),
            "profile should mention run root {run_canon}"
        );
        let runner_canon = runner
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            text.contains(&runner_canon),
            "profile should mention runner {runner_canon}"
        );
    }

    struct EnvRestore {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvRestore {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var_os(key);
            env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => env::set_var(self.key, value),
                None => env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn missing_runner_fails_closed() {
        let _lock = SEATBELT_ENV_LOCK
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let _seatbelt = EnvRestore::set(SEATBELT_ENV, "1");
        let _bin = EnvRestore::set(
            SEATBELT_BIN_ENV,
            "/nonexistent/lattice-wasi-seatbelt-missing",
        );

        let temp = tempfile::tempdir().expect("temp");
        let run = temp.path().join("run");
        fs::create_dir_all(&run).expect("run");
        let err = run_wasi_in_seatbelt(&run, b"\0asm\x01\x00\x00\x00", &WasiRunOptions::default())
            .expect_err("missing runner must fail closed");
        if cfg!(target_os = "macos") {
            assert!(
                matches!(err, SeatbeltError::RunnerMissing),
                "expected RunnerMissing, got {err:?}"
            );
        } else {
            assert!(
                matches!(err, SeatbeltError::UnsupportedPlatform),
                "forced Seatbelt on non-macOS must be UnsupportedPlatform, got {err:?}"
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn sandbox_exec_accepts_deny_default_profile() {
        let temp = tempfile::tempdir().expect("temp");
        let run = temp.path().join("run");
        fs::create_dir_all(&run).expect("run");
        let profile = temp.path().join("p.sb");
        let runner = Path::new("/usr/bin/true");
        write_profile(&profile, &run, runner).expect("profile");
        let text = fs::read_to_string(&profile).expect("read");
        assert!(text.contains("(deny default)"));
        assert!(!text.contains("(allow default)"));

        let output = Command::new("sandbox-exec")
            .arg("-f")
            .arg(&profile)
            .arg(runner)
            .output()
            .expect("sandbox-exec");
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_ne!(
                output.status.signal(),
                Some(6),
                "deny-default profile SIGABRT stderr={}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert!(
            output.status.success(),
            "sandbox-exec -f profile /usr/bin/true failed: code={:?} stderr={} stdout={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
    }
}

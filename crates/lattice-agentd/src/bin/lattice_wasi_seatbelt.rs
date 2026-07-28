//! Seatbelt-isolated WASI guest runner (`sandbox-exec` child).

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use lattice_agentd::seatbelt::{run_seatbelt_child, SeatbeltError};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let mut job: Option<PathBuf> = None;
    let mut result: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--job" => job = args.next().map(PathBuf::from),
            "--result" => result = args.next().map(PathBuf::from),
            other => {
                eprintln!("lattice-wasi-seatbelt: unknown arg {other}");
                return ExitCode::from(2);
            }
        }
    }
    let (Some(job), Some(result)) = (job, result) else {
        eprintln!("usage: lattice-wasi-seatbelt --job <path> --result <path>");
        return ExitCode::from(2);
    };

    match run_seatbelt_child(&job, &result) {
        Ok(()) => ExitCode::SUCCESS,
        Err(SeatbeltError::Cancelled) => ExitCode::from(130),
        Err(err) => {
            eprintln!("lattice-wasi-seatbelt: {err}");
            ExitCode::FAILURE
        }
    }
}

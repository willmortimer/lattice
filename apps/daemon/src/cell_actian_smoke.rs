//! Actian VectorAI smoke test against the Cell host gRPC relay (`127.0.0.1:16574`).
//!
//! TCP readiness is checked in Rust; collection upsert/search uses the Python SDK
//! over gRPC when available (no published Rust protos yet).

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// Actian gRPC endpoint URL (host relay on macOS Apple VZ).
pub const ENV_LATTICE_ACTIAN_URL: &str = "LATTICE_ACTIAN_URL";
/// Default host relay from `cell-host-macos` (guest `:6574` via vsock).
pub const DEFAULT_ACTIAN_URL: &str = "http://127.0.0.1:16574";
/// Default relay port when the URL omits an explicit port.
pub const DEFAULT_ACTIAN_PORT: u16 = 16574;
/// Smoke collection name (ephemeral lab use).
pub const SMOKE_COLLECTION: &str = "lattice_smoke";
/// Vector dimension for the lattice Actian profile (matches `LATTICE_EMBEDDING_DIMENSIONS=512`).
pub const SMOKE_VECTOR_DIM: usize = 512;

const TCP_TIMEOUT: Duration = Duration::from_secs(3);

/// One labeled step in the smoke report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellActianSmokeStep {
    pub name: String,
    pub ok: bool,
    pub detail: Option<String>,
}

/// Aggregated smoke outcome for daemon / Tauri surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellActianSmokeResult {
    pub ok: bool,
    pub steps: Vec<CellActianSmokeStep>,
    pub error: Option<String>,
}

/// Resolved gRPC dial target (`host:port`) and socket address for TCP probes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActianEndpoint {
    pub dial: String,
    pub socket_addr: SocketAddr,
    pub display: String,
}

/// Resolve `LATTICE_ACTIAN_URL` (or default) into dial + TCP probe addresses.
pub fn resolve_actian_endpoint() -> ActianEndpoint {
    let raw = std::env::var(ENV_LATTICE_ACTIAN_URL)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_ACTIAN_URL.to_string());
    parse_actian_url(&raw)
}

/// Parse `http://`, `grpc://`, or bare `host:port` Actian URLs.
pub fn parse_actian_url(raw: &str) -> ActianEndpoint {
    let trimmed = raw.trim();
    let without_scheme = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))
        .or_else(|| trimmed.strip_prefix("grpc://"))
        .unwrap_or(trimmed);

    let (host, port) = if let Some((host, port_str)) = without_scheme.rsplit_once(':') {
        let port = port_str
            .parse::<u16>()
            .unwrap_or(DEFAULT_ACTIAN_PORT);
        (host.to_string(), port)
    } else {
        (without_scheme.to_string(), DEFAULT_ACTIAN_PORT)
    };

    let dial = format!("{host}:{port}");
    let socket_addr = resolve_socket_addr(&host, port).unwrap_or_else(|_| {
        format!("{host}:{port}")
            .parse()
            .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], port)))
    });

    ActianEndpoint {
        dial,
        socket_addr,
        display: format!("{host}:{port}"),
    }
}

fn resolve_socket_addr(host: &str, port: u16) -> std::io::Result<SocketAddr> {
    let addrs: Vec<SocketAddr> = (host, port).to_socket_addrs()?.collect();
    addrs
        .into_iter()
        .find(|addr| addr.is_ipv4())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no IPv4 address for {host}:{port}"),
            )
        })
}

/// Run the full smoke sequence (TCP then optional SDK steps).
pub fn run_cell_actian_smoke() -> CellActianSmokeResult {
    let endpoint = resolve_actian_endpoint();
    let mut steps = Vec::new();

    let tcp_step = tcp_connect_step(&endpoint);
    let tcp_ok = tcp_step.ok;
    steps.push(tcp_step);

    if !tcp_ok {
        return CellActianSmokeResult {
            ok: false,
            steps,
            error: Some(format!(
                "Actian gRPC relay not reachable at {} — ensure cell-host-macos is running \
                 with the vsock forward to guest :6574 (set {ENV_LATTICE_ACTIAN_URL} to override)",
                endpoint.display
            )),
        };
    }

    match run_sdk_smoke(&endpoint.dial) {
        Ok(mut sdk_steps) => {
            steps.append(&mut sdk_steps);
            let ok = steps.iter().all(|step| step.ok);
            let error = if ok {
                None
            } else {
                Some("one or more Actian SDK smoke steps failed".into())
            };
            CellActianSmokeResult { ok, steps, error }
        }
        Err(message) => {
            steps.push(CellActianSmokeStep {
                name: "sdk".into(),
                ok: false,
                detail: Some(message.clone()),
            });
            CellActianSmokeResult {
                ok: false,
                steps,
                error: Some(message),
            }
        }
    }
}

fn tcp_connect_step(endpoint: &ActianEndpoint) -> CellActianSmokeStep {
    match TcpStream::connect_timeout(&endpoint.socket_addr, TCP_TIMEOUT) {
        Ok(_) => CellActianSmokeStep {
            name: "tcp_connect".into(),
            ok: true,
            detail: Some(format!("connected to {}", endpoint.display)),
        },
        Err(err) => CellActianSmokeStep {
            name: "tcp_connect".into(),
            ok: false,
            detail: Some(format!(
                "connect {} failed: {err} (is the host relay listening on :{}?)",
                endpoint.display, endpoint.socket_addr.port()
            )),
        },
    }
}

fn run_sdk_smoke(dial: &str) -> Result<Vec<CellActianSmokeStep>, String> {
    let script = sdk_script_path();
    if !script.is_file() {
        return Err(format!(
            "Actian SDK smoke script missing at {} (rebuild lattice-daemon)",
            script.display()
        ));
    }

    let python = resolve_python()?;
    let output = Command::new(&python)
        .arg(&script)
        .arg(dial)
        .arg(SMOKE_COLLECTION)
        .arg(SMOKE_VECTOR_DIM.to_string())
        .output()
        .map_err(|err| format!("failed to spawn {python} for Actian SDK smoke: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        if detail.contains("No module named 'actian_vectorai'")
            || detail.contains("ModuleNotFoundError")
        {
            return Err(
                "actian-vectorai Python package not installed \
                 (`pip install actian-vectorai-client`); TCP relay is up but gRPC smoke needs the SDK"
                    .into(),
            );
        }
        return Err(if detail.is_empty() {
            format!("Actian SDK smoke exited with {}", output.status)
        } else {
            detail
        });
    }

    let parsed: SdkSmokePayload = serde_json::from_slice(&output.stdout).map_err(|err| {
        format!(
            "Actian SDK smoke returned invalid JSON: {err} (stdout: {})",
            String::from_utf8_lossy(&output.stdout)
        )
    })?;

    Ok(parsed
        .steps
        .into_iter()
        .map(|step| CellActianSmokeStep {
            name: step.name,
            ok: step.ok,
            detail: step.detail,
        })
        .collect())
}

fn sdk_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/cell_actian_smoke_sdk.py")
}

fn resolve_python() -> Result<String, String> {
    for candidate in ["python3", "python"] {
        if Command::new(candidate)
            .arg("-c")
            .arg("import sys; sys.exit(0)")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Ok(candidate.to_string());
        }
    }
    Err("python3 not found on PATH (required for Actian gRPC smoke)".into())
}

#[derive(Debug, serde::Deserialize)]
struct SdkSmokePayload {
    steps: Vec<SdkSmokeStep>,
}

#[derive(Debug, serde::Deserialize)]
struct SdkSmokeStep {
    name: String,
    ok: bool,
    detail: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn parse_actian_url_defaults_port() {
        let endpoint = parse_actian_url("http://127.0.0.1");
        assert_eq!(endpoint.dial, "127.0.0.1:16574");
        assert_eq!(endpoint.socket_addr.port(), DEFAULT_ACTIAN_PORT);
    }

    #[test]
    fn parse_actian_url_grpc_scheme() {
        let endpoint = parse_actian_url("grpc://127.0.0.1:16574");
        assert_eq!(endpoint.dial, "127.0.0.1:16574");
        assert_eq!(endpoint.display, "127.0.0.1:16574");
    }

    #[test]
    fn tcp_connect_fails_when_port_closed() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);

        let endpoint = ActianEndpoint {
            dial: format!("127.0.0.1:{}", addr.port()),
            socket_addr: addr,
            display: format!("127.0.0.1:{}", addr.port()),
        };
        let step = tcp_connect_step(&endpoint);
        assert!(!step.ok);
        assert_eq!(step.name, "tcp_connect");
        let detail = step.detail.expect("detail");
        assert!(detail.contains(&addr.port().to_string()));
    }

    #[test]
    fn smoke_fails_loudly_when_relay_closed() {
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let addr = listener.local_addr().expect("local addr");
            addr.port()
        };
        let prev = std::env::var(ENV_LATTICE_ACTIAN_URL).ok();
        std::env::set_var(ENV_LATTICE_ACTIAN_URL, format!("http://127.0.0.1:{port}"));
        let result = run_cell_actian_smoke();
        if let Some(value) = prev {
            std::env::set_var(ENV_LATTICE_ACTIAN_URL, value);
        } else {
            std::env::remove_var(ENV_LATTICE_ACTIAN_URL);
        }

        assert!(!result.ok);
        assert_eq!(result.steps.len(), 1);
        assert_eq!(result.steps[0].name, "tcp_connect");
        let error = result.error.expect("error");
        assert!(error.contains(":16574") || error.contains(&port.to_string()));
        assert!(error.contains("relay"));
    }

    #[test]
    fn sdk_script_path_exists() {
        assert!(sdk_script_path().is_file());
    }
}

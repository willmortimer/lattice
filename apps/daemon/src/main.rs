use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use lattice_daemon::{
    default_socket_path, mcp, serve_with_shutdown_and_controllers, DaemonConfig, DaemonPreferences,
    SemanticController, SemanticProviderMode, VoiceController, VoiceProviderMode, DEFAULT_API_PORT,
};
use lattice_runtime::LatticeRuntime;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(
    name = "latticed",
    version,
    about = "Lattice daemon: private Unix-domain control plane + localhost API/MCP"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Unix-domain socket path.
    #[arg(long, default_value_os_t = default_socket_path())]
    socket: PathBuf,

    /// Shared authentication token for the connection handshake and local API.
    #[arg(long, env = "LATTICE_AUTH_TOKEN")]
    auth_token: Option<String>,

    /// Optional fixed instance id (default: UUIDv7).
    #[arg(long)]
    instance_id: Option<String>,

    /// Localhost HTTP API port (127.0.0.1 only). Pass 0 to disable.
    #[arg(long, default_value_t = DEFAULT_API_PORT)]
    api_port: u16,

    /// Remain running after the last client disconnects (overrides profile preference).
    #[arg(long)]
    keep_services_running: bool,

    /// Seconds of idle time after the last client disconnects before exit when
    /// keep-services-running is false (default 30).
    #[arg(long)]
    idle_shutdown_secs: Option<u64>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Serve MCP tools over stdio (search/read/related/build_context).
    Mcp {
        /// Shared auth token (informational / launcher parity with the HTTP API).
        #[arg(long, env = "LATTICE_AUTH_TOKEN")]
        auth_token: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    if let Some(Commands::Mcp { auth_token }) = cli.command {
        let token = auth_token
            .or(cli.auth_token)
            .unwrap_or_else(|| Uuid::now_v7().to_string());
        let runtime = Arc::new(LatticeRuntime::new());
        mcp::serve_stdio(runtime, &token).context("mcp stdio serve failed")?;
        return Ok(());
    }

    let auth_token_provided = cli.auth_token.is_some();
    let auth_token = cli.auth_token.unwrap_or_else(|| Uuid::now_v7().to_string());
    if !auth_token_provided {
        // Surface generated tokens for interactive launches; spawn helpers pass --auth-token.
        tracing::info!(%auth_token, "generated auth token (pass --auth-token to pin)");
    }

    let mut config = DaemonConfig::new(cli.socket, auth_token);
    if let Some(instance_id) = cli.instance_id {
        config = config.with_instance_id(instance_id);
    }
    config = config.with_api_port(if cli.api_port == 0 {
        None
    } else {
        Some(cli.api_port)
    });

    let prefs = DaemonPreferences::load();
    let keep_services_running = cli.keep_services_running || prefs.keep_services_running;
    let idle_shutdown_timeout = cli
        .idle_shutdown_secs
        .map(std::time::Duration::from_secs)
        .unwrap_or(prefs.idle_shutdown_timeout);
    config = config
        .with_keep_services_running(keep_services_running)
        .with_idle_shutdown_timeout(idle_shutdown_timeout);

    if keep_services_running {
        tracing::info!("keep-services-running enabled; daemon will stay up after clients disconnect");
    } else {
        tracing::info!(
            secs = idle_shutdown_timeout.as_secs(),
            "idle shutdown enabled after last client disconnects"
        );
    }

    let runtime = Arc::new(LatticeRuntime::new());
    // Always own a semantic controller so EnableSemanticSearch works without env
    // gates. Env still selects Fake / ExternalSocket / SpawnHost.
    let mode = SemanticProviderMode::from_env_or_fake();
    tracing::info!(?mode, "semantic controller ready for user-driven enable");
    let semantic = Some(
        SemanticController::start(Arc::clone(&runtime), mode)
            .context("start semantic controller")?,
    );
    let voice = match VoiceProviderMode::from_env() {
        Some(mode) => {
            tracing::info!("voice-host supervision enabled via environment");
            Some(
                VoiceController::start(mode)
                    .await
                    .context("start voice controller")?,
            )
        }
        None => None,
    };

    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            if let Err(err) = wait_for_shutdown_signal().await {
                tracing::warn!(error = %err, "signal handler failed");
            }
            let _ = tx.send(());
        });
        serve_with_shutdown_and_controllers(config, runtime, semantic, voice, rx)
            .await
            .context("latticed serve failed")?;
    }
    Ok(())
}

async fn wait_for_shutdown_signal() -> std::io::Result<()> {
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    tokio::select! {
        _ = sigterm.recv() => {}
        _ = sigint.recv() => {}
    }
    Ok(())
}

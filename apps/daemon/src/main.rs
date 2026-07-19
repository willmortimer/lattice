use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use lattice_daemon::{
    default_socket_path, mcp, serve, serve_with_shutdown_and_semantic, DaemonConfig,
    SemanticController, SemanticProviderMode, DEFAULT_API_PORT,
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

    let runtime = Arc::new(LatticeRuntime::new());
    match SemanticProviderMode::from_env() {
        Some(mode) => {
            tracing::info!("semantic indexing enabled via environment");
            let semantic = SemanticController::start(Arc::clone(&runtime), mode)
                .context("start semantic controller")?;
            let (tx, rx) = tokio::sync::oneshot::channel();
            tokio::spawn(async move {
                if let Err(err) = wait_for_shutdown_signal().await {
                    tracing::warn!(error = %err, "signal handler failed");
                }
                let _ = tx.send(());
            });
            serve_with_shutdown_and_semantic(config, runtime, Some(semantic), rx)
                .await
                .context("latticed serve failed")?;
        }
        None => {
            serve(config, runtime)
                .await
                .context("latticed serve failed")?;
        }
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

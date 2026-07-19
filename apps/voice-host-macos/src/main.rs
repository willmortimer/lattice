use std::path::PathBuf;

use clap::{Parser, Subcommand};
use lattice_voice_host::{run_server, BackendKind, HostConfig, HostState, PROTOCOL_VERSION};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "lattice-voice-host",
    about = "Isolated voice inference host for Lattice"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Serve the private Unix-domain socket protocol.
    Serve {
        /// Socket path (private UDS; never a public TCP port).
        #[arg(long)]
        socket: PathBuf,

        /// Backend: `fake` (default, always available) or `fluidaudio` (feature-gated).
        #[arg(long, default_value = "fake")]
        backend: String,

        /// Optional FluidAudio / Parakeet model cache directory.
        #[arg(long)]
        model_cache_dir: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    match Cli::parse().command {
        Command::Serve {
            socket,
            backend,
            model_cache_dir,
        } => {
            let backend = BackendKind::parse(&backend)?;
            let config = HostConfig::new(socket, backend, model_cache_dir);
            tracing::info!(
                protocol_version = PROTOCOL_VERSION,
                backend = backend.as_str(),
                instance_id = %config.instance_id,
                "starting lattice-voice-host"
            );
            let state = HostState::new(config)?;
            run_server(state).await?;
            Ok(())
        }
    }
}

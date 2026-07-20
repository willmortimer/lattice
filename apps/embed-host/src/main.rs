use std::path::PathBuf;

use clap::{Parser, Subcommand};
use lattice_embed_host::{
    install_model, run_server, BackendKind, HostConfig, HostState, PROTOCOL_VERSION,
};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "lattice-embed-host",
    about = "Isolated embedding inference host for Lattice"
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

        /// Backend: `fake` (default, always available) or `llama-cpp` (feature-gated).
        #[arg(long, default_value = "fake")]
        backend: String,

        /// Models directory root for install/load.
        #[arg(long)]
        models_dir: PathBuf,
    },
    /// Explicitly install a local model artifact (sha256 verified; no download).
    Install {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        artifact: PathBuf,
        #[arg(long)]
        models_dir: PathBuf,
    },
    /// Print backends compiled into this binary (one name per line).
    Backends,
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
            models_dir,
        } => {
            let backend = BackendKind::parse(&backend)?;
            let config = HostConfig::new(socket, backend, models_dir);
            tracing::info!(
                protocol_version = PROTOCOL_VERSION,
                backend = backend.as_str(),
                instance_id = %config.instance_id,
                "starting lattice-embed-host"
            );
            let state = HostState::new(config);
            run_server(state).await?;
            Ok(())
        }
        Command::Install {
            manifest,
            artifact,
            models_dir,
        } => {
            let result = install_model(&manifest, &artifact, &models_dir)?;
            println!(
                "installed model_dir={} sha256={}",
                result.model_dir.display(),
                result.artifact_sha256
            );
            Ok(())
        }
        Command::Backends => {
            for name in BackendKind::available() {
                println!("{name}");
            }
            Ok(())
        }
    }
}

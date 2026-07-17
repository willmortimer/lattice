use clap::Parser;
use lattice_bridge::{serve, BridgeState};

#[derive(Debug, Parser)]
#[command(
    name = "lattice-bridge",
    version,
    about = "Localhost HTTP bridge over lattice-handlers for the browser demo"
)]
struct Cli {
    /// Bind address (default: 127.0.0.1).
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Listen port (default: 8787).
    #[arg(long, default_value_t = 8787)]
    port: u16,

    /// Default workspace root for routes that accept an optional `root` field.
    #[arg(long)]
    root: Option<String>,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    if let Some(root) = &cli.root {
        tracing::info!(%root, "default workspace root configured");
    }

    serve(
        &cli.host,
        cli.port,
        BridgeState {
            default_root: cli.root,
        },
    )
    .await
}

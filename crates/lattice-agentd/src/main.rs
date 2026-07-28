//! `lattice-agentd` — JSONL agent sidecar over stdin/stdout.

use lattice_agentd::{run_jsonl_loop, LoopConfig};
use tokio::io::{stdin, stdout, BufReader};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let reader = BufReader::new(stdin());
    let writer = stdout();

    if let Err(err) = run_jsonl_loop(reader, writer, LoopConfig::default()).await {
        eprintln!("lattice-agentd fatal: {err}");
        std::process::exit(1);
    }
}

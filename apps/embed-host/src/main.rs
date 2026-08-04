use std::path::PathBuf;

use clap::{Parser, Subcommand};
use lattice_embed_host::{
    install_model, run_server, BackendKind, HostConfig, HostState, PROTOCOL_VERSION,
};
#[cfg(feature = "llama-cpp")]
use lattice_embed_host::{run_llama_bench, BenchStats, BENCH_ENV_GGUF};
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
    /// Time query embeddings against a local GGUF (llama-cpp; not run in CI).
    #[cfg(feature = "llama-cpp")]
    Bench {
        /// Path to the pinned Qwen3 GGUF (or set `LATTICE_EMBED_LLAMA_GGUF`).
        #[arg(long)]
        gguf: Option<PathBuf>,

        /// Matryoshka output dimensions.
        #[arg(long, default_value = "512")]
        dimensions: u32,

        /// Warm-up iterations (not timed).
        #[arg(long, default_value = "3")]
        warmup: u32,

        /// Timed query iterations.
        #[arg(long, default_value = "20")]
        iterations: u32,

        /// Probe query text.
        #[arg(long, default_value = "capability grants for plugins")]
        query: String,

        /// Emit JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
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
        #[cfg(feature = "llama-cpp")]
        Command::Bench {
            gguf,
            dimensions,
            warmup,
            iterations,
            query,
            json,
        } => {
            let gguf_path = resolve_bench_gguf(gguf)?;
            let stats = run_llama_bench(
                &gguf_path,
                dimensions,
                warmup,
                iterations,
                &query,
            )
            .await?;
            print_bench_stats(&stats, json);
            Ok(())
        }
    }
}

#[cfg(feature = "llama-cpp")]
fn resolve_bench_gguf(flag: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(path) = flag {
        return Ok(path);
    }
    let env = std::env::var(BENCH_ENV_GGUF).map_err(|_| {
        anyhow::anyhow!(
            "missing GGUF path: pass --gguf or set {BENCH_ENV_GGUF} to a verified Qwen3-Embedding-0.6B-Q8_0.gguf"
        )
    })?;
    Ok(PathBuf::from(env))
}

#[cfg(feature = "llama-cpp")]
fn print_bench_stats(stats: &BenchStats, json: bool) {
    if json {
        println!("{}", serde_json::to_string(stats).expect("bench stats json"));
        return;
    }
    println!(
        "lattice-embed-host bench ({}, dims={})",
        stats.backend, stats.dimensions
    );
    println!(
        "  warmup={} iterations={} p50={:.1}ms p95={:.1}ms mean={:.1}ms min={:.1}ms max={:.1}ms",
        stats.warmup,
        stats.iterations,
        stats.p50_ms,
        stats.p95_ms,
        stats.mean_ms,
        stats.min_ms,
        stats.max_ms
    );
}

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use lattice_daemon::{
    default_socket_path, mcp, mcp_client_config, serve_with_shutdown_and_controllers, AgentController,
    AgentProviderMode, DaemonConfig, DaemonPreferences, SemanticController, SemanticProviderMode,
    VoiceController, VoiceProviderMode, DEFAULT_API_PORT,
};
use lattice_runtime::LatticeRuntime;
use serde_json;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(
    name = "latticed",
    version,
    about = "Lattice daemon: private IPC control plane + localhost API/MCP"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// IPC endpoint (Unix UDS path or Windows named-pipe name).
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

#[derive(Debug, Clone, Copy, ValueEnum)]
enum McpClientKind {
    #[value(name = "cursor")]
    Cursor,
    #[value(name = "claude-desktop")]
    ClaudeDesktop,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Serve MCP tools over stdio (search/read/related/build_context).
    Mcp {
        /// Shared auth token (informational / launcher parity with the HTTP API).
        #[arg(long, env = "LATTICE_AUTH_TOKEN")]
        auth_token: Option<String>,

        /// Print a client-ready `mcpServers` JSON block and exit (no stdio server).
        #[arg(long)]
        print_client_config: bool,

        /// MCP client target (`cursor` or `claude-desktop`). Required with
        /// `--print-client-config`.
        #[arg(long, requires = "print_client_config")]
        #[arg(required_if_eq("print_client_config", "true"))]
        client: Option<McpClientKind>,
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

    if let Some(Commands::Mcp {
        auth_token,
        print_client_config,
        client,
    }) = cli.command
    {
        if print_client_config {
            let _client = client.expect("clap requires --client with --print-client-config");
            let exe = std::env::current_exe().context("resolve current executable path")?;
            let command_path = exe.to_string_lossy();
            let env_token = std::env::var("LATTICE_AUTH_TOKEN").ok();
            let config = mcp_client_config::build_mcp_client_config(
                &command_path,
                env_token.as_deref(),
            );
            println!("{}", serde_json::to_string_pretty(&config)?);
            return Ok(());
        }

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

    // Expose auth + API base URL to supervised agentd (Lattice HTTP tools).
    // Never log token values.
    std::env::set_var("LATTICE_AUTH_TOKEN", &config.auth_token);
    if let Some(port) = config.api_port {
        std::env::set_var(
            "LATTICE_API_BASE_URL",
            format!("http://127.0.0.1:{port}"),
        );
    } else {
        std::env::remove_var("LATTICE_API_BASE_URL");
    }

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
        tracing::info!(
            "keep-services-running enabled; daemon will stay up after clients disconnect"
        );
    } else {
        tracing::info!(
            secs = idle_shutdown_timeout.as_secs(),
            "idle shutdown enabled after last client disconnects"
        );
    }

    let runtime = Arc::new(LatticeRuntime::new());
    // Always own a semantic controller so EnableSemanticSearch works without env
    // gates. Discovers/spawns lattice-embed-host by default; Fake only via
    // LATTICE_SEMANTIC_FAKE; Unavailable when the host binary cannot be found.
    let mode = SemanticProviderMode::from_env_or_default();
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
    let agent = match AgentProviderMode::from_env() {
        Some(mode) => {
            tracing::info!(?mode, "agent runtime enabled via environment");
            Some(
                AgentController::start(mode)
                    .await
                    .map_err(|err| anyhow::anyhow!("start agent controller: {err}"))?,
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
        serve_with_shutdown_and_controllers(config, runtime, semantic, voice, agent, rx)
            .await
            .context("latticed serve failed")?;
    }
    Ok(())
}

async fn wait_for_shutdown_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        let mut sigint =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
        tokio::select! {
            _ = sigterm.recv() => {}
            _ = sigint.recv() => {}
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        tokio::signal::ctrl_c().await
    }
}

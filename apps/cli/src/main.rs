use std::path::{Path, PathBuf};
use std::process::ExitCode;

use std::time::SystemTime;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use lattice_core::{Diagnostic, Resource, Severity, Workspace};
use lattice_storage::{NativeWorkspaceStore, RecoveryJournal, WorkspaceStore};

#[derive(Parser)]
#[command(
    name = "lattice",
    version,
    about = "Headless CLI for Lattice workspaces"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new workspace.
    Init {
        /// Directory to create the workspace in. Defaults to the current directory.
        path: Option<PathBuf>,
        /// Workspace title. Defaults to the directory name.
        #[arg(long)]
        title: Option<String>,
    },
    /// Show workspace details.
    Info {
        /// Path inside the workspace to discover from. Defaults to the current directory.
        path: Option<PathBuf>,
    },
    /// List workspace resources.
    Ls {
        /// Path inside the workspace to discover from. Defaults to the current directory.
        path: Option<PathBuf>,
        /// Emit results as a JSON array.
        #[arg(long)]
        json: bool,
    },
    /// Validate workspace structure.
    Validate {
        /// Path inside the workspace to discover from. Defaults to the current directory.
        path: Option<PathBuf>,
        /// Emit diagnostics as a JSON array.
        #[arg(long)]
        json: bool,
    },
    /// Inspect and replay the crash-recovery journal.
    Recover {
        #[command(subcommand)]
        command: RecoverCommand,
    },
}

#[derive(Subcommand)]
enum RecoverCommand {
    /// List pending (unmaterialized) journal entries.
    List {
        /// Path inside the workspace to discover from. Defaults to the current directory.
        path: Option<PathBuf>,
    },
    /// Materialize a pending entry's content to its path.
    Apply {
        /// Journal entry id (from `recover list`).
        id: i64,
        /// Path inside the workspace to discover from. Defaults to the current directory.
        path: Option<PathBuf>,
    },
    /// Drop a pending entry without materializing it.
    Discard {
        /// Journal entry id (from `recover list`).
        id: i64,
        /// Path inside the workspace to discover from. Defaults to the current directory.
        path: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:?}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<ExitCode> {
    match command {
        Command::Init { path, title } => cmd_init(path, title),
        Command::Info { path } => cmd_info(path),
        Command::Ls { path, json } => cmd_ls(path, json),
        Command::Validate { path, json } => cmd_validate(path, json),
        Command::Recover { command } => match command {
            RecoverCommand::List { path } => cmd_recover_list(path),
            RecoverCommand::Apply { id, path } => cmd_recover_apply(id, path),
            RecoverCommand::Discard { id, path } => cmd_recover_discard(id, path),
        },
    }
}

fn cwd_or(path: Option<PathBuf>) -> Result<PathBuf> {
    match path {
        Some(p) => Ok(p),
        None => std::env::current_dir().context("failed to determine current directory"),
    }
}

/// Resolve a directory name to use as a default workspace title: the final
/// path component of the absolute, lexically-normalized path.
fn default_title(path: &Path) -> Result<String> {
    let absolute = std::path::absolute(path)
        .with_context(|| format!("failed to resolve path {}", path.display()))?;
    let name = absolute
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| absolute.to_string_lossy().into_owned());
    Ok(name)
}

fn cmd_init(path: Option<PathBuf>, title: Option<String>) -> Result<ExitCode> {
    let root = cwd_or(path)?;
    let title = match title {
        Some(t) => t,
        None => default_title(&root)?,
    };
    let ws = Workspace::init(&root, title)?;
    println!("created workspace at {}", ws.root().display());
    println!("id: {}", ws.manifest().id);
    Ok(ExitCode::SUCCESS)
}

fn cmd_info(path: Option<PathBuf>) -> Result<ExitCode> {
    let start = cwd_or(path)?;
    let ws = Workspace::discover(&start)?;
    let manifest = ws.manifest();
    println!("root: {}", ws.root().display());
    println!("id: {}", manifest.id);
    println!("title: {}", manifest.title);
    println!("version: {}", manifest.version);
    if manifest.capabilities.enabled.is_empty() {
        println!("capabilities: (none)");
    } else {
        println!("capabilities: {}", manifest.capabilities.enabled.join(", "));
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_ls(path: Option<PathBuf>, json: bool) -> Result<ExitCode> {
    let start = cwd_or(path)?;
    let ws = Workspace::discover(&start)?;
    let resources = ws.scan()?;
    if json {
        print_json(&resources)?;
    } else {
        for resource in &resources {
            print_resource(resource);
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn print_resource(resource: &Resource) {
    println!(
        "{:<10} {}",
        format!("{:?}", resource.kind),
        resource.path.display()
    );
}

fn cmd_validate(path: Option<PathBuf>, json: bool) -> Result<ExitCode> {
    let start = cwd_or(path)?;
    let ws = Workspace::discover(&start)?;
    let diagnostics = ws.validate()?;
    let has_error = diagnostics.iter().any(|d| d.severity == Severity::Error);

    if json {
        print_json(&diagnostics)?;
    } else if diagnostics.is_empty() {
        println!("workspace is valid");
    } else {
        for diagnostic in &diagnostics {
            print_diagnostic(diagnostic);
        }
    }

    Ok(if has_error {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

fn print_diagnostic(diagnostic: &Diagnostic) {
    println!("{diagnostic}");
}

fn cmd_recover_list(path: Option<PathBuf>) -> Result<ExitCode> {
    let start = cwd_or(path)?;
    let ws = Workspace::discover(&start)?;
    let journal = RecoveryJournal::open(ws.root())?;
    let pending = journal.pending()?;
    if pending.is_empty() {
        println!("no pending recovery entries");
        return Ok(ExitCode::SUCCESS);
    }
    for entry in &pending {
        println!(
            "{:>4}  {:<10} {:>8}  {:<16} {}",
            entry.id,
            format_age(entry.created_at),
            format!("{} B", entry.content.len()),
            entry.session_id,
            entry.path.display(),
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_recover_apply(id: i64, path: Option<PathBuf>) -> Result<ExitCode> {
    let start = cwd_or(path)?;
    let ws = Workspace::discover(&start)?;
    let journal = RecoveryJournal::open(ws.root())?;
    let store = NativeWorkspaceStore::new(ws.root());

    let entry = journal
        .pending()?
        .into_iter()
        .find(|e| e.id == id)
        .with_context(|| format!("no pending recovery entry with id {id}"))?;

    // Warn (but proceed) if the base the edit was made against no longer
    // matches what is on disk: applying may overwrite an external change.
    let current = match store.metadata(&entry.path) {
        Ok(meta) => Some(meta.revision.hash),
        Err(_) => None,
    };
    if current.as_deref() != entry.base_revision.as_deref() {
        eprintln!(
            "warning: base revision mismatch for {} (expected {:?}, found {:?}); applying anyway",
            entry.path.display(),
            entry.base_revision,
            current,
        );
    }

    let revision = store.write_atomic(&entry.path, &entry.content)?;
    journal.discard(entry.id)?;
    println!(
        "applied entry {} to {} ({})",
        entry.id,
        entry.path.display(),
        revision.hash,
    );
    Ok(ExitCode::SUCCESS)
}

fn cmd_recover_discard(id: i64, path: Option<PathBuf>) -> Result<ExitCode> {
    let start = cwd_or(path)?;
    let ws = Workspace::discover(&start)?;
    let journal = RecoveryJournal::open(ws.root())?;
    if !journal.pending()?.iter().any(|e| e.id == id) {
        bail!("no pending recovery entry with id {id}");
    }
    journal.discard(id)?;
    println!("discarded entry {id}");
    Ok(ExitCode::SUCCESS)
}

/// Render the age of a journal entry as a coarse human-readable duration.
fn format_age(created_at: SystemTime) -> String {
    let secs = SystemTime::now()
        .duration_since(created_at)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    let text = serde_json::to_string_pretty(value)?;
    println!("{text}");
    Ok(())
}

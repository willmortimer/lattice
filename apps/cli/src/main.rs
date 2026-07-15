use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use lattice_core::{Diagnostic, Resource, Severity, Workspace};

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

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    let text = serde_json::to_string_pretty(value)?;
    println!("{text}");
    Ok(())
}

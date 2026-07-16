use std::path::{Path, PathBuf};
use std::process::ExitCode;

use std::time::SystemTime;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use lattice_commands::{Command as Semantic, CommandEngine, Transaction};
use lattice_core::{
    ensure_lattice_home, init_with_template, Diagnostic, Resource, Severity, Workspace,
    WorkspaceTemplate,
};
use lattice_data::{parse_csv_file, CellValue, DataApp};
use lattice_index::{Backlink, SearchHit, WorkspaceIndex};
use lattice_storage::{NativeWorkspaceStore, RecoveryJournal, WorkspaceStore};
use lattice_theme::{
    check_theme_file, discover_themes, load_appearance, save_appearance, AppearanceMode,
};

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
        /// Folder scaffolding: `personal` (default), `team`, `demo`, or `blank`.
        #[arg(long, default_value = "personal")]
        template: String,
    },
    /// Manage the Lattice home directory (`~/Lattice`).
    Home {
        #[command(subcommand)]
        command: HomeCommand,
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
    /// Create and update pages through the command engine.
    Page {
        #[command(subcommand)]
        command: PageCommand,
    },
    /// Create and inspect `.data` table packages.
    Table {
        #[command(subcommand)]
        command: TableCommand,
    },
    /// Insert, update, and delete rows in `.data` packages.
    Record {
        #[command(subcommand)]
        command: RecordCommand,
    },
    /// Rename a resource, or move it into an existing directory.
    Mv {
        /// Source path.
        from: PathBuf,
        /// Destination path; if it is an existing directory, the source is
        /// moved into it under its own name.
        to: PathBuf,
    },
    /// Delete a resource (sent to the OS Trash; undoable for files).
    Rm {
        /// Path to delete.
        path: PathBuf,
    },
    /// List applied transactions, newest first.
    History {
        /// Maximum number of transactions to show.
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Undo the most recent transaction.
    Undo,
    /// Redo the most recently undone transaction.
    Redo,
    /// Rebuild the workspace search index.
    Index {
        /// Path inside the workspace to discover from. Defaults to the current directory.
        path: Option<PathBuf>,
    },
    /// Full-text search over indexed pages.
    Search {
        /// Search query.
        query: String,
        /// Path inside the workspace to discover from. Defaults to the current directory.
        path: Option<PathBuf>,
        /// Maximum number of hits to return.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Emit hits as a JSON array.
        #[arg(long)]
        json: bool,
    },
    /// List resources that link to a page.
    Backlinks {
        /// Workspace path of the target page.
        target: PathBuf,
        /// Path inside the workspace to discover from. Defaults to the current directory.
        path: Option<PathBuf>,
        /// Emit backlinks as a JSON array.
        #[arg(long)]
        json: bool,
    },
    /// Themes and appearance settings.
    Theme {
        #[command(subcommand)]
        command: ThemeCommand,
    },
}

#[derive(Subcommand)]
enum HomeCommand {
    /// Create `~/Lattice/{Workspaces,Settings}` and seed `Workspaces/Personal` if empty.
    Ensure,
    /// Print the Lattice home paths.
    Path,
}

#[derive(Subcommand)]
enum ThemeCommand {
    /// List built-in and user themes.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Validate a theme YAML file (defaults to checking all discovered themes).
    Check {
        /// Path to a `.theme.yaml` file. When omitted, checks every discovered theme.
        path: Option<PathBuf>,
    },
    /// Set the fixed active theme id.
    Set {
        /// Theme id (e.g. `lattice-slate`).
        id: String,
    },
    /// Choose fixed theme vs system-follow pair.
    Mode {
        /// `fixed` or `auto`.
        mode: String,
        /// Dark theme id when mode is `auto`.
        #[arg(long)]
        dark: Option<String>,
        /// Light theme id when mode is `auto`.
        #[arg(long)]
        light: Option<String>,
    },
}

#[derive(Subcommand)]
enum PageCommand {
    /// Create a new page.
    Create {
        /// Workspace path of the page (e.g. Notes/Ideas.md).
        path: PathBuf,
        /// Page content. Defaults to a heading derived from the filename.
        #[arg(long, conflicts_with = "stdin")]
        content: Option<String>,
        /// Read the page content from standard input.
        #[arg(long)]
        stdin: bool,
    },
    /// Replace the content of an existing page (reads from standard input).
    Update {
        /// Workspace path of the page.
        path: PathBuf,
        /// Read the new content from standard input (required).
        #[arg(long)]
        stdin: bool,
        /// Base revision ("sha256:...") the edit is based on. Defaults to
        /// the current on-disk revision (convenient for scripting, but skips
        /// the lost-update protection).
        #[arg(long)]
        base: Option<String>,
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

#[derive(Subcommand)]
enum TableCommand {
    /// Create a new `.data` package.
    Create {
        /// Workspace path of the package (e.g. CRM.data).
        path: PathBuf,
        /// Human-readable package title.
        #[arg(long)]
        title: String,
        /// Default table name inside the package.
        #[arg(long)]
        table: String,
    },
    /// Show package metadata, columns, and sample rows.
    Show {
        /// Workspace path of the package.
        path: PathBuf,
        /// Emit output as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Import a CSV file into a new `.data` package.
    Import {
        /// CSV file to import (may be outside the workspace).
        #[arg(long)]
        csv: PathBuf,
        /// Package name (creates `{name}.data` at the workspace root).
        #[arg(long)]
        name: String,
        /// Human-readable package title. Defaults to `name`.
        #[arg(long)]
        title: Option<String>,
        /// Table name inside the package. Defaults to `records`.
        #[arg(long, default_value = "records")]
        table: String,
    },
    /// List and inspect saved grid views.
    View {
        #[command(subcommand)]
        command: TableViewCommand,
    },
}

#[derive(Subcommand)]
enum TableViewCommand {
    /// List saved views in a `.data` package.
    List {
        /// Workspace path of the package.
        path: PathBuf,
        /// Emit view names as a JSON array.
        #[arg(long)]
        json: bool,
    },
    /// Show one saved view definition.
    Show {
        /// Workspace path of the package.
        path: PathBuf,
        /// View name (without `.yaml`).
        #[arg(long)]
        name: String,
        /// Emit the view as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum RecordCommand {
    /// Insert a row.
    Insert {
        /// Workspace path of the `.data` package.
        path: PathBuf,
        /// Table name.
        #[arg(long)]
        table: String,
        /// Field values as a JSON object.
        #[arg(long, conflicts_with = "field")]
        json: Option<String>,
        /// Field value as `name=value` (repeatable).
        #[arg(long = "field")]
        fields: Vec<String>,
    },
    /// Update a row.
    Update {
        /// Workspace path of the `.data` package.
        path: PathBuf,
        /// Table name.
        #[arg(long)]
        table: String,
        /// Row id.
        #[arg(long)]
        id: String,
        /// Field values as a JSON object.
        #[arg(long, conflicts_with = "field")]
        json: Option<String>,
        /// Field value as `name=value` (repeatable).
        #[arg(long = "field")]
        fields: Vec<String>,
        /// Base package revision (`sha256:...`). Defaults to the current revision.
        #[arg(long)]
        base: Option<String>,
    },
    /// Delete a row.
    Delete {
        /// Workspace path of the `.data` package.
        path: PathBuf,
        /// Table name.
        #[arg(long)]
        table: String,
        /// Row id.
        #[arg(long)]
        id: String,
        /// Base package revision (`sha256:...`). Defaults to the current revision.
        #[arg(long)]
        base: Option<String>,
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
        Command::Init {
            path,
            title,
            template,
        } => cmd_init(path, title, template),
        Command::Home { command } => match command {
            HomeCommand::Ensure => cmd_home_ensure(),
            HomeCommand::Path => cmd_home_path(),
        },
        Command::Info { path } => cmd_info(path),
        Command::Ls { path, json } => cmd_ls(path, json),
        Command::Validate { path, json } => cmd_validate(path, json),
        Command::Recover { command } => match command {
            RecoverCommand::List { path } => cmd_recover_list(path),
            RecoverCommand::Apply { id, path } => cmd_recover_apply(id, path),
            RecoverCommand::Discard { id, path } => cmd_recover_discard(id, path),
        },
        Command::Page { command } => match command {
            PageCommand::Create {
                path,
                content,
                stdin,
            } => cmd_page_create(path, content, stdin),
            PageCommand::Update { path, stdin, base } => cmd_page_update(path, stdin, base),
        },
        Command::Table { command } => match command {
            TableCommand::Create { path, title, table } => cmd_table_create(path, title, table),
            TableCommand::Show { path, json } => cmd_table_show(path, json),
            TableCommand::Import {
                csv,
                name,
                title,
                table,
            } => cmd_table_import(csv, name, title, table),
            TableCommand::View { command } => match command {
                TableViewCommand::List { path, json } => cmd_table_view_list(path, json),
                TableViewCommand::Show { path, name, json } => {
                    cmd_table_view_show(path, name, json)
                }
            },
        },
        Command::Record { command } => match command {
            RecordCommand::Insert {
                path,
                table,
                json,
                fields,
            } => cmd_record_insert(path, table, json, fields),
            RecordCommand::Update {
                path,
                table,
                id,
                json,
                fields,
                base,
            } => cmd_record_update(path, table, id, json, fields, base),
            RecordCommand::Delete {
                path,
                table,
                id,
                base,
            } => cmd_record_delete(path, table, id, base),
        },
        Command::Mv { from, to } => cmd_mv(from, to),
        Command::Rm { path } => cmd_rm(path),
        Command::History { limit } => cmd_history(limit),
        Command::Undo => cmd_undo(),
        Command::Redo => cmd_redo(),
        Command::Index { path } => cmd_index(path),
        Command::Search {
            query,
            path,
            limit,
            json,
        } => cmd_search(query, path, limit, json),
        Command::Backlinks { target, path, json } => cmd_backlinks(target, path, json),
        Command::Theme { command } => match command {
            ThemeCommand::List { json } => cmd_theme_list(json),
            ThemeCommand::Check { path } => cmd_theme_check(path),
            ThemeCommand::Set { id } => cmd_theme_set(id),
            ThemeCommand::Mode { mode, dark, light } => cmd_theme_mode(mode, dark, light),
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

fn cmd_init(path: Option<PathBuf>, title: Option<String>, template: String) -> Result<ExitCode> {
    let root = cwd_or(path)?;
    let title = match title {
        Some(t) => t,
        None => default_title(&root)?,
    };
    let template = WorkspaceTemplate::parse(&template).with_context(|| {
        format!("unknown template {template:?}; expected personal, team, demo, or blank")
    })?;
    let ws = init_with_template(&root, title, template)?;
    println!("created workspace at {}", ws.root().display());
    println!("id: {}", ws.manifest().id);
    println!("template: {}", template.id());
    Ok(ExitCode::SUCCESS)
}

fn cmd_home_ensure() -> Result<ExitCode> {
    let home = ensure_lattice_home()?;
    println!("home: {}", home.root.display());
    println!("workspaces: {}", home.workspaces.display());
    println!("settings: {}", home.settings.display());
    let default = home.default_workspace();
    if default.join("lattice.yaml").exists() {
        println!("default workspace: {}", default.display());
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_home_path() -> Result<ExitCode> {
    let home = ensure_lattice_home()?;
    println!("{}", home.root.display());
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

/// Discover the enclosing workspace from the current directory and open the
/// command engine over it. All semantic mutations flow through this.
fn open_engine() -> Result<(Workspace, CommandEngine)> {
    let start = std::env::current_dir().context("failed to determine current directory")?;
    let ws = Workspace::discover(&start)?;
    let engine = CommandEngine::open(ws.root())?;
    Ok((ws, engine))
}

/// Resolve a user-supplied path (relative to the current directory, or
/// absolute) to a workspace-relative path.
fn workspace_relative(ws: &Workspace, path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path)
        .with_context(|| format!("failed to resolve path {}", path.display()))?;
    absolute
        .strip_prefix(ws.root())
        .map(Path::to_path_buf)
        .map_err(|_| {
            anyhow::anyhow!(
                "path {} is outside the workspace at {}",
                path.display(),
                ws.root().display()
            )
        })
}

fn read_stdin() -> Result<String> {
    use std::io::Read;
    let mut buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut buffer)
        .context("failed to read content from stdin")?;
    Ok(buffer)
}

/// Default content for a new page: a heading derived from the filename.
fn default_page_content(path: &Path) -> String {
    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".to_string());
    format!("# {title}\n")
}

fn cmd_page_create(path: PathBuf, content: Option<String>, stdin: bool) -> Result<ExitCode> {
    let (ws, mut engine) = open_engine()?;
    let rel = workspace_relative(&ws, &path)?;
    let content = if stdin {
        read_stdin()?
    } else {
        content.unwrap_or_else(|| default_page_content(&rel))
    };
    let receipt = engine.apply(Transaction::new(
        format!("Create page {}", rel.display()),
        vec![Semantic::PageCreate {
            path: rel.clone(),
            content,
        }],
    ))?;
    println!(
        "created {} ({})",
        rel.display(),
        receipt.outcomes[0]
            .resulting_revision
            .as_deref()
            .unwrap_or("?")
    );
    Ok(ExitCode::SUCCESS)
}

fn cmd_page_update(path: PathBuf, stdin: bool, base: Option<String>) -> Result<ExitCode> {
    if !stdin {
        bail!("page update reads the new content from stdin; pass --stdin");
    }
    let (ws, mut engine) = open_engine()?;
    let rel = workspace_relative(&ws, &path)?;
    let base_revision = match base {
        Some(base) => base,
        // Convenience: base on whatever is on disk right now. This skips the
        // lost-update protection a caller-supplied --base provides.
        None => {
            let store = NativeWorkspaceStore::new(ws.root());
            store.metadata(&rel)?.revision.hash
        }
    };
    let content = read_stdin()?;
    let receipt = engine.apply(Transaction::new(
        format!("Update page {}", rel.display()),
        vec![Semantic::PageUpdate {
            path: rel.clone(),
            content,
            base_revision,
        }],
    ))?;
    println!(
        "updated {} ({})",
        rel.display(),
        receipt.outcomes[0]
            .resulting_revision
            .as_deref()
            .unwrap_or("?")
    );
    Ok(ExitCode::SUCCESS)
}

fn cmd_table_create(path: PathBuf, title: String, table: String) -> Result<ExitCode> {
    let (ws, mut engine) = open_engine()?;
    let rel = workspace_relative(&ws, &path)?;
    let receipt = engine.apply(Transaction::new(
        format!("Create table package {}", rel.display()),
        vec![Semantic::TableCreate {
            path: rel.clone(),
            title,
            table_name: table,
        }],
    ))?;
    println!(
        "created {} ({})",
        rel.display(),
        receipt.outcomes[0]
            .resulting_revision
            .as_deref()
            .unwrap_or("?")
    );
    Ok(ExitCode::SUCCESS)
}

fn package_path_from_name(name: &str) -> PathBuf {
    let trimmed = name.trim().trim_end_matches(".data");
    PathBuf::from(format!("{trimmed}.data"))
}

fn cmd_table_import(
    csv: PathBuf,
    name: String,
    title: Option<String>,
    table: String,
) -> Result<ExitCode> {
    let parsed = parse_csv_file(&csv)?;
    let (ws, mut engine) = open_engine()?;
    let rel = workspace_relative(&ws, &package_path_from_name(&name))?;
    let title = title.unwrap_or_else(|| name.trim().replace(".data", "").to_string());

    engine.apply(Transaction::new(
        format!("Create table package {} from CSV", rel.display()),
        vec![Semantic::TableCreate {
            path: rel.clone(),
            title: title.clone(),
            table_name: table.clone(),
        }],
    ))?;

    let mut app = DataApp::open(&ws.root().join(&rel))?;
    app.add_columns_from_csv(&table, &parsed)?;

    for row in &parsed.rows {
        let mut values = std::collections::BTreeMap::new();
        for ((header, field_type), cell) in parsed
            .headers
            .iter()
            .zip(&parsed.field_types)
            .zip(row.iter())
        {
            values.insert(
                header.clone(),
                lattice_data::cell_from_csv(cell, *field_type)?,
            );
        }
        engine.apply(Transaction::new(
            format!("Import row into {}.{}", rel.display(), table),
            vec![Semantic::RecordInsert {
                path: rel.clone(),
                table: table.clone(),
                values,
                id: None,
            }],
        ))?;
    }

    println!(
        "imported {} row(s) into {} ({})",
        parsed.rows.len(),
        rel.display(),
        table
    );
    Ok(ExitCode::SUCCESS)
}

fn cmd_table_view_list(path: PathBuf, json: bool) -> Result<ExitCode> {
    let start = std::env::current_dir().context("failed to determine current directory")?;
    let ws = Workspace::discover(&start)?;
    let rel = workspace_relative(&ws, &path)?;
    let app = DataApp::open(&ws.root().join(&rel))?;
    let views = app.list_views()?;

    if json {
        print_json(&views)?;
    } else if views.is_empty() {
        println!("no views saved");
    } else {
        for name in views {
            println!("{name}");
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_table_view_show(path: PathBuf, name: String, json: bool) -> Result<ExitCode> {
    let start = std::env::current_dir().context("failed to determine current directory")?;
    let ws = Workspace::discover(&start)?;
    let rel = workspace_relative(&ws, &path)?;
    let app = DataApp::open(&ws.root().join(&rel))?;
    let view = app.load_view(&name)?;

    if json {
        print_json(&view)?;
    } else {
        println!("{}", app.render_view_yaml(&view)?);
    }
    Ok(ExitCode::SUCCESS)
}

#[derive(serde::Serialize)]
struct TableShowOutput {
    path: PathBuf,
    title: String,
    default_table: String,
    tables: Vec<String>,
    columns: Vec<TableColumnOutput>,
    rows: Vec<TableRowOutput>,
    package_revision: String,
}

#[derive(serde::Serialize)]
struct TableColumnOutput {
    name: String,
    field_type: String,
}

#[derive(serde::Serialize)]
struct TableRowOutput {
    id: String,
    values: std::collections::BTreeMap<String, CellValue>,
}

fn cmd_table_show(path: PathBuf, json: bool) -> Result<ExitCode> {
    let start = std::env::current_dir().context("failed to determine current directory")?;
    let ws = Workspace::discover(&start)?;
    let rel = workspace_relative(&ws, &path)?;
    let app = DataApp::open(&ws.root().join(&rel))?;
    let table = app.default_table().to_string();
    let columns = app
        .columns(&table)?
        .into_iter()
        .map(|column| TableColumnOutput {
            name: column.name,
            field_type: column.field_type.to_string(),
        })
        .collect::<Vec<_>>();
    let rows = app
        .list_rows(&table, 20, 0)?
        .into_iter()
        .map(|row| TableRowOutput {
            id: row.id,
            values: row.values,
        })
        .collect::<Vec<_>>();
    let output = TableShowOutput {
        path: rel.clone(),
        title: app.title().to_string(),
        default_table: table,
        tables: app.list_tables()?,
        columns,
        rows,
        package_revision: app.package_revision()?,
    };

    if json {
        print_json(&output)?;
    } else {
        println!("{}  {}", output.path.display(), output.title);
        println!("default table: {}", output.default_table);
        println!("revision: {}", output.package_revision);
        println!("columns:");
        for column in &output.columns {
            println!("  {} ({})", column.name, column.field_type);
        }
        if output.rows.is_empty() {
            println!("rows: (none)");
        } else {
            println!("rows:");
            for row in &output.rows {
                println!("  {}  {:?}", row.id, row.values);
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn parse_record_values(
    json: Option<String>,
    fields: Vec<String>,
) -> Result<std::collections::BTreeMap<String, CellValue>> {
    use std::collections::BTreeMap;

    if let Some(json_text) = json {
        let raw: BTreeMap<String, serde_json::Value> =
            serde_json::from_str(&json_text).context("failed to parse --json as an object")?;
        return raw
            .into_iter()
            .map(|(key, value)| Ok((key, json_to_cell(value)?)))
            .collect();
    }
    if fields.is_empty() {
        bail!("provide --json or one or more --field name=value arguments");
    }
    let mut values = BTreeMap::new();
    for field in fields {
        let (name, value) = field
            .split_once('=')
            .with_context(|| format!("invalid --field {field:?}; expected name=value"))?;
        values.insert(name.to_string(), CellValue::Text(value.to_string()));
    }
    Ok(values)
}

fn json_to_cell(value: serde_json::Value) -> Result<CellValue> {
    match value {
        serde_json::Value::Null => Ok(CellValue::Null),
        serde_json::Value::Bool(boolean) => Ok(CellValue::Boolean(boolean)),
        serde_json::Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                Ok(CellValue::Integer(integer))
            } else if let Some(decimal) = number.as_f64() {
                Ok(CellValue::Decimal(decimal))
            } else {
                bail!("unsupported JSON number {number}");
            }
        }
        serde_json::Value::String(text) => Ok(CellValue::Text(text)),
        serde_json::Value::Object(object) => {
            serde_json::from_value(serde_json::Value::Object(object))
                .context("failed to parse typed cell value object")
        }
        serde_json::Value::Array(_) => bail!("array cell values are not supported"),
    }
}

fn package_revision(ws: &Workspace, package: &Path) -> Result<String> {
    Ok(DataApp::open(&ws.root().join(package))?.package_revision()?)
}

fn cmd_record_insert(
    path: PathBuf,
    table: String,
    json: Option<String>,
    fields: Vec<String>,
) -> Result<ExitCode> {
    let (ws, mut engine) = open_engine()?;
    let rel = workspace_relative(&ws, &path)?;
    let values = parse_record_values(json, fields)?;
    let receipt = engine.apply(Transaction::new(
        format!("Insert row into {}.{}", rel.display(), table),
        vec![Semantic::RecordInsert {
            path: rel.clone(),
            table,
            values,
            id: None,
        }],
    ))?;
    println!(
        "inserted row into {} ({})",
        rel.display(),
        receipt.outcomes[0]
            .resulting_revision
            .as_deref()
            .unwrap_or("?")
    );
    Ok(ExitCode::SUCCESS)
}

fn cmd_record_update(
    path: PathBuf,
    table: String,
    id: String,
    json: Option<String>,
    fields: Vec<String>,
    base: Option<String>,
) -> Result<ExitCode> {
    let (ws, mut engine) = open_engine()?;
    let rel = workspace_relative(&ws, &path)?;
    let base_revision = match base {
        Some(base) => base,
        None => package_revision(&ws, &rel)?,
    };
    let values = parse_record_values(json, fields)?;
    let receipt = engine.apply(Transaction::new(
        format!("Update row {} in {}.{}", id, rel.display(), table),
        vec![Semantic::RecordUpdate {
            path: rel.clone(),
            table,
            id,
            values,
            base_revision,
        }],
    ))?;
    println!(
        "updated row in {} ({})",
        rel.display(),
        receipt.outcomes[0]
            .resulting_revision
            .as_deref()
            .unwrap_or("?")
    );
    Ok(ExitCode::SUCCESS)
}

fn cmd_record_delete(
    path: PathBuf,
    table: String,
    id: String,
    base: Option<String>,
) -> Result<ExitCode> {
    let (ws, mut engine) = open_engine()?;
    let rel = workspace_relative(&ws, &path)?;
    let base_revision = match base {
        Some(base) => base,
        None => package_revision(&ws, &rel)?,
    };
    let receipt = engine.apply(Transaction::new(
        format!("Delete row {} from {}.{}", id, rel.display(), table),
        vec![Semantic::RecordDelete {
            path: rel.clone(),
            table,
            id,
            base_revision,
        }],
    ))?;
    println!(
        "deleted row from {} ({})",
        rel.display(),
        receipt.outcomes[0]
            .resulting_revision
            .as_deref()
            .unwrap_or("?")
    );
    Ok(ExitCode::SUCCESS)
}

fn cmd_mv(from: PathBuf, to: PathBuf) -> Result<ExitCode> {
    let (ws, mut engine) = open_engine()?;
    let from_rel = workspace_relative(&ws, &from)?;
    let to_rel = workspace_relative(&ws, &to)?;

    // If the destination is an existing directory, move into it; otherwise
    // this is a rename.
    let to_is_dir = ws.root().join(&to_rel).is_dir();
    if to_is_dir {
        engine.apply(Transaction::new(
            format!("Move {} into {}", from_rel.display(), to_rel.display()),
            vec![Semantic::ResourceMove {
                from: from_rel.clone(),
                to_dir: to_rel.clone(),
            }],
        ))?;
        println!("moved {} into {}", from_rel.display(), to_rel.display());
    } else {
        engine.apply(Transaction::new(
            format!("Rename {} to {}", from_rel.display(), to_rel.display()),
            vec![Semantic::ResourceRename {
                from: from_rel.clone(),
                to: to_rel.clone(),
            }],
        ))?;
        println!("renamed {} to {}", from_rel.display(), to_rel.display());
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_rm(path: PathBuf) -> Result<ExitCode> {
    let (ws, mut engine) = open_engine()?;
    let rel = workspace_relative(&ws, &path)?;
    engine.apply(Transaction::new(
        format!("Delete {}", rel.display()),
        vec![Semantic::ResourceDelete { path: rel.clone() }],
    ))?;
    println!("deleted {} (sent to Trash)", rel.display());
    Ok(ExitCode::SUCCESS)
}

fn cmd_history(limit: usize) -> Result<ExitCode> {
    let (_ws, engine) = open_engine()?;
    let entries = engine.history(limit)?;
    if entries.is_empty() {
        println!("no transactions recorded");
        return Ok(ExitCode::SUCCESS);
    }
    for entry in &entries {
        let short_id: String = entry.id.chars().take(8).collect();
        println!(
            "{short_id}  {:<10} {}{}",
            format_age(entry.created_at),
            entry.summary,
            if entry.undone { "  (undone)" } else { "" },
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_undo() -> Result<ExitCode> {
    let (_ws, mut engine) = open_engine()?;
    match engine.undo()? {
        Some(report) => println!("undid: {}", report.summary),
        None => println!("nothing to undo"),
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_redo() -> Result<ExitCode> {
    let (_ws, mut engine) = open_engine()?;
    match engine.redo()? {
        Some(report) => println!("redid: {}", report.summary),
        None => println!("nothing to redo"),
    }
    Ok(ExitCode::SUCCESS)
}

fn open_index(path: Option<PathBuf>) -> Result<(Workspace, WorkspaceIndex)> {
    let start = cwd_or(path)?;
    let ws = Workspace::discover(&start)?;
    let index = WorkspaceIndex::open(ws.root())?;
    Ok((ws, index))
}

fn cmd_index(path: Option<PathBuf>) -> Result<ExitCode> {
    let (ws, index) = open_index(path)?;
    let stats = index.rebuild(ws.root())?;
    println!(
        "indexed {} page(s), removed {} stale entr{}",
        stats.pages_indexed,
        stats.pages_removed,
        if stats.pages_removed == 1 { "y" } else { "ies" }
    );
    Ok(ExitCode::SUCCESS)
}

fn cmd_search(query: String, path: Option<PathBuf>, limit: usize, json: bool) -> Result<ExitCode> {
    let (_ws, index) = open_index(path)?;
    let hits = index.search(&query, limit)?;
    if json {
        print_json(&hits)?;
    } else if hits.is_empty() {
        println!("no matches");
    } else {
        for hit in &hits {
            print_search_hit(hit);
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn print_search_hit(hit: &SearchHit) {
    match &hit.snippet {
        Some(snippet) => println!("{}  {}  {}", hit.path.display(), hit.title, snippet),
        None => println!("{}  {}", hit.path.display(), hit.title),
    }
}

fn cmd_backlinks(target: PathBuf, path: Option<PathBuf>, json: bool) -> Result<ExitCode> {
    let (ws, index) = open_index(path)?;
    let rel = workspace_relative(&ws, &target)?;
    let backlinks = index.backlinks(&rel)?;
    if json {
        print_json(&backlinks)?;
    } else if backlinks.is_empty() {
        println!("no backlinks to {}", rel.display());
    } else {
        for link in &backlinks {
            print_backlink(link);
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_theme_list(json: bool) -> Result<ExitCode> {
    let (home, settings) = load_appearance()?;
    let (themes, diagnostics) = discover_themes(&home)?;
    if json {
        print_json(&serde_json::json!({
            "settings": settings,
            "themes": themes,
            "diagnostics": diagnostics,
        }))?;
    } else {
        println!(
            "mode={} theme={} pair=dark:{} light:{}",
            match settings.mode {
                AppearanceMode::Fixed => "fixed",
                AppearanceMode::Auto => "auto",
            },
            settings.theme,
            settings.pair.dark,
            settings.pair.light
        );
        for theme in &themes {
            println!(
                "{}\t{}\t{}\t{}",
                theme.id, theme.name, theme.appearance, theme.path
            );
        }
        for diag in &diagnostics {
            eprintln!("error: {}: {}", diag.path, diag.message);
        }
    }
    Ok(if diagnostics.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn cmd_theme_check(path: Option<PathBuf>) -> Result<ExitCode> {
    if let Some(path) = path {
        match check_theme_file(&path) {
            Ok(doc) => {
                println!("ok {} ({})", doc.id, doc.name);
                Ok(ExitCode::SUCCESS)
            }
            Err(err) => {
                eprintln!("error: {err}");
                Ok(ExitCode::FAILURE)
            }
        }
    } else {
        let (home, _) = load_appearance()?;
        let (themes, diagnostics) = discover_themes(&home)?;
        let mut failed = !diagnostics.is_empty();
        for diag in &diagnostics {
            eprintln!("error: {}: {}", diag.path, diag.message);
        }
        for theme in &themes {
            if theme.path.starts_with("builtin:") {
                println!("ok {}", theme.id);
                continue;
            }
            match check_theme_file(Path::new(&theme.path)) {
                Ok(_) => println!("ok {}", theme.id),
                Err(err) => {
                    eprintln!("error: {err}");
                    failed = true;
                }
            }
        }
        Ok(if failed {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        })
    }
}

fn cmd_theme_set(id: String) -> Result<ExitCode> {
    let (home, mut settings) = load_appearance()?;
    let (themes, _) = discover_themes(&home)?;
    if !themes.iter().any(|t| t.id == id) {
        bail!("theme not found: {id}");
    }
    settings.mode = AppearanceMode::Fixed;
    settings.theme = id.clone();
    save_appearance(&settings)?;
    println!("theme set to {id}");
    Ok(ExitCode::SUCCESS)
}

fn cmd_theme_mode(mode: String, dark: Option<String>, light: Option<String>) -> Result<ExitCode> {
    let (_home, mut settings) = load_appearance()?;
    settings.mode = match mode.to_ascii_lowercase().as_str() {
        "auto" => AppearanceMode::Auto,
        "fixed" => AppearanceMode::Fixed,
        other => bail!("mode must be fixed|auto, got {other}"),
    };
    if let Some(dark) = dark {
        settings.pair.dark = dark;
    }
    if let Some(light) = light {
        settings.pair.light = light;
    }
    save_appearance(&settings)?;
    println!(
        "appearance mode={} (dark={} light={})",
        mode, settings.pair.dark, settings.pair.light
    );
    Ok(ExitCode::SUCCESS)
}

fn print_backlink(link: &Backlink) {
    let anchor = link
        .anchor
        .as_deref()
        .map(|a| format!("#{a}"))
        .unwrap_or_default();
    println!(
        "{}  {:?} -> {}{}",
        link.source_path.display(),
        link.kind,
        link.target,
        anchor
    );
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

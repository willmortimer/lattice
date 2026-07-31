//! One-shot celld dogfood: hydrate → run → collect → propose_resource.
//!
//! Invoked by `scripts/cell-firecracker-dogfood.sh --live` (and the Mac OCI
//! wrapper). Requires `CELLD_BASE_URL`, `LATTICE_API_BASE_URL`, and
//! `LATTICE_AUTH_TOKEN`.

use std::path::{Path, PathBuf};

use kernelfs::{normalize_guest_path, InputMount};
use lattice_agentd::cell_host::{run_cell_task_and_propose, CellProposalProvenance};
use lattice_agentd::kernelfs_export::{export_oci_roles_under_agent_share, OciKernelfsExportRequest};
use lattice_agentd::lattice_client::lattice_client_from_env;
use lattice_agentd::wasi_host::WorkspaceBinding;
use lattice_cell_client::{
    is_oci_execution_mode, require_celld_base_url, CelldClient, HttpCelldClient, HydrateFile,
    KernelFSHydrationPlan, ProjectionRunRequest, EXECUTION_MODE_OCI,
};
use serde_json::json;
use tempfile::TempDir;

const DEFAULT_CELL_ID: &str = "cell_dogfood";
const DEFAULT_PROJECTION_ID: &str = "proj_dogfood";
const DEFAULT_OUTPUT_TARGET: &str = "Reports";
const DEFAULT_HYDRATE: &str = "input/hello.txt";
const DEFAULT_ARGV: &[&str] = &[
    "/bin/sh",
    "-c",
    "cp \"$KERNELFS_INPUT/hello.txt\" \"$KERNELFS_OUTPUT/out.txt\"",
];

#[derive(Debug, Default)]
struct Cli {
    workspace: Option<PathBuf>,
    cell_id: String,
    projection_id: String,
    output_target: String,
    hydrate_paths: Vec<String>,
    argv: Vec<String>,
    execution_mode: String,
    oci_bundle_path: String,
    vz_runtime_dir: Option<PathBuf>,
    with_work: bool,
    allow_network: bool,
}

#[tokio::main]
async fn main() {
    if let Err(message) = run().await {
        eprintln!("cell-firecracker-dogfood: {message}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let cli = parse_cli(std::env::args().skip(1).collect())?;
    let lattice = lattice_client_from_env()
        .ok_or("LATTICE_API_BASE_URL and LATTICE_AUTH_TOKEN must be set for live dogfood")?;
    let celld_url = require_celld_base_url().map_err(|err| err.to_string())?;
    let celld = CelldClient::new(celld_url, HttpCelldClient);

    let (_temp_workspace, workspace_root) = resolve_workspace(&cli)?;
    seed_default_input(&workspace_root, &cli.hydrate_paths)?;

    let hydrate_files = load_hydrate_files(&workspace_root, &cli.hydrate_paths)?;
    let execution_mode = normalize_execution_mode(&cli.execution_mode)?;
    if is_oci_execution_mode(&execution_mode) && cli.oci_bundle_path.trim().is_empty() {
        return Err(
            "--oci-bundle-path is required when --execution-mode=oci (live Mac OCI dogfood)"
                .into(),
        );
    }

    let (_temp_roles, input_host, work_host, output_host) =
        resolve_role_host_dirs(&cli, &execution_mode, &workspace_root, &cli.hydrate_paths)?;
    let mut plan = KernelFSHydrationPlan::from_role_paths(input_host, work_host, output_host);
    if cli.allow_network {
        plan = plan.with_network_deny_all(false);
    }

    let request = ProjectionRunRequest {
        cell_id: cli.cell_id.clone(),
        projection_id: cli.projection_id.clone(),
        plan,
        hydrate_files,
        argv: cli.argv.clone(),
        task_id: cli.projection_id.clone(),
        execution_mode,
        oci_bundle_path: cli.oci_bundle_path.clone(),
        ..ProjectionRunRequest::default()
    };

    let role_input = request
        .plan
        .input
        .first()
        .map(|path| path.host_path.clone());
    let role_work = request.plan.work.as_ref().map(|path| path.host_path.clone());
    let role_output = request.plan.output.host_path.clone();

    let provenance = CellProposalProvenance {
        cell_id: cli.cell_id.clone(),
        projection_id: cli.projection_id.clone(),
        task_id: cli.projection_id.clone(),
        output_proposal_target: cli.output_target.clone(),
        hydration_inputs: lattice_agentd::cell_host::hydration_inputs_from_files(
            &request.hydrate_files,
            &std::collections::BTreeMap::new(),
        ),
    };

    let workspace = WorkspaceBinding::new(None, Some(workspace_root.clone()));
    let (run_result, proposals) = run_cell_task_and_propose(
        &celld,
        &lattice,
        &workspace,
        &request,
        &cli.output_target,
        &provenance,
    )
    .await
    .map_err(|err| err.to_string())?;

    if proposals.is_empty() {
        return Err("expected >=1 proposal from collected /output files, got 0".into());
    }

    let source_resource = format!("cell://{}/{}", cli.cell_id, cli.projection_id);
    let summary = json!({
        "cellId": cli.cell_id,
        "projectionId": cli.projection_id,
        "workspace": workspace_root,
        "celldBaseUrl": celld.base_url(),
        "exitCode": run_result.run.exit_code,
        "draftCount": proposals.len(),
        "sourceResource": source_resource,
        "proposals": proposals,
        "roleHostDirs": {
            "input": role_input,
            "work": role_work,
            "output": role_output,
        },
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&summary).map_err(|err| err.to_string())?
    );
    Ok(())
}

fn parse_cli(args: Vec<String>) -> Result<Cli, String> {
    let mut cli = Cli {
        cell_id: DEFAULT_CELL_ID.to_string(),
        projection_id: DEFAULT_PROJECTION_ID.to_string(),
        output_target: DEFAULT_OUTPUT_TARGET.to_string(),
        hydrate_paths: vec![DEFAULT_HYDRATE.to_string()],
        argv: DEFAULT_ARGV.iter().map(|s| (*s).to_string()).collect(),
        ..Cli::default()
    };

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--workspace" => {
                index += 1;
                let path = args
                    .get(index)
                    .ok_or("--workspace requires a path")?
                    .clone();
                cli.workspace = Some(PathBuf::from(path));
            }
            "--cell-id" => {
                index += 1;
                cli.cell_id = next_value(&args, &mut index, "--cell-id")?;
            }
            "--projection-id" => {
                index += 1;
                cli.projection_id = next_value(&args, &mut index, "--projection-id")?;
            }
            "--output-target" => {
                index += 1;
                cli.output_target = next_value(&args, &mut index, "--output-target")?;
            }
            "--hydrate" => {
                index += 1;
                let path = next_value(&args, &mut index, "--hydrate")?;
                if cli.hydrate_paths == vec![DEFAULT_HYDRATE] {
                    cli.hydrate_paths.clear();
                }
                cli.hydrate_paths.push(path);
            }
            "--execution-mode" => {
                index += 1;
                cli.execution_mode = next_value(&args, &mut index, "--execution-mode")?;
            }
            "--oci-bundle-path" => {
                index += 1;
                cli.oci_bundle_path = next_value(&args, &mut index, "--oci-bundle-path")?;
            }
            "--vz-runtime-dir" => {
                index += 1;
                let path = next_value(&args, &mut index, "--vz-runtime-dir")?;
                cli.vz_runtime_dir = Some(PathBuf::from(path));
            }
            "--with-work" => {
                cli.with_work = true;
            }
            "--allow-network" => {
                cli.allow_network = true;
            }
            "--" => {
                cli.argv = args[index + 1..].to_vec();
                if cli.argv.is_empty() {
                    return Err("guest argv after -- must be non-empty".into());
                }
                return Ok(cli);
            }
            flag => return Err(format!("unknown argument: {flag}")),
        }
        index += 1;
    }
    Ok(cli)
}

fn next_value(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    let value = args
        .get(*index)
        .ok_or_else(|| format!("{flag} requires a value"))?
        .clone();
    if value.is_empty() {
        return Err(format!("{flag} requires a non-empty value"));
    }
    Ok(value)
}

fn resolve_workspace(cli: &Cli) -> Result<(Option<TempDir>, String), String> {
    if let Some(path) = &cli.workspace {
        let abs = std::fs::canonicalize(path)
            .map_err(|err| format!("workspace {}: {err}", path.display()))?;
        return Ok((None, abs.to_string_lossy().into_owned()));
    }
    if let Ok(path) = std::env::var("CELL_DOGFOOD_WORKSPACE") {
        if !path.trim().is_empty() {
            let abs = std::fs::canonicalize(path.trim())
                .map_err(|err| format!("CELL_DOGFOOD_WORKSPACE: {err}"))?;
            return Ok((None, abs.to_string_lossy().into_owned()));
        }
    }
    let temp = tempfile::tempdir().map_err(|err| err.to_string())?;
    let root = temp.path().to_string_lossy().into_owned();
    Ok((Some(temp), root))
}

/// Resolve KernelFS role host dirs.
///
/// MicroVM: ephemeral temp tree. OCI (Mac ivisor): materialize + kernelfs export
/// under `{agent-share}/.kernelfs-runs/{run_id}/` with flat volume sources at
/// `{agent-share}/{input,work?,output}`.
fn resolve_role_host_dirs(
    cli: &Cli,
    execution_mode: &str,
    workspace_root: &str,
    hydrate_paths: &[String],
) -> Result<(Option<TempDir>, PathBuf, Option<PathBuf>, PathBuf), String> {
    if is_oci_execution_mode(execution_mode) {
        let runtime = resolve_vz_runtime_dir(cli)?;
        let input_mounts = input_mounts_from_hydrate_paths(workspace_root, hydrate_paths)?;
        let workspace_root_path = PathBuf::from(workspace_root);
        let run_id = oci_run_id_from_projection(&cli.projection_id);
        let exported = export_oci_roles_under_agent_share(&OciKernelfsExportRequest {
            vz_runtime_dir: runtime,
            cell_id: cli.cell_id.clone(),
            run_id,
            input_mounts,
            host_path_roots: vec![workspace_root_path],
            with_work: cli.with_work,
            include_secrets: false,
        })
        .map_err(|err| format!("kernelfs OCI export under agent-share: {err}"))?;
        return Ok((None, exported.input, exported.work, exported.output));
    }

    let plan_parent = tempfile::tempdir().map_err(|err| err.to_string())?;
    let input_host = plan_parent.path().join("input");
    let output_host = plan_parent.path().join("output");
    std::fs::create_dir_all(&input_host).map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&output_host).map_err(|err| err.to_string())?;
    let work_host = if cli.with_work {
        let work = plan_parent.path().join("work");
        std::fs::create_dir_all(&work).map_err(|err| err.to_string())?;
        Some(work)
    } else {
        None
    };
    Ok((Some(plan_parent), input_host, work_host, output_host))
}

fn resolve_vz_runtime_dir(cli: &Cli) -> Result<PathBuf, String> {
    if let Some(path) = &cli.vz_runtime_dir {
        return Ok(path.clone());
    }
    if let Ok(path) = std::env::var("CELL_VZ_RUNTIME_DIR") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    if let Ok(workspace) = std::env::var("CELL_OCI_IVISOR_WORKSPACE") {
        let trimmed = workspace.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed).join("vz-runtime"));
        }
    }
    Err(
        "Mac OCI dogfood requires --vz-runtime-dir, CELL_VZ_RUNTIME_DIR, or \
         CELL_OCI_IVISOR_WORKSPACE (agent-share under ivisor-worker-<id>/agent-share; \
         see Cell docs/mac-live-bind-demo.md)"
            .into(),
    )
}

fn seed_default_input(workspace_root: &str, hydrate_paths: &[String]) -> Result<(), String> {
    for rel in hydrate_paths {
        let host = Path::new(workspace_root).join(rel);
        if host.is_file() {
            continue;
        }
        if let Some(parent) = host.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        std::fs::write(&host, "hello from cell dogfood\n").map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn load_hydrate_files(workspace_root: &str, hydrate_paths: &[String]) -> Result<Vec<HydrateFile>, String> {
    let mut files = Vec::with_capacity(hydrate_paths.len());
    for rel in hydrate_paths {
        let host = Path::new(workspace_root).join(rel);
        let guest_name = host
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("hydrate path {rel:?} has no file name"))?;
        let content = std::fs::read_to_string(&host)
            .map_err(|err| format!("cannot read hydrate file {rel}: {err}"))?;
        files.push(HydrateFile::text(format!("input/{guest_name}"), content));
    }
    Ok(files)
}

/// Map workspace hydrate paths to KernelFS [`InputMount`] entries for materialize.
///
/// Host paths are absolute under `workspace_root`; guest paths use the hydrate
/// file basename (e.g. `input/hello.txt` → guest `hello.txt`).
fn input_mounts_from_hydrate_paths(
    workspace_root: &str,
    hydrate_paths: &[String],
) -> Result<Vec<InputMount>, String> {
    let mut mounts = Vec::with_capacity(hydrate_paths.len());
    for rel in hydrate_paths {
        let host = Path::new(workspace_root).join(rel);
        let guest_path = host
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("hydrate path {rel:?} has no file name"))?
            .to_string();
        normalize_guest_path(&guest_path).map_err(|err| err.to_string())?;
        mounts.push(InputMount {
            host_path: host,
            guest_path,
        });
    }
    Ok(mounts)
}

/// Derive a kernelfs run id from the dogfood projection id.
fn oci_run_id_from_projection(projection_id: &str) -> String {
    let trimmed = projection_id.trim();
    if is_valid_oci_run_id(trimmed) {
        return trimmed.to_string();
    }
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let collapsed = out.trim_matches('_');
    if collapsed.is_empty() {
        "dogfood_run".into()
    } else {
        collapsed.to_string()
    }
}

fn is_valid_oci_run_id(id: &str) -> bool {
    !id.is_empty()
        && !id.contains('/')
        && !id.contains('\\')
        && id != "."
        && id != ".."
        && id.chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn normalize_execution_mode(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("microvm") {
        return Ok(String::new());
    }
    if is_oci_execution_mode(trimmed) {
        return Ok(EXECUTION_MODE_OCI.to_string());
    }
    Err(format!(
        "unsupported --execution-mode {raw:?} (use oci or leave empty for microVM)"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hydrate_paths_map_to_input_mounts_by_basename() {
        let workspace = tempfile::tempdir().expect("workspace");
        let rel = "input/hello.txt";
        let host = workspace.path().join(rel);
        std::fs::create_dir_all(host.parent().expect("parent")).expect("mkdir");
        std::fs::write(&host, "hello").expect("write");

        let mounts = input_mounts_from_hydrate_paths(
            &workspace.path().to_string_lossy(),
            &[rel.into()],
        )
        .expect("mounts");

        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].guest_path, "hello.txt");
        assert_eq!(mounts[0].host_path, host);
    }

    #[test]
    fn oci_run_id_uses_projection_id_when_valid() {
        assert_eq!(oci_run_id_from_projection("proj_dogfood"), "proj_dogfood");
    }

    #[test]
    fn oci_run_id_sanitizes_invalid_projection_id() {
        assert_eq!(oci_run_id_from_projection("ns/proj#1"), "ns_proj_1");
        assert_eq!(oci_run_id_from_projection("///"), "dogfood_run");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn oci_export_volume_sources_flat_under_agent_share() {
        let vz = tempfile::tempdir().expect("vz runtime");
        let workspace = tempfile::tempdir().expect("workspace");
        let rel = "input/hello.txt";
        let host = workspace.path().join(rel);
        std::fs::create_dir_all(host.parent().expect("parent")).expect("mkdir");
        std::fs::write(&host, "hello from hydrate\n").expect("write");

        let run_id = "proj_dogfood";
        let exported = export_oci_roles_under_agent_share(&OciKernelfsExportRequest {
            vz_runtime_dir: vz.path().to_path_buf(),
            cell_id: "cell_dogfood".into(),
            run_id: run_id.into(),
            input_mounts: input_mounts_from_hydrate_paths(
                &workspace.path().to_string_lossy(),
                &[rel.into()],
            )
            .expect("mounts"),
            host_path_roots: vec![workspace.path().to_path_buf()],
            with_work: false,
            include_secrets: false,
        })
        .expect("export");

        assert_eq!(exported.export_root, exported.agent_share);
        assert_eq!(exported.input, exported.agent_share.join("input"));
        assert_eq!(exported.output, exported.agent_share.join("output"));
        assert!(!exported.agent_share.join(run_id).exists());
        assert!(exported.input.starts_with(&exported.agent_share));
        assert!(exported.output.starts_with(&exported.agent_share));
    }
}

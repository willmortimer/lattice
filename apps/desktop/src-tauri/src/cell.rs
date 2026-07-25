//! Cell VZ lab env forwarding when the desktop spawns `latticed`.
//!
//! Supervision lives in `latticed`; this module only forwards parent env vars so
//! `pnpm tauri:dev` / `run-with-dotenv` can enable the lab path without manual
//! shell exports.

const ENV_CELL_VZ: &str = "LATTICE_CELL_VZ";
const ENV_CELL_HOST_BIN: &str = "LATTICE_CELL_HOST_BIN";
const ENV_CELLD_BIN: &str = "LATTICE_CELLD_BIN";
const ENV_CELL_VZ_IMAGES_DIR: &str = "CELL_VZ_IMAGES_DIR";
const ENV_CELL_VZ_HELPER_SOCKET: &str = "CELL_VZ_HELPER_SOCKET";
const ENV_CELL_DATA_DIR: &str = "LATTICE_CELL_DATA_DIR";
const ENV_CELL_LISTEN: &str = "LATTICE_CELL_LISTEN";

fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn forward_env_var(extra_env: &mut Vec<(String, String)>, key: &str) {
    if let Ok(value) = std::env::var(key) {
        if !value.is_empty() {
            extra_env.push((key.to_string(), value));
        }
    }
}

/// Append Cell VZ env vars when `LATTICE_CELL_VZ` is set in the parent process.
pub fn append_cell_vz_spawn_env(extra_env: &mut Vec<(String, String)>) {
    if !env_truthy(ENV_CELL_VZ) {
        return;
    }
    extra_env.push((ENV_CELL_VZ.to_string(), "1".into()));
    for key in [
        ENV_CELL_HOST_BIN,
        ENV_CELLD_BIN,
        ENV_CELL_VZ_IMAGES_DIR,
        ENV_CELL_VZ_HELPER_SOCKET,
        ENV_CELL_DATA_DIR,
        ENV_CELL_LISTEN,
    ] {
        forward_env_var(extra_env, key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_noop_without_gate() {
        if std::env::var(ENV_CELL_VZ).is_ok() {
            return;
        }
        let mut env = Vec::new();
        append_cell_vz_spawn_env(&mut env);
        assert!(env.is_empty());
    }
}

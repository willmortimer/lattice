//! LatticeFS resource authority and materialization for the Inspect panel.

use latticefs_core::{resource_stat_or_register, ResourceStat};

#[tauri::command]
pub fn get_resource_stat(root: String, rel_path: String) -> Result<ResourceStat, String> {
    resource_stat_or_register(std::path::Path::new(&root), &rel_path).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use latticefs_core::AuthorityMode;

    #[test]
    fn get_resource_stat_registers_missing_local_file() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Inspect").unwrap();
        std::fs::write(dir.path().join("Notes.md"), "# Notes\n").unwrap();

        let stat = get_resource_stat(
            dir.path().to_string_lossy().into_owned(),
            "Notes.md".to_string(),
        )
        .unwrap();

        assert_eq!(stat.path, "Notes.md");
        assert_eq!(stat.authority, AuthorityMode::Local);
    }
}

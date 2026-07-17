use lattice_commands::Error as CommandError;

/// Prefix for stale-revision errors so callers can distinguish optimistic-lock
/// conflicts from generic failures without parsing prose.
pub const STALE_REVISION_PREFIX: &str = "STALE_REVISION:";

pub fn command_error_to_string(err: CommandError) -> String {
    match err {
        CommandError::StaleBaseRevision {
            path,
            expected,
            found,
        } => {
            format!(
                "{STALE_REVISION_PREFIX}{}|expected={expected}|found={found}",
                path.display()
            )
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_revision_error_is_prefixed() {
        let err = CommandError::StaleBaseRevision {
            path: "Notes.md".into(),
            expected: "sha256:aaa".into(),
            found: "sha256:bbb".into(),
        };
        let message = command_error_to_string(err);
        assert!(message.starts_with(STALE_REVISION_PREFIX));
        assert!(message.contains("Notes.md"));
    }
}

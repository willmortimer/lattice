use std::path::PathBuf;

/// Platform default interactive shell.
///
/// Resolution order:
/// - Windows: `powershell.exe`
/// - Unix: `$SHELL` when set and non-empty, else `/bin/zsh` on macOS or
///   `/bin/bash` elsewhere
pub fn default_shell() -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from("powershell.exe")
    }

    #[cfg(not(windows))]
    {
        if let Ok(shell) = std::env::var("SHELL") {
            if !shell.is_empty() {
                return PathBuf::from(shell);
            }
        }

        #[cfg(target_os = "macos")]
        {
            PathBuf::from("/bin/zsh")
        }

        #[cfg(not(target_os = "macos"))]
        {
            PathBuf::from("/bin/bash")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shell_is_non_empty() {
        let shell = default_shell();
        assert!(!shell.as_os_str().is_empty());
    }
}

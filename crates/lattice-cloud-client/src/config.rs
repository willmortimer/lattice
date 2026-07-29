//! Cloud API base URL resolution.

pub const DEFAULT_CLOUD_URL: &str = "https://cloud.lattice-notes.com";

/// Compile-time channel default (internal DMG); falls back to [`DEFAULT_CLOUD_URL`].
fn compiled_default_cloud_url() -> &'static str {
    option_env!("LATTICE_CLOUD_URL_DEFAULT").unwrap_or(DEFAULT_CLOUD_URL)
}

/// Resolved lattice-server origin without a trailing slash.
pub fn cloud_url() -> String {
    std::env::var("LATTICE_CLOUD_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| compiled_default_cloud_url().to_string())
        .trim_end_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cloud_url_when_unset() {
        let _guard = EnvGuard::unset("LATTICE_CLOUD_URL");
        assert_eq!(cloud_url(), compiled_default_cloud_url().to_string());
    }

    #[test]
    fn trims_trailing_slash() {
        let _guard = EnvGuard::set("LATTICE_CLOUD_URL", "https://example.com/");
        assert_eq!(cloud_url(), "https://example.com");
    }

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn unset(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            // SAFETY: tests run single-threaded; env is restored on drop.
            unsafe { std::env::remove_var(key) };
            Self { key, previous }
        }

        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            // SAFETY: tests run single-threaded; env is restored on drop.
            unsafe { std::env::set_var(key, value) };
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: tests run single-threaded; env is restored on drop.
            unsafe {
                if let Some(value) = &self.previous {
                    std::env::set_var(self.key, value);
                } else {
                    std::env::remove_var(self.key);
                }
            }
        }
    }
}

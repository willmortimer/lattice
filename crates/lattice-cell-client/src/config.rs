//! `CELLD_BASE_URL` resolution — fail closed when unset.

/// Env var for the celld Connect/HTTP origin (no trailing slash).
pub const CELLD_BASE_URL_ENV: &str = "CELLD_BASE_URL";

use crate::error::{CellClientError, Result};

/// Resolved celld origin without a trailing slash, or `None` when unset/blank.
pub fn celld_base_url() -> Option<String> {
    std::env::var(CELLD_BASE_URL_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_end_matches('/').to_string())
}

/// Require [`CELLD_BASE_URL_ENV`]; fail closed when missing.
pub fn require_celld_base_url() -> Result<String> {
    celld_base_url().ok_or(CellClientError::MissingBaseUrl)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_fails_when_unset() {
        let _guard = EnvGuard::unset(CELLD_BASE_URL_ENV);
        assert!(matches!(
            require_celld_base_url(),
            Err(CellClientError::MissingBaseUrl)
        ));
    }

    #[test]
    fn trims_trailing_slash() {
        let _guard = EnvGuard::set(CELLD_BASE_URL_ENV, "http://127.0.0.1:8080/");
        assert_eq!(
            require_celld_base_url().unwrap(),
            "http://127.0.0.1:8080"
        );
    }

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn unset(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            // SAFETY: tests restore env on drop; crate tests are single-threaded.
            unsafe { std::env::remove_var(key) };
            Self { key, previous }
        }

        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            // SAFETY: tests restore env on drop; crate tests are single-threaded.
            unsafe { std::env::set_var(key, value) };
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: tests restore env on drop; crate tests are single-threaded.
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

//! User preferences that influence daemon lifecycle.
//!
//! Reads `services.keepServicesRunning` from the Lattice profile desktop
//! settings (`Settings/desktop.yaml`). Environment variables override for tests.

use std::time::Duration;

use lattice_profile::{ensure_profile_layout, DesktopSettings, DESKTOP_SETTINGS_SPEC};

/// Environment override for [`DaemonPreferences::keep_services_running`].
pub const LATTICE_KEEP_SERVICES_RUNNING_ENV: &str = "LATTICE_KEEP_SERVICES_RUNNING";

/// Environment override for idle shutdown seconds (tests).
pub const LATTICE_IDLE_SHUTDOWN_SECS_ENV: &str = "LATTICE_IDLE_SHUTDOWN_SECS";

/// Default idle time after the last client disconnects before shutdown.
pub const DEFAULT_IDLE_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

/// Lifecycle preferences loaded from the user profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DaemonPreferences {
    pub keep_services_running: bool,
    pub idle_shutdown_timeout: Duration,
}

impl Default for DaemonPreferences {
    fn default() -> Self {
        Self {
            keep_services_running: false,
            idle_shutdown_timeout: DEFAULT_IDLE_SHUTDOWN_TIMEOUT,
        }
    }
}

impl DaemonPreferences {
    /// Load profile settings, applying environment overrides when set.
    pub fn load() -> Self {
        let mut prefs = Self::default();
        if let Ok(home) = ensure_profile_layout() {
            if let Ok(loaded) = home
                .settings_store()
                .load::<DesktopSettings>(DESKTOP_SETTINGS_SPEC)
            {
                prefs.keep_services_running = loaded.value.services.keep_services_running;
            }
        }
        prefs.apply_env_overrides();
        prefs
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(value) = std::env::var(LATTICE_KEEP_SERVICES_RUNNING_ENV) {
            self.keep_services_running = env_truthy(&value);
        }
        if let Ok(value) = std::env::var(LATTICE_IDLE_SHUTDOWN_SECS_ENV) {
            if let Ok(secs) = value.parse::<f64>() {
                self.idle_shutdown_timeout = Duration::from_secs_f64(secs.max(0.0));
            }
        }
    }
}

fn env_truthy(value: &str) -> bool {
    matches!(value, "1" | "true" | "yes" | "on")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn env_overrides_keep_services_running() {
        let _guard = env_lock();
        std::env::set_var(LATTICE_KEEP_SERVICES_RUNNING_ENV, "1");
        let mut prefs = DaemonPreferences::default();
        prefs.apply_env_overrides();
        assert!(prefs.keep_services_running);
        std::env::remove_var(LATTICE_KEEP_SERVICES_RUNNING_ENV);
    }

    #[test]
    fn env_overrides_idle_shutdown_secs() {
        let _guard = env_lock();
        std::env::set_var(LATTICE_IDLE_SHUTDOWN_SECS_ENV, "0.25");
        let mut prefs = DaemonPreferences::default();
        prefs.apply_env_overrides();
        assert_eq!(prefs.idle_shutdown_timeout, Duration::from_millis(250));
        std::env::remove_var(LATTICE_IDLE_SHUTDOWN_SECS_ENV);
    }
}

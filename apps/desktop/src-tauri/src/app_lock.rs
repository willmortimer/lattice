//! Session app lock: privacy overlay + privileged IPC gate.
//!
//! Not workspace encryption. See ADR 0049.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lattice_core::ensure_lattice_home;
use lattice_profile::{DesktopSettings, DESKTOP_SETTINGS_SPEC};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::presence::{
    presence_available, request_user_presence, PresenceError, PresenceReason,
};

pub const APP_LOCK_EVENT: &str = "lattice-app-lock";
pub const APP_LOCKED_ERROR: &str = "app-locked";

const DEFAULT_IDLE_LOCK_MINUTES: u32 = 5;
const MIN_IDLE_LOCK_MINUTES: u32 = 0;
const MAX_IDLE_LOCK_MINUTES: u32 = 120;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLockStatus {
    pub enabled: bool,
    pub locked: bool,
    pub idle_lock_minutes: u32,
    pub presence_available: bool,
    pub platform_supported: bool,
}

#[derive(Debug)]
struct AppLockInner {
    enabled: bool,
    locked: bool,
    idle_lock_minutes: u32,
    unfocused_since: Option<Instant>,
}

impl AppLockInner {
    fn from_settings(settings: &DesktopSettings) -> Self {
        let enabled = settings.privacy.app_lock_enabled;
        Self {
            enabled,
            // Start locked when the preference is on so content IPC requires unlock.
            locked: enabled,
            idle_lock_minutes: clamp_idle_minutes(settings.privacy.idle_lock_minutes),
            unfocused_since: None,
        }
    }

    fn status(&self) -> AppLockStatus {
        AppLockStatus {
            enabled: self.enabled,
            locked: self.locked,
            idle_lock_minutes: self.idle_lock_minutes,
            presence_available: presence_available(),
            platform_supported: cfg!(any(target_os = "macos", target_os = "windows")),
        }
    }
}

#[derive(Clone, Default)]
pub struct AppLockState {
    inner: Arc<Mutex<AppLockInner>>,
}

impl Default for AppLockInner {
    fn default() -> Self {
        Self {
            enabled: false,
            locked: false,
            idle_lock_minutes: DEFAULT_IDLE_LOCK_MINUTES,
            unfocused_since: None,
        }
    }
}

impl AppLockState {
    pub fn load_from_profile() -> Self {
        let inner = ensure_lattice_home()
            .ok()
            .and_then(|home| {
                home.settings_store()
                    .load::<DesktopSettings>(DESKTOP_SETTINGS_SPEC)
                    .ok()
                    .map(|loaded| AppLockInner::from_settings(&loaded.value))
            })
            .unwrap_or_default();
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    pub fn status(&self) -> AppLockStatus {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .status()
    }

    pub fn is_locked(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .locked
    }

    pub fn apply_policy(&self, enabled: bool, idle_lock_minutes: u32, lock_when_enabling: bool) {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let was_enabled = guard.enabled;
        guard.enabled = enabled;
        guard.idle_lock_minutes = clamp_idle_minutes(idle_lock_minutes);
        if !enabled {
            guard.locked = false;
            guard.unfocused_since = None;
        } else if lock_when_enabling && !was_enabled {
            guard.locked = true;
            guard.unfocused_since = None;
        }
    }

    pub fn lock_now(&self) -> bool {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !guard.enabled {
            return false;
        }
        if guard.locked {
            return true;
        }
        guard.locked = true;
        guard.unfocused_since = None;
        true
    }

    pub fn unlock_with_presence(&self) -> Result<(), PresenceError> {
        let enabled = {
            let guard = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !guard.enabled {
                return Ok(());
            }
            if !guard.locked {
                return Ok(());
            }
            true
        };
        debug_assert!(enabled);
        request_user_presence(PresenceReason::UnlockApp)?;
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.locked = false;
        guard.unfocused_since = None;
        Ok(())
    }

    pub fn note_focus(&self, focused: bool) -> Option<Duration> {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !guard.enabled || guard.locked {
            guard.unfocused_since = None;
            return None;
        }
        if focused {
            guard.unfocused_since = None;
            None
        } else {
            guard.unfocused_since = Some(Instant::now());
            let minutes = guard.idle_lock_minutes;
            if minutes == 0 {
                None
            } else {
                Some(Duration::from_secs(u64::from(minutes) * 60))
            }
        }
    }

    /// Returns true when an idle lock should fire (still unfocused long enough).
    pub fn idle_lock_due(&self) -> bool {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !guard.enabled || guard.locked || guard.idle_lock_minutes == 0 {
            return false;
        }
        let Some(since) = guard.unfocused_since else {
            return false;
        };
        let threshold = Duration::from_secs(u64::from(guard.idle_lock_minutes) * 60);
        if since.elapsed() >= threshold {
            guard.locked = true;
            guard.unfocused_since = None;
            true
        } else {
            false
        }
    }
}

fn clamp_idle_minutes(value: u32) -> u32 {
    value.clamp(MIN_IDLE_LOCK_MINUTES, MAX_IDLE_LOCK_MINUTES)
}

pub fn ensure_unlocked(state: &AppLockState) -> Result<(), String> {
    if state.is_locked() {
        Err(APP_LOCKED_ERROR.to_string())
    } else {
        Ok(())
    }
}

/// Commands that remain callable while the session is locked.
pub fn command_allowed_while_locked(command: &str) -> bool {
    matches!(
        command,
        "app_lock_status"
            | "app_lock_lock"
            | "app_lock_unlock"
            | "app_lock_enable"
            | "get_profile_snapshot"
            | "list_themes"
            | "get_resolved_theme"
            | "plugin:event|listen"
            | "plugin:event|unlisten"
            | "plugin:event|emit"
            | "plugin:app|version"
            | "plugin:app|name"
            | "plugin:app|tauri_version"
            | "plugin:app|identifier"
            | "plugin:window|show"
            | "plugin:window|hide"
            | "plugin:window|set_focus"
            | "plugin:window|is_focused"
            | "plugin:window|is_visible"
            | "plugin:window|outer_position"
            | "plugin:window|inner_size"
            | "plugin:window|scale_factor"
            | "plugin:webview|set_webview_focus"
    ) || command.starts_with("plugin:event|")
        || command.starts_with("plugin:app|")
        || command.starts_with("plugin:window|")
        || command.starts_with("plugin:webview|")
        || command.starts_with("plugin:path|")
}

pub fn emit_status<R: Runtime>(app: &AppHandle<R>, status: &AppLockStatus) {
    let _ = app.emit(APP_LOCK_EVENT, status);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit(APP_LOCK_EVENT, status);
    }
}

/// Wrap a Tauri invoke handler so privileged commands fail while locked.
pub fn gated_invoke_handler<R, F>(
    handler: F,
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static
where
    R: Runtime,
    F: Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static,
{
    move |invoke_message| {
        let command = invoke_message.message.command().to_string();
        if !command_allowed_while_locked(&command) {
            let webview = invoke_message.message.webview();
            let app = webview.app_handle();
            if let Some(state) = app.try_state::<AppLockState>() {
                if let Err(err) = ensure_unlocked(&state) {
                    invoke_message.resolver.reject(err);
                    return true;
                }
            }
        }
        handler(invoke_message)
    }
}

pub fn lock_and_emit(app: &AppHandle) {
    let Some(state) = app.try_state::<AppLockState>() else {
        return;
    };
    if state.lock_now() {
        emit_status(app, &state.status());
    }
}

pub fn schedule_idle_check(app: &AppHandle, delay: Duration) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        let Some(state) = app.try_state::<AppLockState>() else {
            return;
        };
        if state.idle_lock_due() {
            emit_status(&app, &state.status());
        }
    });
}

#[tauri::command]
pub fn app_lock_status(state: State<'_, AppLockState>) -> AppLockStatus {
    state.status()
}

#[tauri::command]
pub fn app_lock_lock(app: AppHandle, state: State<'_, AppLockState>) -> Result<AppLockStatus, String> {
    if !state.status().enabled {
        return Err("App lock is not enabled".into());
    }
    state.lock_now();
    let status = state.status();
    emit_status(&app, &status);
    Ok(status)
}

#[tauri::command]
pub fn app_lock_unlock(
    app: AppHandle,
    state: State<'_, AppLockState>,
) -> Result<AppLockStatus, String> {
    state
        .unlock_with_presence()
        .map_err(|err| format!("{}: {err}", err.code()))?;
    let status = state.status();
    emit_status(&app, &status);
    Ok(status)
}

/// Prove presence, enable app lock in settings, and lock the session.
#[tauri::command]
pub fn app_lock_enable(
    app: AppHandle,
    state: State<'_, AppLockState>,
    idle_lock_minutes: Option<u32>,
) -> Result<AppLockStatus, String> {
    if !cfg!(any(target_os = "macos", target_os = "windows")) {
        return Err(PresenceError::Unsupported.to_string());
    }
    if !presence_available() {
        return Err(PresenceError::NotAvailable.to_string());
    }
    request_user_presence(PresenceReason::EnableAppLock)
        .map_err(|err| format!("{}: {err}", err.code()))?;
    let minutes = clamp_idle_minutes(
        idle_lock_minutes.unwrap_or_else(|| state.status().idle_lock_minutes),
    );
    let home = ensure_lattice_home().map_err(|err| err.to_string())?;
    let mut loaded = home
        .settings_store()
        .load::<DesktopSettings>(DESKTOP_SETTINGS_SPEC)
        .map_err(|err| err.to_string())?;
    loaded.value.privacy.app_lock_enabled = true;
    loaded.value.privacy.idle_lock_minutes = minutes;
    home.settings_store()
        .save(
            DESKTOP_SETTINGS_SPEC,
            &loaded.value,
            loaded.revision.as_deref(),
        )
        .map_err(|err| err.to_string())?;
    state.apply_policy(true, minutes, true);
    let status = state.status();
    emit_status(&app, &status);
    Ok(status)
}

pub fn sync_policy_from_settings(state: &AppLockState, settings: &DesktopSettings) {
    let enabled = settings.privacy.app_lock_enabled;
    let minutes = settings.privacy.idle_lock_minutes;
    // Settings save is not an auth step; only clear lock when disabling or
    // refresh idle minutes. Enabling goes through `app_lock_enable` first.
    state.apply_policy(enabled, minutes, false);
}

#[cfg(target_os = "macos")]
pub fn install_sleep_lock_observer(app: &AppHandle) {
    sleep_observer::install(app);
}

#[cfg(not(target_os = "macos"))]
pub fn install_sleep_lock_observer(_app: &AppHandle) {}

#[cfg(target_os = "macos")]
mod sleep_observer {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2_app_kit::{
        NSWorkspace, NSWorkspaceScreensDidSleepNotification, NSWorkspaceWillSleepNotification,
    };
    use objc2_foundation::{NSNotification, NSOperationQueue};
    use tauri::{AppHandle, Manager};

    use super::{emit_status, AppLockState};

    pub(super) fn install(app: &AppHandle) {
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        let names = [
            unsafe { NSWorkspaceWillSleepNotification },
            unsafe { NSWorkspaceScreensDidSleepNotification },
        ];
        for name in names {
            let app = app.clone();
            let block =
                RcBlock::new(move |_notification: NonNull<NSNotification>| {
                    let Some(state) = app.try_state::<AppLockState>() else {
                        return;
                    };
                    if state.lock_now() {
                        emit_status(&app, &state.status());
                    }
                });
            let token = unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    None,
                    Some(&*NSOperationQueue::mainQueue()),
                    &block,
                )
            };
            // Retain observer for process lifetime.
            std::mem::forget(token);
            std::mem::forget(block);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_profile::PrivacySettings;

    fn settings(enabled: bool, idle: u32) -> DesktopSettings {
        let mut settings = DesktopSettings::default();
        settings.privacy = PrivacySettings {
            app_lock_enabled: enabled,
            idle_lock_minutes: idle,
            ..PrivacySettings::default()
        };
        settings
    }

    #[test]
    fn starts_locked_when_enabled() {
        let state = AppLockState {
            inner: Arc::new(Mutex::new(AppLockInner::from_settings(&settings(true, 5)))),
        };
        assert!(state.is_locked());
        assert!(state.status().enabled);
    }

    #[test]
    fn starts_unlocked_when_disabled() {
        let state = AppLockState {
            inner: Arc::new(Mutex::new(AppLockInner::from_settings(&settings(false, 5)))),
        };
        assert!(!state.is_locked());
    }

    #[test]
    fn ensure_unlocked_rejects_when_locked() {
        let state = AppLockState {
            inner: Arc::new(Mutex::new(AppLockInner::from_settings(&settings(true, 5)))),
        };
        assert_eq!(ensure_unlocked(&state).unwrap_err(), APP_LOCKED_ERROR);
    }

    #[test]
    fn allowlist_includes_unlock() {
        assert!(command_allowed_while_locked("app_lock_unlock"));
        assert!(command_allowed_while_locked("get_profile_snapshot"));
        assert!(!command_allowed_while_locked("read_file"));
        assert!(!command_allowed_while_locked("open_workspace"));
    }

    #[test]
    fn idle_zero_disables_timer_but_keeps_manual_lock() {
        let state = AppLockState {
            inner: Arc::new(Mutex::new(AppLockInner::from_settings(&settings(true, 0)))),
        };
        // Unlock for focus tracking without presence in unit tests.
        {
            let mut guard = state.inner.lock().unwrap();
            guard.locked = false;
        }
        assert!(state.note_focus(false).is_none());
        assert!(!state.idle_lock_due());
        assert!(state.lock_now());
        assert!(state.is_locked());
    }

    #[test]
    fn clamp_idle_minutes_bounds() {
        assert_eq!(clamp_idle_minutes(0), 0);
        assert_eq!(clamp_idle_minutes(5), 5);
        assert_eq!(clamp_idle_minutes(999), 120);
    }

    #[test]
    fn platform_supported_matches_presence_backends() {
        let state = AppLockState::default();
        let supported = state.status().platform_supported;
        assert_eq!(
            supported,
            cfg!(any(target_os = "macos", target_os = "windows"))
        );
    }
}

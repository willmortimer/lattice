//! BYO OpenAI keychain commands and spawn-time credential resolution.

use std::sync::OnceLock;

use lattice_cloud_client::{
    KeychainOpenAiKeyStore, MemoryOpenAiKeyStore, OpenAiKeyStore, OPENAI_KEY_ACCOUNT,
    OPENAI_KEY_SERVICE,
};
use lattice_profile::{
    ensure_profile_layout, AiMode, DesktopSettings, DESKTOP_SETTINGS_SPEC,
};

const ENV_OPENAI_API_KEY: &str = "OPENAI_API_KEY";

fn openai_key_store() -> &'static dyn OpenAiKeyStore {
    static KEYCHAIN: OnceLock<KeychainOpenAiKeyStore> = OnceLock::new();
    static MEMORY: OnceLock<MemoryOpenAiKeyStore> = OnceLock::new();
    static USE_MEMORY: OnceLock<bool> = OnceLock::new();

    let use_memory = *USE_MEMORY.get_or_init(|| {
        let store = KeychainOpenAiKeyStore::new();
        match store.set_key("probe") {
            Ok(()) => {
                let _ = store.clear_key();
                false
            }
            Err(_) => true,
        }
    });
    if use_memory {
        MEMORY.get_or_init(MemoryOpenAiKeyStore::new)
    } else {
        KEYCHAIN.get_or_init(KeychainOpenAiKeyStore::new)
    }
}

fn map_err(err: impl std::fmt::Display) -> String {
    err.to_string()
}

pub fn load_desktop_ai_settings() -> lattice_profile::AiSettings {
    ensure_profile_layout()
        .ok()
        .and_then(|home| {
            home.settings_store()
                .load::<DesktopSettings>(DESKTOP_SETTINGS_SPEC)
                .ok()
        })
        .map(|loaded| loaded.value.ai)
        .unwrap_or_default()
}

/// Resolve `OPENAI_API_KEY` for daemon/agent spawn when BYO mode is active.
///
/// Process env wins over keychain. Keychain is consulted only when
/// `ai.mode == byoOpenai`.
pub fn resolve_openai_api_key_for_spawn() -> Option<String> {
    if let Ok(value) = std::env::var(ENV_OPENAI_API_KEY) {
        if !value.is_empty() {
            return Some(value);
        }
    }
    let settings = load_desktop_ai_settings();
    if settings.mode != AiMode::ByoOpenai {
        return None;
    }
    openai_key_store()
        .load_key()
        .ok()
        .flatten()
        .filter(|value| !value.is_empty())
}

#[tauri::command]
pub fn set_openai_api_key(key: String) -> Result<(), String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("OpenAI API key cannot be empty".into());
    }
    openai_key_store().set_key(trimmed).map_err(map_err)
}

#[tauri::command]
pub fn clear_openai_api_key() -> Result<(), String> {
    openai_key_store().clear_key().map_err(map_err)
}

#[tauri::command]
pub fn has_openai_api_key() -> Result<bool, String> {
    openai_key_store().has_key().map_err(map_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_key_constants_match_keychain_layout() {
        assert_eq!(OPENAI_KEY_SERVICE, "lattice.ai.openai");
        assert_eq!(OPENAI_KEY_ACCOUNT, "api-key");
    }
}

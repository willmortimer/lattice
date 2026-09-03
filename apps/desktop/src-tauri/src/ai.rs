//! BYO OpenAI keychain commands and spawn-time credential resolution.

use std::sync::OnceLock;

use lattice_cloud_client::{
    cloud_ai_responses_base_url, KeychainOpenAiKeyStore, MemoryOpenAiKeyStore, OpenAiKeyStore,
    OPENAI_KEY_ACCOUNT, OPENAI_KEY_SERVICE,
};
use lattice_profile::{
    ensure_profile_layout, AiMode, AiSettings, DesktopSettings, DESKTOP_SETTINGS_SPEC,
};

const ENV_OPENAI_API_KEY: &str = "OPENAI_API_KEY";
const ENV_OPENAI_BASE_URL: &str = "OPENAI_BASE_URL";
const ENV_AGENT_PROVIDER: &str = "LATTICE_AGENT_PROVIDER";

/// Lattice paid spawn credentials for agentd's OpenAI provider path.
pub struct AccountAiSpawnCredentials {
    /// Cloud session bearer passed as `OPENAI_API_KEY` for Responses auth (never log).
    pub bearer_token: String,
    pub openai_base_url: String,
}

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

/// When desktop AI mode is BYO OpenAI, agent spawn must pin the OpenAI provider.
pub fn agent_provider_for_profile(settings: &AiSettings) -> Option<&'static str> {
    match settings.mode {
        AiMode::ByoOpenai => Some("openai"),
        AiMode::Local => Some("local"),
        AiMode::Account => None,
    }
}

/// Whether spawn should use the in-process fake backend instead of the sidecar.
pub fn should_use_fake_agent_backend(
    settings: &AiSettings,
    fake_env: bool,
    pioneer_key_set: bool,
    openai_key_set: bool,
) -> bool {
    if fake_env {
        return true;
    }
    if settings.mode == AiMode::ByoOpenai {
        // BYO must not silently fake when the keychain key is missing.
        return false;
    }
    if settings.mode == AiMode::Local {
        // On-device must not silently fake; missing endpoint fails in health/run.
        return false;
    }
    !pioneer_key_set && !openai_key_set
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

/// Resolve Lattice paid credentials when `ai.mode == account` and a cloud session exists.
///
/// agentd's OpenAI path reads `OPENAI_API_KEY` as the Bearer token and
/// `OPENAI_BASE_URL` as the Responses API origin (`{cloud}/v1/ai`).
pub fn resolve_account_ai_for_spawn() -> Option<AccountAiSpawnCredentials> {
    let settings = load_desktop_ai_settings();
    if settings.mode != AiMode::Account {
        return None;
    }
    let bearer = lattice_handlers::resolve_cloud_bearer_cmd().ok()?;
    if bearer.is_empty() {
        return None;
    }
    Some(AccountAiSpawnCredentials {
        bearer_token: bearer,
        openai_base_url: cloud_ai_responses_base_url(),
    })
}

/// Env pairs for Lattice paid agent spawn (values must not be logged).
pub fn account_ai_spawn_env(credentials: &AccountAiSpawnCredentials) -> Vec<(String, String)> {
    vec![
        (ENV_AGENT_PROVIDER.to_string(), "openai".into()),
        (
            ENV_OPENAI_BASE_URL.to_string(),
            credentials.openai_base_url.clone(),
        ),
        // Cloud session bearer for lattice-server `/v1/ai/responses` (not a user OpenAI key).
        (
            ENV_OPENAI_API_KEY.to_string(),
            credentials.bearer_token.clone(),
        ),
    ]
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

    #[test]
    fn agent_provider_for_profile_forces_openai_for_byo() {
        let mut settings = AiSettings::default();
        settings.mode = AiMode::ByoOpenai;
        assert_eq!(agent_provider_for_profile(&settings), Some("openai"));

        settings.mode = AiMode::Local;
        assert_eq!(agent_provider_for_profile(&settings), Some("local"));

        settings.mode = AiMode::Account;
        assert_eq!(agent_provider_for_profile(&settings), None);
    }

    #[test]
    fn should_use_fake_agent_backend_skips_fake_for_byo_without_key() {
        let mut settings = AiSettings::default();
        settings.mode = AiMode::ByoOpenai;
        assert!(!should_use_fake_agent_backend(
            &settings, false, false, false
        ));

        settings.mode = AiMode::Local;
        assert!(!should_use_fake_agent_backend(
            &settings, false, false, false
        ));
        assert!(!should_use_fake_agent_backend(
            &settings, false, false, true
        ));

        settings.mode = AiMode::Account;
        assert!(should_use_fake_agent_backend(
            &settings, false, false, false
        ));
        assert!(!should_use_fake_agent_backend(
            &settings, false, false, true
        ));
    }

    #[test]
    fn account_ai_spawn_env_sets_openai_provider_and_proxy_base() {
        let credentials = AccountAiSpawnCredentials {
            bearer_token: "cloud-session".into(),
            openai_base_url: "https://cloud.test/v1/ai".into(),
        };
        let env = account_ai_spawn_env(&credentials);
        assert!(env
            .iter()
            .any(|(key, value)| key == ENV_AGENT_PROVIDER && value == "openai"));
        assert!(env
            .iter()
            .any(|(key, value)| key == ENV_OPENAI_BASE_URL && value == "https://cloud.test/v1/ai"));
        assert!(env
            .iter()
            .any(|(key, value)| key == ENV_OPENAI_API_KEY && value == "cloud-session"));
    }
}

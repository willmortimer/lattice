//! macOS SecItem token storage in the Lattice App Group keychain access group.
//!
//! Items use the Data Protection keychain (`kSecUseDataProtectionKeychain`) so
//! `kSecAttrAccessGroup` is honored on macOS. Requires the desktop entitlements
//! `keychain-access-groups` entry for `PQNKMDU3U3.group.dev.lattice.shared`.

use security_framework::passwords::{
    delete_generic_password_options, generic_password, set_generic_password_options,
    PasswordOptions,
};
use security_framework_sys::base::errSecItemNotFound;

use super::{Error, KeychainTokenStore, Result, TokenMaterial, TokenStore};

/// Team-prefixed keychain access group (must match Entitlements.plist).
pub const LATTICE_KEYCHAIN_ACCESS_GROUP: &str = "PQNKMDU3U3.group.dev.lattice.shared";
/// App Group container id (Finder helpers / Quick Look share this).
pub const LATTICE_APP_GROUP: &str = "group.dev.lattice.shared";

/// SecItem-backed store scoped to the shared Lattice access group.
pub struct AppGroupSecItemTokenStore {
    service: String,
}

impl AppGroupSecItemTokenStore {
    pub fn with_service(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn options(&self, account: &str) -> PasswordOptions {
        let mut options = PasswordOptions::new_generic_password(&self.service, account);
        options.use_protected_keychain();
        options.set_access_group(LATTICE_KEYCHAIN_ACCESS_GROUP);
        options.set_label("Lattice");
        options.set_description("Lattice credential");
        options
    }
}

impl TokenStore for AppGroupSecItemTokenStore {
    fn set(&self, key: &str, material: &TokenMaterial) -> Result<()> {
        let payload = serde_json::to_vec(material)?;
        set_generic_password_options(&payload, self.options(key))
            .map_err(|err| Error::credentials(err.to_string()))
    }

    fn get(&self, key: &str) -> Result<Option<TokenMaterial>> {
        match generic_password(self.options(key)) {
            Ok(bytes) => {
                let material: TokenMaterial = serde_json::from_slice(&bytes)?;
                Ok(Some(material))
            }
            Err(err) if err.code() == errSecItemNotFound => Ok(None),
            Err(err) => Err(Error::credentials(err.to_string())),
        }
    }

    fn delete(&self, key: &str) -> Result<()> {
        match delete_generic_password_options(self.options(key)) {
            Ok(()) => Ok(()),
            Err(err) if err.code() == errSecItemNotFound => Ok(()),
            Err(err) => Err(Error::credentials(err.to_string())),
        }
    }
}

/// Prefer App Group SecItem; migrate from legacy `keyring` service/account items.
pub struct MigratingAppGroupTokenStore {
    app_group: AppGroupSecItemTokenStore,
    legacy: KeychainTokenStore,
}

impl MigratingAppGroupTokenStore {
    pub fn with_service(service: impl Into<String>) -> Self {
        let service = service.into();
        Self {
            app_group: AppGroupSecItemTokenStore::with_service(service.clone()),
            legacy: KeychainTokenStore::with_service(service),
        }
    }
}

impl TokenStore for MigratingAppGroupTokenStore {
    fn set(&self, key: &str, material: &TokenMaterial) -> Result<()> {
        match self.app_group.set(key, material) {
            Ok(()) => {
                let _ = self.legacy.delete(key);
                Ok(())
            }
            // Unsigned / CLI builds lack the access-group entitlement — fall back.
            Err(_) => self.legacy.set(key, material),
        }
    }

    fn get(&self, key: &str) -> Result<Option<TokenMaterial>> {
        match self.app_group.get(key) {
            Ok(Some(material)) => Ok(Some(material)),
            Ok(None) => {
                let legacy = self.legacy.get(key)?;
                if let Some(ref material) = legacy {
                    if self.app_group.set(key, material).is_ok() {
                        let _ = self.legacy.delete(key);
                    }
                }
                Ok(legacy)
            }
            Err(_) => self.legacy.get(key),
        }
    }

    fn delete(&self, key: &str) -> Result<()> {
        let app_group_result = self.app_group.delete(key);
        let legacy_result = self.legacy.delete(key);
        app_group_result.or(legacy_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_group_constant_matches_entitlements_team_prefix() {
        assert!(LATTICE_KEYCHAIN_ACCESS_GROUP.starts_with("PQNKMDU3U3."));
        assert!(LATTICE_KEYCHAIN_ACCESS_GROUP.ends_with(LATTICE_APP_GROUP));
    }
}

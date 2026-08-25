use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudUser {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AuthTokenResponse {
    pub token: String,
    pub user: CloudUser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiAccess {
    None,
    Allowlisted,
    Paid,
}

impl AiAccess {
    pub fn allows_ai(self) -> bool {
        matches!(self, Self::Allowlisted | Self::Paid)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntitlementsView {
    pub ai_access: AiAccess,
    pub ai_daily_request_budget: i64,
    pub ai_daily_requests_used: i64,
}

/// Account-level consent flags mirrored from `/v1/me` (ADR 0064 / 0073).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreferencesView {
    pub ai_audit_enabled: bool,
    pub anonymous_telemetry_enabled: bool,
}

impl Default for PreferencesView {
    fn default() -> Self {
        Self {
            ai_audit_enabled: true,
            anonymous_telemetry_enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MeResponse {
    pub user: CloudUser,
    #[serde(default)]
    pub entitlements: Option<EntitlementsView>,
    #[serde(default)]
    pub preferences: Option<PreferencesView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudSessionStatus {
    pub signed_in: bool,
    pub cloud_url: String,
    pub user: Option<CloudUser>,
    /// Present when `/v1/me` returned entitlements; omitted on older servers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entitlements: Option<EntitlementsView>,
    /// Present when `/v1/me` returned preferences; omitted on older servers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferences: Option<PreferencesView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Cloud workspace registry row (`POST/GET /v1/workspaces`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CloudWorkspaceRecord {
    pub id: String,
    pub owner_user_id: String,
    pub name: String,
    pub local_workspace_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BackupWrapKeyResponse {
    /// Account wrap key as 64 hex characters (32 bytes). Never log this value.
    pub wrap_key: String,
}

/// Opaque backup metadata returned by `PUT/GET /v1/workspaces/{id}/backups`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BackupMetadataResponse {
    pub id: String,
    pub workspace_id: String,
    pub device_id: Option<String>,
    pub object_key: String,
    pub size: i64,
    pub content_hash: String,
    pub created_at: i64,
}

/// Workspace sync-head row from `GET /v1/workspaces/{id}/sync-heads`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSyncHead {
    pub resource_id: String,
    pub content_hash: String,
    pub updated_at: i64,
}

impl CloudSessionStatus {
    pub fn signed_out(cloud_url: String) -> Self {
        Self {
            signed_in: false,
            cloud_url,
            user: None,
            entitlements: None,
            preferences: None,
            error: None,
        }
    }

    pub fn signed_in(cloud_url: String, user: CloudUser) -> Self {
        Self::signed_in_with_entitlements(cloud_url, user, None, None)
    }

    pub fn signed_in_with_entitlements(
        cloud_url: String,
        user: CloudUser,
        entitlements: Option<EntitlementsView>,
        preferences: Option<PreferencesView>,
    ) -> Self {
        Self {
            signed_in: true,
            cloud_url,
            user: Some(user),
            entitlements,
            preferences,
            error: None,
        }
    }

    /// Lattice paid AI is runnable when signed in and either entitlements are
    /// missing (legacy `/v1/me`) or `ai_access` allows AI.
    pub fn ai_entitled(&self) -> bool {
        if !self.signed_in {
            return false;
        }
        match self.entitlements.as_ref() {
            None => true,
            Some(view) => view.ai_access.allows_ai(),
        }
    }
}

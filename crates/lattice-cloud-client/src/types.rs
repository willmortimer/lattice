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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MeResponse {
    pub user: CloudUser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudSessionStatus {
    pub signed_in: bool,
    pub cloud_url: String,
    pub user: Option<CloudUser>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl CloudSessionStatus {
    pub fn signed_out(cloud_url: String) -> Self {
        Self {
            signed_in: false,
            cloud_url,
            user: None,
            error: None,
        }
    }

    pub fn signed_in(cloud_url: String, user: CloudUser) -> Self {
        Self {
            signed_in: true,
            cloud_url,
            user: Some(user),
            error: None,
        }
    }
}

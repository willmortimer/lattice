//! Shared OAuth callback ingest for deep links (`lattice://oauth/...`).

use lattice_connectors::oauth_ingest_callback_url;

pub fn oauth_ingest_callback(url: String) -> Result<(), String> {
    oauth_ingest_callback_url(&url).map_err(|err| err.to_string())
}

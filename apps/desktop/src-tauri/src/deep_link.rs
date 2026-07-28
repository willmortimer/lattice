//! Deep-link URL parsing for custom scheme and Universal Links.
//!
//! OAuth callbacks stay on `lattice://oauth/…`. Workspace open links use
//! `lattice://open?root=…&path=…` (and matching https hosts once AASA is live).

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenResourcePayload {
    pub root: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepLinkAction {
    OAuthCallback(String),
    OpenResource(OpenResourcePayload),
}

/// Classify a URL delivered by `tauri-plugin-deep-link` (custom scheme or https).
pub fn classify_deep_link(url: &str) -> Option<DeepLinkAction> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with("lattice://oauth/") {
        return Some(DeepLinkAction::OAuthCallback(trimmed.to_string()));
    }

    parse_open_resource(trimmed).map(DeepLinkAction::OpenResource)
}

fn parse_open_resource(url: &str) -> Option<OpenResourcePayload> {
    let parsed = url::Url::parse(url).ok()?;
    match parsed.scheme() {
        "lattice" => parse_lattice_open(&parsed),
        "https" => parse_https_open(&parsed),
        _ => None,
    }
}

fn parse_lattice_open(parsed: &url::Url) -> Option<OpenResourcePayload> {
    let host = parsed.host_str().unwrap_or("");
    // `lattice://open?…` → host "open"; `lattice:///open?…` → path "/open".
    if host == "open" || ((host.is_empty()) && matches!(parsed.path(), "/open" | "open")) {
        return query_open_payload(parsed);
    }
    if host == "resource" {
        let path = path_from_segments(parsed);
        if path.is_empty() {
            return None;
        }
        let root = query_value(parsed, "root")?;
        return Some(OpenResourcePayload { root, path });
    }
    None
}

fn parse_https_open(parsed: &url::Url) -> Option<OpenResourcePayload> {
    let host = parsed.host_str()?;
    if !matches!(
        host,
        "lattice-notes.com" | "www.lattice-notes.com" | "app.lattice-notes.com"
    ) {
        return None;
    }
    if parsed.path() == "/open" || parsed.path().starts_with("/open/") {
        return query_open_payload(parsed);
    }
    None
}

fn query_open_payload(parsed: &url::Url) -> Option<OpenResourcePayload> {
    let root = query_value(parsed, "root")?;
    let path = query_value(parsed, "path")?;
    Some(OpenResourcePayload { root, path })
}

fn query_value(parsed: &url::Url, key: &str) -> Option<String> {
    for (k, v) in parsed.query_pairs() {
        if k == key {
            let value = v.trim();
            if value.is_empty() {
                return None;
            }
            return Some(value.to_string());
        }
    }
    None
}

fn path_from_segments(parsed: &url::Url) -> String {
    parsed
        .path_segments()
        .map(|segments| segments.collect::<Vec<_>>().join("/"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_oauth_callback() {
        let url = "lattice://oauth/callback?code=abc&state=1";
        assert_eq!(
            classify_deep_link(url),
            Some(DeepLinkAction::OAuthCallback(url.into()))
        );
    }

    #[test]
    fn classifies_lattice_open_query() {
        let url = "lattice://open?root=/Users/me/ws&path=Notes/Hello.md";
        assert_eq!(
            classify_deep_link(url),
            Some(DeepLinkAction::OpenResource(OpenResourcePayload {
                root: "/Users/me/ws".into(),
                path: "Notes/Hello.md".into(),
            }))
        );
    }

    #[test]
    fn classifies_lattice_resource_host() {
        let url = "lattice://resource/Notes%2FHello.md?root=/tmp/ws";
        assert_eq!(
            classify_deep_link(url),
            Some(DeepLinkAction::OpenResource(OpenResourcePayload {
                root: "/tmp/ws".into(),
                path: "Notes/Hello.md".into(),
            }))
        );
    }

    #[test]
    fn classifies_https_open() {
        let url = "https://app.lattice-notes.com/open?root=/tmp/ws&path=a.md";
        assert_eq!(
            classify_deep_link(url),
            Some(DeepLinkAction::OpenResource(OpenResourcePayload {
                root: "/tmp/ws".into(),
                path: "a.md".into(),
            }))
        );
    }

    #[test]
    fn rejects_unknown() {
        assert_eq!(
            classify_deep_link("https://example.com/open?root=a&path=b"),
            None
        );
        assert_eq!(classify_deep_link("lattice://oauth-not"), None);
        assert_eq!(classify_deep_link(""), None);
    }
}

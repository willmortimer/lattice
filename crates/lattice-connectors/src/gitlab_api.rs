//! GitLab REST helpers for listing accessible projects.

use serde::Deserialize;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
pub struct GitLabProjectSummary {
    pub id: u64,
    pub path_with_namespace: String,
    pub owner: String,
    pub name: String,
    pub default_branch: String,
    pub private: bool,
    pub clone_url: String,
}

pub trait GitLabApiClient: Send + Sync {
    fn get_json(&self, url: &str, bearer: &str) -> Result<String>;
}

pub struct HttpGitLabApiClient;

impl GitLabApiClient for HttpGitLabApiClient {
    fn get_json(&self, url: &str, bearer: &str) -> Result<String> {
        let body = ureq::get(url)
            .set("Accept", "application/json")
            .set("Authorization", &format!("Bearer {bearer}"))
            .set("User-Agent", "lattice-connectors")
            .call()
            .map_err(|err| Error::http(err.to_string()))?
            .into_string()
            .map_err(|err| Error::http(err.to_string()))?;
        Ok(body)
    }
}

#[derive(Debug, Deserialize)]
struct ProjectJson {
    id: u64,
    path_with_namespace: String,
    name: String,
    #[serde(default)]
    visibility: String,
    #[serde(default)]
    default_branch: Option<String>,
    #[serde(default)]
    http_url_to_repo: Option<String>,
    #[serde(default)]
    namespace: Option<NamespaceJson>,
}

#[derive(Debug, Deserialize)]
struct NamespaceJson {
    #[serde(default)]
    full_path: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

fn to_summary(project: ProjectJson) -> GitLabProjectSummary {
    let (owner, name) = match project.path_with_namespace.rsplit_once('/') {
        Some((o, n)) => (o.to_string(), n.to_string()),
        None => (
            project
                .namespace
                .as_ref()
                .and_then(|n| n.full_path.clone().or_else(|| n.path.clone()))
                .unwrap_or_default(),
            project.name.clone(),
        ),
    };
    GitLabProjectSummary {
        id: project.id,
        path_with_namespace: project.path_with_namespace.clone(),
        owner,
        name: if name.is_empty() { project.name } else { name },
        default_branch: project.default_branch.unwrap_or_else(|| "main".into()),
        private: project.visibility != "public",
        clone_url: project
            .http_url_to_repo
            .unwrap_or_else(|| format!("https://gitlab.com/{}.git", project.path_with_namespace)),
    }
}

pub fn get_project(
    client: &dyn GitLabApiClient,
    access_token: &str,
    path_with_namespace: &str,
) -> Result<GitLabProjectSummary> {
    let encoded = path_with_namespace
        .split('/')
        .map(|p| {
            let mut out = String::new();
            for b in p.bytes() {
                match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                        out.push(b as char)
                    }
                    _ => out.push_str(&format!("%{b:02X}")),
                }
            }
            out
        })
        .collect::<Vec<_>>()
        .join("%2F");
    let url = format!("https://gitlab.com/api/v4/projects/{encoded}");
    let body = client.get_json(&url, access_token)?;
    let parsed: ProjectJson =
        serde_json::from_str(&body).map_err(|err| Error::http(format!("project get: {err}")))?;
    Ok(to_summary(parsed))
}

pub fn list_accessible_projects(
    client: &dyn GitLabApiClient,
    access_token: &str,
    max_pages: u32,
) -> Result<Vec<GitLabProjectSummary>> {
    let mut out = Vec::new();
    let pages = max_pages.max(1);
    for page in 1..=pages {
        let url = format!(
            "https://gitlab.com/api/v4/projects?membership=true&simple=false&per_page=100&page={page}&order_by=last_activity_at"
        );
        let body = client.get_json(&url, access_token)?;
        let page_projects: Vec<ProjectJson> = serde_json::from_str(&body)
            .map_err(|err| Error::http(format!("projects list: {err}")))?;
        if page_projects.is_empty() {
            break;
        }
        for project in page_projects {
            out.push(to_summary(project));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct ScriptedApi {
        body: Mutex<String>,
    }

    impl GitLabApiClient for ScriptedApi {
        fn get_json(&self, _url: &str, _bearer: &str) -> Result<String> {
            Ok(self.body.lock().unwrap().clone())
        }
    }

    #[test]
    fn parses_project_page() {
        let client = ScriptedApi {
            body: Mutex::new(
                r#"[{
                    "id": 1,
                    "path_with_namespace": "acme/widget",
                    "name": "widget",
                    "visibility": "private",
                    "default_branch": "main",
                    "http_url_to_repo": "https://gitlab.com/acme/widget.git",
                    "namespace": { "full_path": "acme", "path": "acme" }
                }]"#
                .into(),
            ),
        };
        let projects = list_accessible_projects(&client, "tok", 1).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path_with_namespace, "acme/widget");
        assert_eq!(projects[0].default_branch, "main");
        assert!(projects[0].private);
    }
}

//! GitHub REST helpers for listing accessible repositories.

use serde::Deserialize;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
pub struct GitHubRepoSummary {
    pub id: u64,
    pub full_name: String,
    pub owner: String,
    pub name: String,
    pub default_branch: String,
    pub private: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_id: Option<u64>,
    pub clone_url: String,
}

pub trait GitHubApiClient: Send + Sync {
    fn get_json(&self, url: &str, bearer: &str) -> Result<String>;
}

pub struct HttpGitHubApiClient;

impl GitHubApiClient for HttpGitHubApiClient {
    fn get_json(&self, url: &str, bearer: &str) -> Result<String> {
        let body = ureq::get(url)
            .set("Accept", "application/vnd.github+json")
            .set("Authorization", &format!("Bearer {bearer}"))
            .set("User-Agent", "lattice-connectors")
            .set("X-GitHub-Api-Version", "2022-11-28")
            .call()
            .map_err(|err| Error::http(err.to_string()))?
            .into_string()
            .map_err(|err| Error::http(err.to_string()))?;
        Ok(body)
    }
}

#[derive(Debug, Deserialize)]
struct RepoJson {
    id: u64,
    full_name: String,
    name: String,
    private: bool,
    default_branch: String,
    clone_url: String,
    owner: OwnerJson,
}

#[derive(Debug, Deserialize)]
struct OwnerJson {
    login: String,
}

/// Fetch one repository by owner/name.
pub fn get_repo(
    client: &dyn GitHubApiClient,
    access_token: &str,
    owner: &str,
    repo: &str,
) -> Result<GitHubRepoSummary> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}");
    let body = client.get_json(&url, access_token)?;
    let parsed: RepoJson =
        serde_json::from_str(&body).map_err(|err| Error::http(format!("repo get: {err}")))?;
    Ok(GitHubRepoSummary {
        id: parsed.id,
        full_name: parsed.full_name,
        owner: parsed.owner.login,
        name: parsed.name,
        default_branch: parsed.default_branch,
        private: parsed.private,
        installation_id: None,
        clone_url: parsed.clone_url,
    })
}

/// List repositories visible to the user access token (owned + collaborator).
///
/// Uses `/user/repos?affiliation=owner,collaborator` and pages until empty or
/// `max_pages` is reached.
pub fn list_accessible_repos(
    client: &dyn GitHubApiClient,
    access_token: &str,
    max_pages: u32,
) -> Result<Vec<GitHubRepoSummary>> {
    let mut out = Vec::new();
    let pages = max_pages.max(1);
    for page in 1..=pages {
        let url = format!(
            "https://api.github.com/user/repos?per_page=100&page={page}&affiliation=owner,collaborator&sort=updated"
        );
        let body = client.get_json(&url, access_token)?;
        let page_repos: Vec<RepoJson> = serde_json::from_str(&body)
            .map_err(|err| Error::http(format!("repos list: {err}")))?;
        if page_repos.is_empty() {
            break;
        }
        for repo in page_repos {
            out.push(GitHubRepoSummary {
                id: repo.id,
                full_name: repo.full_name,
                owner: repo.owner.login,
                name: repo.name,
                default_branch: repo.default_branch,
                private: repo.private,
                installation_id: None,
                clone_url: repo.clone_url,
            });
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

    impl GitHubApiClient for ScriptedApi {
        fn get_json(&self, _url: &str, _bearer: &str) -> Result<String> {
            Ok(self.body.lock().unwrap().clone())
        }
    }

    #[test]
    fn parses_repo_page() {
        let client = ScriptedApi {
            body: Mutex::new(
                r#"[{
                    "id": 1,
                    "full_name": "acme/widget",
                    "name": "widget",
                    "private": false,
                    "default_branch": "main",
                    "clone_url": "https://github.com/acme/widget.git",
                    "owner": { "login": "acme" }
                }]"#
                .into(),
            ),
        };
        let repos = list_accessible_repos(&client, "tok", 1).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].full_name, "acme/widget");
        assert_eq!(repos[0].default_branch, "main");
    }
}

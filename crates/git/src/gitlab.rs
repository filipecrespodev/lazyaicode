use std::time::Duration;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::error::GitCommandError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLabProject {
    pub id: u64,
    pub default_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRequestInfo {
    pub iid: u64,
    pub title: String,
    pub source_branch: String,
    pub target_branch: String,
    pub web_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLabUser {
    pub id: u64,
    pub username: String,
    pub name: String,
}

/// One entry of a project's activity feed (`GET /projects/:id/events`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLabEvent {
    pub id: u64,
    /// e.g. "pushed to", "pushed new", "deleted", "opened", "accepted", "commented on".
    pub action_name: String,
    pub created_at: String,
    pub author_name: String,
    pub author_username: String,
    pub author_avatar_url: Option<String>,
    /// "MergeRequest", "Issue", "Note"... (`None` for pushes).
    pub target_type: Option<String>,
    pub target_title: Option<String>,
    pub target_iid: Option<u64>,
    pub push: Option<GitLabPushData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct GitLabPushData {
    /// "pushed", "created" or "removed".
    pub action: String,
    /// "branch" or "tag".
    pub ref_type: String,
    #[serde(rename(deserialize = "ref"))]
    pub ref_name: Option<String>,
    pub commit_count: u64,
    pub commit_to: Option<String>,
    pub commit_title: Option<String>,
}

pub struct CreateMergeRequestParams {
    pub source_branch: String,
    pub target_branch: String,
    pub title: String,
    pub description: String,
    pub assignee_id: Option<u64>,
    pub reviewer_ids: Vec<u64>,
    pub remove_source_branch: bool,
    pub squash: bool,
}

// Raw shapes exactly as GitLab's API returns them (snake_case), kept separate
// from the camelCase types above that get serialized back to the frontend.
#[derive(Deserialize)]
struct RawProject {
    id: u64,
    default_branch: Option<String>,
}

#[derive(Deserialize)]
struct RawMergeRequest {
    iid: u64,
    title: String,
    source_branch: String,
    target_branch: String,
    web_url: String,
}

#[derive(Deserialize)]
struct RawUser {
    id: u64,
    username: String,
    name: String,
}

#[derive(Deserialize)]
struct RawEventAuthor {
    name: String,
    username: String,
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct RawEvent {
    id: u64,
    action_name: String,
    created_at: String,
    author: RawEventAuthor,
    target_type: Option<String>,
    target_title: Option<String>,
    target_iid: Option<u64>,
    push_data: Option<GitLabPushData>,
}

impl From<RawEvent> for GitLabEvent {
    fn from(r: RawEvent) -> Self {
        Self {
            id: r.id,
            action_name: r.action_name,
            created_at: r.created_at,
            author_name: r.author.name,
            author_username: r.author.username,
            author_avatar_url: r.author.avatar_url,
            target_type: r.target_type,
            target_title: r.target_title,
            target_iid: r.target_iid,
            push: r.push_data,
        }
    }
}

impl From<RawProject> for GitLabProject {
    fn from(r: RawProject) -> Self {
        Self { id: r.id, default_branch: r.default_branch }
    }
}

impl From<RawMergeRequest> for MergeRequestInfo {
    fn from(r: RawMergeRequest) -> Self {
        Self { iid: r.iid, title: r.title, source_branch: r.source_branch, target_branch: r.target_branch, web_url: r.web_url }
    }
}

impl From<RawUser> for GitLabUser {
    fn from(r: RawUser) -> Self {
        Self { id: r.id, username: r.username, name: r.name }
    }
}

fn client() -> Result<reqwest::Client, GitCommandError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| GitCommandError::Internal(format!("falha ao criar o cliente HTTP: {e}")))
}

/// GitLab accepts a URL-encoded `namespace/path` (as well as a numeric id) as
/// the `:id` path segment, so callers can pass `owner/repo` directly.
fn encode_project_path(path: &str) -> String {
    let mut out = String::new();
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn project_url(base_url: &str, project_path: &str, suffix: &str) -> String {
    format!("{}/api/v4/projects/{}{}", base_url.trim_end_matches('/'), encode_project_path(project_path), suffix)
}

async fn send<T: DeserializeOwned>(req: reqwest::RequestBuilder) -> Result<T, GitCommandError> {
    let res = req.send().await.map_err(|e| GitCommandError::Internal(format!("falha na requisição ao GitLab: {e}")))?;
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(GitCommandError::Internal(format!("erro da API do GitLab ({status}): {body}")));
    }
    res.json::<T>().await.map_err(|e| GitCommandError::Internal(format!("falha ao interpretar a resposta do GitLab: {e}")))
}

pub async fn get_project(base_url: &str, token: &str, project_path: &str) -> Result<GitLabProject, GitCommandError> {
    let url = project_url(base_url, project_path, "");
    let raw: RawProject = send(client()?.get(&url).header("PRIVATE-TOKEN", token)).await?;
    Ok(raw.into())
}

pub async fn list_open_merge_requests(base_url: &str, token: &str, project_path: &str) -> Result<Vec<MergeRequestInfo>, GitCommandError> {
    let url = project_url(base_url, project_path, "/merge_requests?state=opened&per_page=100");
    let raw: Vec<RawMergeRequest> = send(client()?.get(&url).header("PRIVATE-TOKEN", token)).await?;
    Ok(raw.into_iter().map(Into::into).collect())
}

pub async fn create_merge_request(
    base_url: &str,
    token: &str,
    project_path: &str,
    params: CreateMergeRequestParams,
) -> Result<MergeRequestInfo, GitCommandError> {
    let url = project_url(base_url, project_path, "/merge_requests");
    let body = serde_json::json!({
        "source_branch": params.source_branch,
        "target_branch": params.target_branch,
        "title": params.title,
        "description": params.description,
        "assignee_id": params.assignee_id,
        "reviewer_ids": params.reviewer_ids,
        "remove_source_branch": params.remove_source_branch,
        "squash": params.squash,
    });
    let raw: RawMergeRequest = send(client()?.post(&url).header("PRIVATE-TOKEN", token).json(&body)).await?;
    Ok(raw.into())
}

pub async fn search_project_members(base_url: &str, token: &str, project_path: &str) -> Result<Vec<GitLabUser>, GitCommandError> {
    let url = project_url(base_url, project_path, "/members/all?per_page=100");
    let raw: Vec<RawUser> = send(client()?.get(&url).header("PRIVATE-TOKEN", token)).await?;
    Ok(raw.into_iter().map(Into::into).collect())
}

pub async fn list_project_events(base_url: &str, token: &str, project_path: &str) -> Result<Vec<GitLabEvent>, GitCommandError> {
    let url = project_url(base_url, project_path, "/events?per_page=50");
    let raw: Vec<RawEvent> = send(client()?.get(&url).header("PRIVATE-TOKEN", token)).await?;
    Ok(raw.into_iter().map(Into::into).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_push_event_from_the_api_shape() {
        let json = r#"{
            "id": 7, "action_name": "pushed to", "created_at": "2026-10-09T12:00:00.000Z",
            "author": {"name": "Gilson Terra", "username": "gilson.terra", "avatar_url": null},
            "target_type": null, "target_title": null, "target_iid": null,
            "push_data": {"action": "pushed", "ref_type": "branch", "ref": "staging",
                          "commit_count": 1, "commit_to": "5da45280abc", "commit_title": "feat(home): x"}
        }"#;
        let event: GitLabEvent = serde_json::from_str::<RawEvent>(json).unwrap().into();
        let push = event.push.unwrap();
        assert_eq!(push.ref_name.as_deref(), Some("staging"));
        assert_eq!(push.ref_type, "branch");
        let out = serde_json::to_value(&push).unwrap();
        assert_eq!(out["refName"], "staging");
        assert_eq!(out["commitTitle"], "feat(home): x");
    }

    #[test]
    fn encodes_project_path_slashes_for_the_id_segment() {
        assert_eq!(encode_project_path("group/sub/repo"), "group%2Fsub%2Frepo");
        assert_eq!(encode_project_path("owner-name/repo_name.git"), "owner-name%2Frepo_name.git");
    }
}

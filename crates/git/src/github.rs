//! GitHub REST client (github.com and GitHub Enterprise Server). Results are
//! returned in the same wire shapes as `crate::gitlab`'s, so the frontend
//! handles pull requests and merge requests, members and activity alike.

use std::time::Duration;

use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;

use crate::error::GitCommandError;
use crate::gitlab::{GitLabEvent, GitLabPushData, GitLabUser, MergeRequestInfo};

pub struct CreatePullRequestParams {
    pub head: String,
    pub base: String,
    pub title: String,
    pub body: String,
    pub assignee: Option<String>,
    pub reviewers: Vec<String>,
}

#[derive(Deserialize)]
struct RawRef {
    #[serde(rename = "ref")]
    name: String,
}

#[derive(Deserialize)]
struct RawPullRequest {
    number: u64,
    title: String,
    head: RawRef,
    base: RawRef,
    html_url: String,
}

impl From<RawPullRequest> for MergeRequestInfo {
    fn from(r: RawPullRequest) -> Self {
        Self { iid: r.number, title: r.title, source_branch: r.head.name, target_branch: r.base.name, web_url: r.html_url }
    }
}

#[derive(Deserialize)]
struct RawUser {
    id: u64,
    login: String,
}

/// github.com serves its API from a separate host; Enterprise Server from
/// `/api/v3` on the instance itself.
fn api_base(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let is_dotcom = reqwest::Url::parse(base)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.eq_ignore_ascii_case("github.com") || h.eq_ignore_ascii_case("www.github.com")))
        .unwrap_or(false);
    if is_dotcom {
        "https://api.github.com".to_string()
    } else {
        format!("{base}/api/v3")
    }
}

fn repo_url(base_url: &str, repo_path: &str, suffix: &str) -> String {
    format!("{}/repos/{}{}", api_base(base_url), repo_path.trim_matches('/'), suffix)
}

/// Teto da resposta do "último release" (a de verdade tem poucos KB).
const RELEASE_MAX_BYTES: usize = 1 << 20;

/// O último release publicado (`GET …/releases/latest`, sem token):
/// `(tag, endereço da página)`.
pub async fn latest_release(api_url: &str) -> Result<(String, String), GitCommandError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| GitCommandError::Internal(format!("falha ao criar o cliente HTTP: {e}")))?;
    let mut resp = client
        .get(api_url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "LazyAICode")
        .send()
        .await
        .map_err(|e| GitCommandError::Internal(format!("não foi possível consultar as versões: {e}")))?;
    if !resp.status().is_success() {
        return Err(GitCommandError::Internal(format!("consulta de versões: HTTP {}", resp.status())));
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| GitCommandError::Internal(e.to_string()))? {
        if body.len() + chunk.len() > RELEASE_MAX_BYTES {
            return Err(GitCommandError::Internal("resposta de versões grande demais".into()));
        }
        body.extend_from_slice(&chunk);
    }
    let v: Value = serde_json::from_slice(&body).map_err(|e| GitCommandError::Internal(format!("resposta de versões inválida: {e}")))?;
    let tag = v["tag_name"]
        .as_str()
        .filter(|t| !t.is_empty() && t.len() <= 64)
        .ok_or_else(|| GitCommandError::Internal("release sem tag".into()))?;
    let url = v["html_url"].as_str().filter(|u| u.starts_with("https://") && u.len() <= 512).unwrap_or_default();
    Ok((tag.to_string(), url.to_string()))
}

fn request(method: reqwest::Method, url: &str, token: &str) -> Result<reqwest::RequestBuilder, GitCommandError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| GitCommandError::Internal(format!("falha ao criar o cliente HTTP: {e}")))?;
    Ok(client
        .request(method, url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        // GitHub rejects requests without a User-Agent.
        .header("User-Agent", "LazyAICode"))
}

async fn send<T: DeserializeOwned>(req: reqwest::RequestBuilder) -> Result<T, GitCommandError> {
    let res = req.send().await.map_err(|e| GitCommandError::Internal(format!("falha na requisição ao GitHub: {e}")))?;
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(GitCommandError::Internal(format!("erro da API do GitHub ({status}): {body}")));
    }
    res.json::<T>().await.map_err(|e| GitCommandError::Internal(format!("falha ao interpretar a resposta do GitHub: {e}")))
}

pub async fn list_open_pull_requests(base_url: &str, token: &str, repo_path: &str) -> Result<Vec<MergeRequestInfo>, GitCommandError> {
    let url = repo_url(base_url, repo_path, "/pulls?state=open&per_page=100");
    let raw: Vec<RawPullRequest> = send(request(reqwest::Method::GET, &url, token)?).await?;
    Ok(raw.into_iter().map(Into::into).collect())
}

/// Opens the pull request, then assigns and requests reviews — GitHub takes
/// those as separate calls. A failure there doesn't undo the created PR; it's
/// reported, since the PR itself exists by then.
pub async fn create_pull_request(
    base_url: &str,
    token: &str,
    repo_path: &str,
    params: CreatePullRequestParams,
) -> Result<MergeRequestInfo, GitCommandError> {
    let url = repo_url(base_url, repo_path, "/pulls");
    let body = serde_json::json!({
        "title": params.title,
        "head": params.head,
        "base": params.base,
        "body": params.body,
    });
    let pr: MergeRequestInfo = send::<RawPullRequest>(request(reqwest::Method::POST, &url, token)?.json(&body)).await?.into();

    if let Some(assignee) = params.assignee {
        let url = repo_url(base_url, repo_path, &format!("/issues/{}/assignees", pr.iid));
        send::<Value>(request(reqwest::Method::POST, &url, token)?.json(&serde_json::json!({ "assignees": [assignee] }))).await.map_err(
            |e| GitCommandError::Internal(format!("pull request #{} criado, mas a atribuição do responsável falhou: {e}", pr.iid)),
        )?;
    }
    if !params.reviewers.is_empty() {
        let url = repo_url(base_url, repo_path, &format!("/pulls/{}/requested_reviewers", pr.iid));
        send::<Value>(request(reqwest::Method::POST, &url, token)?.json(&serde_json::json!({ "reviewers": params.reviewers })))
            .await
            .map_err(|e| GitCommandError::Internal(format!("pull request #{} criado, mas o pedido de revisão falhou: {e}", pr.iid)))?;
    }
    Ok(pr)
}

/// Users who can be assigned to (and review) issues and PRs — readable with
/// any token that can see the repo, unlike `/collaborators`.
pub async fn list_assignees(base_url: &str, token: &str, repo_path: &str) -> Result<Vec<GitLabUser>, GitCommandError> {
    let url = repo_url(base_url, repo_path, "/assignees?per_page=100");
    let raw: Vec<RawUser> = send(request(reqwest::Method::GET, &url, token)?).await?;
    Ok(raw.into_iter().map(|u| GitLabUser { id: u.id, name: u.login.clone(), username: u.login }).collect())
}

pub async fn list_repo_events(base_url: &str, token: &str, repo_path: &str) -> Result<Vec<GitLabEvent>, GitCommandError> {
    let url = repo_url(base_url, repo_path, "/events?per_page=50");
    let raw: Vec<Value> = send(request(reqwest::Method::GET, &url, token)?).await?;
    Ok(raw.iter().filter_map(map_event).collect())
}

fn str_at<'a>(v: &'a Value, pointer: &str) -> Option<&'a str> {
    v.pointer(pointer).and_then(Value::as_str)
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").to_string()
}

/// Maps one `/repos/:o/:r/events` entry onto the GitLab event shape: pushes
/// and branch/tag creation/deletion become `push` data, PRs/issues/comments
/// become targets ("MergeRequest", "Issue", "Note").
fn map_event(e: &Value) -> Option<GitLabEvent> {
    let kind = str_at(e, "/type")?;
    let p = e.get("payload").unwrap_or(&Value::Null);
    let id = match e.get("id") {
        Some(Value::String(s)) => s.parse().ok()?,
        Some(v) => v.as_u64()?,
        None => return None,
    };
    let login = str_at(e, "/actor/login").unwrap_or("").to_string();

    let mut event = GitLabEvent {
        id,
        action_name: String::new(),
        created_at: str_at(e, "/created_at").unwrap_or("").to_string(),
        author_name: str_at(e, "/actor/display_login").unwrap_or(&login).to_string(),
        author_username: login,
        author_avatar_url: str_at(e, "/actor/avatar_url").map(str::to_string),
        target_type: None,
        target_title: None,
        target_iid: None,
        push: None,
    };
    let push = |action: &str, ref_type: &str, ref_name: Option<&str>| GitLabPushData {
        action: action.to_string(),
        ref_type: ref_type.to_string(),
        ref_name: ref_name.map(str::to_string),
        commit_count: 0,
        commit_to: None,
        commit_title: None,
    };

    match kind {
        "PushEvent" => {
            let full_ref = str_at(p, "/ref").unwrap_or("");
            let (ref_type, name) = match full_ref.strip_prefix("refs/tags/") {
                Some(tag) => ("tag", tag),
                None => ("branch", full_ref.strip_prefix("refs/heads/").unwrap_or(full_ref)),
            };
            let commits = p.get("commits").and_then(Value::as_array);
            let mut data = push("pushed", ref_type, Some(name));
            data.commit_count = p.get("size").and_then(Value::as_u64).or_else(|| commits.map(|c| c.len() as u64)).unwrap_or(0);
            data.commit_to = str_at(p, "/head").map(str::to_string);
            data.commit_title = commits.and_then(|c| c.last()).and_then(|c| str_at(c, "/message")).map(first_line);
            event.action_name = "pushed to".into();
            event.push = Some(data);
        }
        "CreateEvent" | "DeleteEvent" => {
            let ref_type = str_at(p, "/ref_type").unwrap_or("branch");
            if ref_type == "repository" {
                event.action_name = "created repository".into();
            } else {
                let created = kind == "CreateEvent";
                event.action_name = if created { "pushed new" } else { "deleted" }.into();
                event.push = Some(push(if created { "created" } else { "removed" }, ref_type, str_at(p, "/ref")));
            }
        }
        "PullRequestEvent" | "PullRequestReviewEvent" => {
            let action = str_at(p, "/action").unwrap_or("updated");
            let merged = p.pointer("/pull_request/merged").and_then(Value::as_bool) == Some(true);
            event.action_name = match (kind, action) {
                ("PullRequestReviewEvent", _) => "reviewed".into(),
                (_, "closed") if merged => "merged".into(),
                _ => action.to_string(),
            };
            event.target_type = Some("MergeRequest".into());
            event.target_iid = p.pointer("/pull_request/number").and_then(Value::as_u64);
            event.target_title = str_at(p, "/pull_request/title").map(str::to_string);
        }
        "IssuesEvent" => {
            event.action_name = str_at(p, "/action").unwrap_or("updated").to_string();
            event.target_type = Some("Issue".into());
            event.target_iid = p.pointer("/issue/number").and_then(Value::as_u64);
            event.target_title = str_at(p, "/issue/title").map(str::to_string);
        }
        "IssueCommentEvent" | "PullRequestReviewCommentEvent" | "CommitCommentEvent" => {
            event.action_name = "commented on".into();
            event.target_type = Some("Note".into());
            event.target_title = str_at(p, "/issue/title").or_else(|| str_at(p, "/pull_request/title")).map(str::to_string);
        }
        "ReleaseEvent" => {
            event.action_name = "published release".into();
            event.target_title =
                str_at(p, "/release/name").filter(|s| !s.is_empty()).or_else(|| str_at(p, "/release/tag_name")).map(str::to_string);
        }
        "WatchEvent" => event.action_name = "starred".into(),
        "ForkEvent" => event.action_name = "forked".into(),
        "MemberEvent" => event.action_name = "added a member".into(),
        other => event.action_name = other.trim_end_matches("Event").to_lowercase(),
    }
    Some(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_api_github_com_for_dotcom_and_api_v3_for_enterprise() {
        assert_eq!(api_base("https://github.com"), "https://api.github.com");
        assert_eq!(api_base("https://github.com/"), "https://api.github.com");
        assert_eq!(api_base("https://ghe.corp.com/"), "https://ghe.corp.com/api/v3");
        assert_eq!(repo_url("https://github.com", "owner/repo", "/pulls"), "https://api.github.com/repos/owner/repo/pulls");
    }

    #[test]
    fn maps_a_push_event() {
        let e: Value = serde_json::from_str(
            r#"{"id":"42","type":"PushEvent","created_at":"2026-10-09T12:00:00Z",
                "actor":{"login":"gilson","display_login":"gilson","avatar_url":"https://a/x.png"},
                "payload":{"ref":"refs/heads/staging","head":"5da45280abc","size":2,
                           "commits":[{"message":"first"},{"message":"feat(home): x\n\nbody"}]}}"#,
        )
        .unwrap();
        let ev = map_event(&e).unwrap();
        assert_eq!(ev.id, 42);
        assert_eq!(ev.author_username, "gilson");
        let push = ev.push.unwrap();
        assert_eq!(push.action, "pushed");
        assert_eq!(push.ref_type, "branch");
        assert_eq!(push.ref_name.as_deref(), Some("staging"));
        assert_eq!(push.commit_count, 2);
        assert_eq!(push.commit_title.as_deref(), Some("feat(home): x"));
    }

    #[test]
    fn maps_tag_creation_branch_deletion_and_merged_prs() {
        let create: Value =
            serde_json::from_str(r#"{"id":"1","type":"CreateEvent","actor":{"login":"a"},"payload":{"ref_type":"tag","ref":"v1.0"}}"#)
                .unwrap();
        let push = map_event(&create).unwrap().push.unwrap();
        assert_eq!((push.action.as_str(), push.ref_type.as_str()), ("created", "tag"));

        let delete: Value =
            serde_json::from_str(r#"{"id":"2","type":"DeleteEvent","actor":{"login":"a"},"payload":{"ref_type":"branch","ref":"old"}}"#)
                .unwrap();
        assert_eq!(map_event(&delete).unwrap().push.unwrap().action, "removed");

        let pr: Value = serde_json::from_str(
            r#"{"id":"3","type":"PullRequestEvent","actor":{"login":"a"},
                "payload":{"action":"closed","pull_request":{"number":7,"title":"Add X","merged":true}}}"#,
        )
        .unwrap();
        let ev = map_event(&pr).unwrap();
        assert_eq!(ev.action_name, "merged");
        assert_eq!(ev.target_type.as_deref(), Some("MergeRequest"));
        assert_eq!(ev.target_iid, Some(7));
    }
}

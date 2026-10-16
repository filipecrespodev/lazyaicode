use crate::git_client::error::GitCommandError;
use crate::git_client::github::{self, CreatePullRequestParams};
use crate::git_client::gitlab::{GitLabEvent, GitLabUser, MergeRequestInfo};

#[tauri::command]
pub async fn github_list_open_pull_requests(
    base_url: String,
    token: String,
    project_path: String,
) -> Result<Vec<MergeRequestInfo>, GitCommandError> {
    github::list_open_pull_requests(&base_url, &token, &project_path).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn github_create_pull_request(
    base_url: String,
    token: String,
    project_path: String,
    source_branch: String,
    target_branch: String,
    title: String,
    description: String,
    assignee: Option<String>,
    reviewers: Vec<String>,
) -> Result<MergeRequestInfo, GitCommandError> {
    github::create_pull_request(
        &base_url,
        &token,
        &project_path,
        CreatePullRequestParams {
            head: source_branch,
            base: target_branch,
            title,
            body: description,
            assignee,
            reviewers,
        },
    )
    .await
}

#[tauri::command]
pub async fn github_list_assignees(
    base_url: String,
    token: String,
    project_path: String,
) -> Result<Vec<GitLabUser>, GitCommandError> {
    github::list_assignees(&base_url, &token, &project_path).await
}

#[tauri::command]
pub async fn github_list_repo_events(
    base_url: String,
    token: String,
    project_path: String,
) -> Result<Vec<GitLabEvent>, GitCommandError> {
    github::list_repo_events(&base_url, &token, &project_path).await
}

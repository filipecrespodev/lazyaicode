use crate::git_client::error::GitCommandError;
use crate::git_client::gitlab::{
    self, CreateMergeRequestParams, GitLabEvent, GitLabProject, GitLabUser, MergeRequestInfo,
};

#[tauri::command]
pub async fn gitlab_get_project(
    base_url: String,
    token: String,
    project_path: String,
) -> Result<GitLabProject, GitCommandError> {
    gitlab::get_project(&base_url, &token, &project_path).await
}

#[tauri::command]
pub async fn gitlab_list_open_merge_requests(
    base_url: String,
    token: String,
    project_path: String,
) -> Result<Vec<MergeRequestInfo>, GitCommandError> {
    gitlab::list_open_merge_requests(&base_url, &token, &project_path).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn gitlab_create_merge_request(
    base_url: String,
    token: String,
    project_path: String,
    source_branch: String,
    target_branch: String,
    title: String,
    description: String,
    assignee_id: Option<u64>,
    reviewer_ids: Vec<u64>,
    remove_source_branch: bool,
    squash: bool,
) -> Result<MergeRequestInfo, GitCommandError> {
    gitlab::create_merge_request(
        &base_url,
        &token,
        &project_path,
        CreateMergeRequestParams {
            source_branch,
            target_branch,
            title,
            description,
            assignee_id,
            reviewer_ids,
            remove_source_branch,
            squash,
        },
    )
    .await
}

#[tauri::command]
pub async fn gitlab_search_members(
    base_url: String,
    token: String,
    project_path: String,
) -> Result<Vec<GitLabUser>, GitCommandError> {
    gitlab::search_project_members(&base_url, &token, &project_path).await
}

#[tauri::command]
pub async fn gitlab_list_project_events(
    base_url: String,
    token: String,
    project_path: String,
) -> Result<Vec<GitLabEvent>, GitCommandError> {
    gitlab::list_project_events(&base_url, &token, &project_path).await
}

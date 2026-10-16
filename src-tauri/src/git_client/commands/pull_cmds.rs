use tauri::State;

use crate::git_client::error::GitCommandError;
use crate::git_client::git::pull::{PullResult, PullStrategy, UpstreamStatus};
use crate::git_client::state::AppState;

/// `strategy`: overrides the repo's pull config for this one pull.
#[tauri::command]
pub async fn pull_branch(
    repo_id: String,
    strategy: Option<PullStrategy>,
    state: State<'_, AppState>,
) -> Result<PullResult, GitCommandError> {
    state.actor(&repo_id)?.pull(strategy).await
}

#[tauri::command]
pub async fn get_upstream_status(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<Option<UpstreamStatus>, GitCommandError> {
    state.actor(&repo_id)?.upstream_status().await
}

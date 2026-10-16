use tauri::State;
use uuid::Uuid;

use crate::git_client::error::GitCommandError;
use crate::git_client::git::repo::{RepoStatus, RepoSummary};
use crate::git_client::state::AppState;

/// A `\\wsl.localhost\<distro>\...` path opens through that distro's agent
/// (see `git_client::wsl`); anything else in this process.
#[tauri::command]
pub async fn open_repository(
    path: String,
    state: State<'_, AppState>,
) -> Result<RepoSummary, GitCommandError> {
    if let Some((distro, linux)) = crate::git_client::wslpath::parse_unc(&path) {
        return state.open_remote(&distro, linux).await;
    }
    state.open_local(Uuid::new_v4().to_string(), path)
}

#[tauri::command]
pub async fn get_repo_status(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<RepoStatus, GitCommandError> {
    state.actor(&repo_id)?.status().await
}

#[tauri::command]
pub async fn close_repository(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    let removed = state.repos.lock().unwrap().remove(&repo_id);
    if let Some(agent) = removed.and_then(|h| h.agent) {
        agent.call("close_repository", serde_json::json!({ "repoId": repo_id })).await?;
    }
    Ok(())
}

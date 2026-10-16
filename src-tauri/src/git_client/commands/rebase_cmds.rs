use tauri::State;

use crate::git_client::error::GitCommandError;
use crate::git_client::git::rebase::{ConflictResolution, RebaseStatus};
use crate::git_client::state::AppState;

#[tauri::command]
pub async fn start_rebase(
    repo_id: String,
    onto_branch: String,
    state: State<'_, AppState>,
) -> Result<RebaseStatus, GitCommandError> {
    state.actor(&repo_id)?.start_rebase(onto_branch).await
}

#[tauri::command]
pub async fn get_rebase_status(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<Option<RebaseStatus>, GitCommandError> {
    state.actor(&repo_id)?.rebase_status().await
}

#[tauri::command]
pub async fn resolve_rebase_conflict(
    repo_id: String,
    path: String,
    resolution: ConflictResolution,
    state: State<'_, AppState>,
) -> Result<RebaseStatus, GitCommandError> {
    state
        .actor(&repo_id)?
        .resolve_rebase_conflict(path, resolution)
        .await
}

#[tauri::command]
pub async fn continue_rebase(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<RebaseStatus, GitCommandError> {
    state.actor(&repo_id)?.continue_rebase().await
}

#[tauri::command]
pub async fn abort_rebase(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.abort_rebase().await
}

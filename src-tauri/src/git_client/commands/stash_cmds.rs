use tauri::State;

use crate::git_client::error::GitCommandError;
use crate::git_client::git::actor::StashSaveArgs;
use crate::git_client::git::stash::{StashInfo, StashPopResult};
use crate::git_client::state::AppState;

#[tauri::command]
pub async fn stash_save(
    repo_id: String,
    message: Option<String>,
    include_untracked: bool,
    state: State<'_, AppState>,
) -> Result<StashInfo, GitCommandError> {
    state
        .actor(&repo_id)?
        .stash_save(StashSaveArgs {
            message,
            include_untracked,
        })
        .await
}

#[tauri::command]
pub async fn stash_list(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<StashInfo>, GitCommandError> {
    state.actor(&repo_id)?.stash_list().await
}

#[tauri::command]
pub async fn stash_pop(
    repo_id: String,
    index: usize,
    state: State<'_, AppState>,
) -> Result<StashPopResult, GitCommandError> {
    state.actor(&repo_id)?.stash_pop(index).await
}

#[tauri::command]
pub async fn stash_apply(
    repo_id: String,
    index: usize,
    state: State<'_, AppState>,
) -> Result<StashPopResult, GitCommandError> {
    state.actor(&repo_id)?.stash_apply(index).await
}

#[tauri::command]
pub async fn stash_drop(
    repo_id: String,
    index: usize,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.stash_drop(index).await
}

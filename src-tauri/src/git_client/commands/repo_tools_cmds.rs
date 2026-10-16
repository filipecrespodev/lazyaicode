//! Worktrees, submodules and Git LFS.

use tauri::State;

use crate::git_client::error::GitCommandError;
use crate::git_client::git::actor::WorktreeOp;
use crate::git_client::git::external_git;
use crate::git_client::git::lfs::{self, LfsStatus};
use crate::git_client::git::submodule::SubmoduleInfo;
use crate::git_client::git::worktree::{AddWorktreeArgs, WorktreeInfo};
use crate::git_client::state::AppState;

#[tauri::command]
pub async fn list_worktrees(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<WorktreeInfo>, GitCommandError> {
    state.actor(&repo_id)?.worktree(WorktreeOp::List).await
}

#[tauri::command]
pub async fn add_worktree(
    repo_id: String,
    args: AddWorktreeArgs,
    state: State<'_, AppState>,
) -> Result<Vec<WorktreeInfo>, GitCommandError> {
    state.actor(&repo_id)?.worktree(WorktreeOp::Add(args)).await
}

#[tauri::command]
pub async fn remove_worktree(
    repo_id: String,
    name: String,
    force: bool,
    state: State<'_, AppState>,
) -> Result<Vec<WorktreeInfo>, GitCommandError> {
    state
        .actor(&repo_id)?
        .worktree(WorktreeOp::Remove { name, force })
        .await
}

#[tauri::command]
pub async fn prune_worktrees(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<WorktreeInfo>, GitCommandError> {
    state.actor(&repo_id)?.worktree(WorktreeOp::Prune).await
}

#[tauri::command]
pub async fn list_submodules(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<SubmoduleInfo>, GitCommandError> {
    state.actor(&repo_id)?.submodules(None).await
}

/// Decides whether the repo menu offers Submodules at all; cheap enough to
/// ask on every repo open, unlike `list_submodules`.
#[tauri::command]
pub async fn get_submodule_count(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<usize, GitCommandError> {
    state.actor(&repo_id)?.submodule_count().await
}

/// `git submodule update --init` for `paths` (every submodule when empty).
#[tauri::command]
pub async fn update_submodules(
    repo_id: String,
    paths: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<SubmoduleInfo>, GitCommandError> {
    state.actor(&repo_id)?.submodules(Some(paths)).await
}

/// Off the actor: the first call may spawn `git lfs version`.
#[tauri::command]
pub async fn get_lfs_status(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<LfsStatus, GitCommandError> {
    if let Some(agent) = state.agent_of(&repo_id)? {
        return agent.call_as("lfs_status", serde_json::json!({ "repoId": repo_id })).await;
    }
    let workdir = state.actor(&repo_id)?.workdir().await?;
    tauri::async_runtime::spawn_blocking(move || lfs::status(&workdir))
        .await
        .map_err(|e| GitCommandError::Internal(e.to_string()))
}

/// Runs off the actor so a long download doesn't hold up every other
/// command for this repo; git-lfs does its own locking.
#[tauri::command]
pub async fn run_lfs_pull(repo_id: String, state: State<'_, AppState>) -> Result<(), GitCommandError> {
    if let Some(agent) = state.agent_of(&repo_id)? {
        agent.call("lfs_pull", serde_json::json!({ "repoId": repo_id })).await?;
        return Ok(());
    }
    let workdir = state.actor(&repo_id)?.workdir().await?;
    tauri::async_runtime::spawn_blocking(move || external_git::lfs_pull(&workdir))
        .await
        .map_err(|e| GitCommandError::Internal(e.to_string()))?
}

use tauri::State;

use crate::git_client::error::GitCommandError;
use crate::git_client::git::log::{CommitLogPage, CommitNode, LogQuery};
use crate::git_client::git::reflog::ReflogEntry;
use crate::git_client::git::search::SearchMode;
use crate::git_client::git::signing::CommitSignature;
use crate::git_client::git::stats::RepoStats;
use crate::git_client::state::AppState;

#[tauri::command]
pub async fn get_commit_log(
    repo_id: String,
    query: LogQuery,
    state: State<'_, AppState>,
) -> Result<CommitLogPage, GitCommandError> {
    state.actor(&repo_id)?.log(query).await
}

#[tauri::command]
pub async fn get_head_commit(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<Option<CommitNode>, GitCommandError> {
    state.actor(&repo_id)?.head_commit().await
}

/// Whether a commit is signed (and how), for the commit details panel.
/// Asked for the selected commit only, never per graph row.
#[tauri::command]
pub async fn get_commit_signature(
    repo_id: String,
    sha: String,
    state: State<'_, AppState>,
) -> Result<Option<CommitSignature>, GitCommandError> {
    state.actor(&repo_id)?.commit_signature(sha).await
}

#[tauri::command]
pub async fn get_repo_stats(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<RepoStats, GitCommandError> {
    state.actor(&repo_id)?.stats().await
}

#[tauri::command]
pub async fn get_reflog(
    repo_id: String,
    ref_name: String,
    state: State<'_, AppState>,
) -> Result<Vec<ReflogEntry>, GitCommandError> {
    state.actor(&repo_id)?.reflog(ref_name).await
}

#[tauri::command]
pub async fn search_commits(
    repo_id: String,
    query: String,
    mode: SearchMode,
    state: State<'_, AppState>,
) -> Result<Vec<CommitNode>, GitCommandError> {
    state.actor(&repo_id)?.search_commits(query, mode).await
}

/// Token that changes when refs/HEAD/stash move (see `git::fingerprint`). Read
/// straight from disk, not through the repo actor: it must answer even while
/// the actor is busy.
#[tauri::command]
pub async fn get_repo_fingerprint(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<String, GitCommandError> {
    if let Some(agent) = state.agent_of(&repo_id)? {
        return agent.call_as("fingerprint", serde_json::json!({ "repoId": repo_id })).await;
    }
    let path = state.repo_path(&repo_id)?;
    crate::git_client::git::fingerprint::fingerprint(std::path::Path::new(&path))
}

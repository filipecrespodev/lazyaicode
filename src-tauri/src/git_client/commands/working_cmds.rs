use tauri::State;

use crate::git_client::error::GitCommandError;
use crate::git_client::git::actor::{AmendMessageArgs, ApplyLinesArgs, CommitChangesArgs, IgnoreArgs};
use crate::git_client::git::diff::CommitDiff;
use crate::git_client::git::log::CommitNode;
use crate::git_client::git::partial::{LineAction, LineSelection};
use crate::git_client::git::working::WorkingStatus;
use crate::git_client::state::AppState;

#[tauri::command]
pub async fn get_working_status(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<WorkingStatus, GitCommandError> {
    state.actor(&repo_id)?.working_status().await
}

#[tauri::command]
pub async fn stage_path(
    repo_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.stage_path(path).await
}

#[tauri::command]
pub async fn stage_all(repo_id: String, state: State<'_, AppState>) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.stage_all().await
}

#[tauri::command]
pub async fn unstage_path(
    repo_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.unstage_path(path).await
}

#[tauri::command]
pub async fn unstage_all(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.unstage_all().await
}

#[tauri::command]
pub async fn discard_path(
    repo_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.discard_path(path).await
}

#[tauri::command]
pub async fn discard_all(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.discard_all().await
}

/// Stages, unstages or discards only the selected changed lines of `path`
/// (a hunk is just all of its lines).
#[tauri::command]
pub async fn apply_lines(
    repo_id: String,
    path: String,
    action: LineAction,
    selection: LineSelection,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    state
        .actor(&repo_id)?
        .apply_lines(ApplyLinesArgs {
            path,
            action,
            selection,
        })
        .await
}

/// Appends `pattern` to `.gitignore`, or to `.git/info/exclude` when `local`.
/// Resolves to `false` when the pattern was already there.
#[tauri::command]
pub async fn add_ignore_pattern(
    repo_id: String,
    pattern: String,
    local: bool,
    state: State<'_, AppState>,
) -> Result<bool, GitCommandError> {
    state
        .actor(&repo_id)?
        .ignore_pattern(IgnoreArgs { pattern, local })
        .await
}

#[tauri::command]
pub async fn get_workdir_diff(
    repo_id: String,
    staged: bool,
    state: State<'_, AppState>,
) -> Result<CommitDiff, GitCommandError> {
    state.actor(&repo_id)?.workdir_diff(staged).await
}

#[tauri::command]
pub async fn git_commit_changes(
    repo_id: String,
    summary: String,
    body: String,
    author_name: Option<String>,
    author_email: Option<String>,
    amend: bool,
    no_verify: Option<bool>,
    state: State<'_, AppState>,
) -> Result<String, GitCommandError> {
    state
        .actor(&repo_id)?
        .commit_changes(CommitChangesArgs {
            summary,
            body,
            author_name,
            author_email,
            amend,
            no_verify: no_verify.unwrap_or(false),
        })
        .await
}

#[tauri::command]
pub async fn amend_commit_message(
    repo_id: String,
    sha: String,
    summary: String,
    body: String,
    no_verify: Option<bool>,
    state: State<'_, AppState>,
) -> Result<CommitNode, GitCommandError> {
    state
        .actor(&repo_id)?
        .amend_message(AmendMessageArgs { sha, summary, body, no_verify: no_verify.unwrap_or(false) })
        .await
}

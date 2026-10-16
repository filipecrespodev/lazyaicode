use tauri::State;

use crate::git_client::error::GitCommandError;
use crate::git_client::git::actor::{BlameArgs, CommitFileDiffArgs, CompareRefsArgs, RangeDiffArgs, RevertArgs};
use crate::git_client::git::diff::{CommitDiff, FileDiff};
use crate::git_client::git::history::{BlameChunk, BranchCompare, FileHistoryPage, FileHistoryQuery};
use crate::git_client::git::revert::RevertResult;
use crate::git_client::state::AppState;

#[tauri::command]
pub async fn revert_commit(
    repo_id: String,
    sha: String,
    mainline: Option<u32>,
    state: State<'_, AppState>,
) -> Result<RevertResult, GitCommandError> {
    state
        .actor(&repo_id)?
        .revert_commit(RevertArgs { sha, mainline })
        .await
}

#[tauri::command]
pub async fn get_file_history(
    repo_id: String,
    query: FileHistoryQuery,
    state: State<'_, AppState>,
) -> Result<FileHistoryPage, GitCommandError> {
    state.actor(&repo_id)?.file_history(query).await
}

#[tauri::command]
pub async fn get_commit_file_diff(
    repo_id: String,
    sha: String,
    path: String,
    old_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<Option<FileDiff>, GitCommandError> {
    state
        .actor(&repo_id)?
        .commit_file_diff(CommitFileDiffArgs { sha, path, old_path })
        .await
}

#[tauri::command]
pub async fn get_blame(
    repo_id: String,
    path: String,
    sha: Option<String>,
    start_line: u32,
    max_lines: u32,
    state: State<'_, AppState>,
) -> Result<BlameChunk, GitCommandError> {
    state
        .actor(&repo_id)?
        .blame(BlameArgs {
            path,
            sha,
            start_line,
            max_lines,
        })
        .await
}

#[tauri::command]
pub async fn get_branch_compare(
    repo_id: String,
    a: String,
    b: String,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<BranchCompare, GitCommandError> {
    state
        .actor(&repo_id)?
        .compare_refs(CompareRefsArgs { a, b, limit })
        .await
}

#[tauri::command]
pub async fn get_tree_diff(
    repo_id: String,
    from_sha: String,
    to_sha: String,
    state: State<'_, AppState>,
) -> Result<CommitDiff, GitCommandError> {
    state
        .actor(&repo_id)?
        .tree_diff(RangeDiffArgs { from_sha, to_sha })
        .await
}

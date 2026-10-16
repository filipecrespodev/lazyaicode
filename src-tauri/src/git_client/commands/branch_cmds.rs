use tauri::State;

use crate::git_client::error::GitCommandError;
use crate::git_client::git::actor::{
    CheckoutAtArgs, CherryPickArgs, CreateBranchArgs, DeleteBranchEverywhereArgs, DeleteRemoteBranchArgs, ResetToCommitArgs,
    ResetToRefArgs, TagOp,
};
use crate::git_client::git::branch::{BranchInfo, TagInfo};
use crate::git_client::git::cherry_pick::CherryPickResult;
use crate::git_client::git::merge::MergeResult;
use crate::git_client::git::repo::RepoStatus;
use crate::git_client::git::stash::StashPopResult;
use crate::git_client::state::AppState;

#[tauri::command]
pub async fn list_branches(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<BranchInfo>, GitCommandError> {
    state.actor(&repo_id)?.list_branches().await
}

#[tauri::command]
pub async fn list_tags(repo_id: String, state: State<'_, AppState>) -> Result<Vec<TagInfo>, GitCommandError> {
    state.actor(&repo_id)?.list_tags().await
}

#[tauri::command]
pub async fn create_branch(
    repo_id: String,
    name: String,
    start_point: Option<String>,
    checkout: bool,
    state: State<'_, AppState>,
) -> Result<BranchInfo, GitCommandError> {
    state
        .actor(&repo_id)?
        .create_branch(CreateBranchArgs {
            name,
            start_point,
            checkout,
        })
        .await
}

#[tauri::command]
pub async fn checkout_branch(
    repo_id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<RepoStatus, GitCommandError> {
    state.actor(&repo_id)?.checkout_branch(name).await
}

#[tauri::command]
pub async fn checkout_at(
    repo_id: String,
    name: String,
    target_sha: String,
    remote_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<RepoStatus, GitCommandError> {
    state
        .actor(&repo_id)?
        .checkout_at(CheckoutAtArgs {
            name,
            target_sha,
            remote_name,
        })
        .await
}

#[tauri::command]
pub async fn checkout_commit_detached(
    repo_id: String,
    sha: String,
    state: State<'_, AppState>,
) -> Result<RepoStatus, GitCommandError> {
    state.actor(&repo_id)?.checkout_commit_detached(sha).await
}

#[tauri::command]
pub async fn cherry_pick(
    repo_id: String,
    sha: String,
    create_commit: bool,
    state: State<'_, AppState>,
) -> Result<CherryPickResult, GitCommandError> {
    state
        .actor(&repo_id)?
        .cherry_pick(CherryPickArgs { sha, create_commit })
        .await
}

#[tauri::command]
pub async fn reset_to_commit(
    repo_id: String,
    sha: String,
    hard: bool,
    state: State<'_, AppState>,
) -> Result<RepoStatus, GitCommandError> {
    state
        .actor(&repo_id)?
        .reset_to_commit(ResetToCommitArgs { sha, hard })
        .await
}

#[tauri::command]
pub async fn reset_to_ref(
    repo_id: String,
    ref_name: String,
    hard: bool,
    state: State<'_, AppState>,
) -> Result<RepoStatus, GitCommandError> {
    state
        .actor(&repo_id)?
        .reset_to_ref(ResetToRefArgs { ref_name, hard })
        .await
}

#[tauri::command]
pub async fn sync_branch_to_remote(
    repo_id: String,
    target_sha: String,
    state: State<'_, AppState>,
) -> Result<StashPopResult, GitCommandError> {
    state
        .actor(&repo_id)?
        .sync_branch_to_remote(target_sha)
        .await
}

#[tauri::command]
pub async fn delete_local_branch(
    repo_id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.delete_local_branch(name).await
}

#[tauri::command]
pub async fn delete_remote_branch(
    repo_id: String,
    remote_name: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    state
        .actor(&repo_id)?
        .delete_remote_branch(DeleteRemoteBranchArgs {
            remote_name,
            branch: name,
        })
        .await
}

#[tauri::command]
pub async fn delete_branch_both(
    repo_id: String,
    remote_names: Vec<String>,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    state
        .actor(&repo_id)?
        .delete_branch_both(DeleteBranchEverywhereArgs {
            remote_names,
            branch: name,
        })
        .await
}

#[tauri::command]
pub async fn merge_branch(
    repo_id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<MergeResult, GitCommandError> {
    state.actor(&repo_id)?.merge_branch(name).await
}

#[tauri::command]
pub async fn create_tag(
    repo_id: String,
    name: String,
    sha: String,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.tag(TagOp::Create { name, sha }).await.map(|_| ())
}

#[tauri::command]
pub async fn delete_local_tag(
    repo_id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.tag(TagOp::DeleteLocal(name)).await.map(|_| ())
}

#[tauri::command]
pub async fn push_tag(
    repo_id: String,
    remote_name: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    let op = TagOp::Push { remote: remote_name, name };
    state.actor(&repo_id)?.tag(op).await.map(|_| ())
}

#[tauri::command]
pub async fn delete_remote_tag(
    repo_id: String,
    remote_name: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), GitCommandError> {
    let op = TagOp::DeleteRemote { remote: remote_name, name };
    state.actor(&repo_id)?.tag(op).await.map(|_| ())
}

#[tauri::command]
pub async fn list_remote_tags(
    repo_id: String,
    remote_name: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, GitCommandError> {
    state.actor(&repo_id)?.tag(TagOp::ListRemote(remote_name)).await
}

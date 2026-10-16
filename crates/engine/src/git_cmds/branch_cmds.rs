pub use lazyaicode_git::error::GitCommandError;
pub use lazyaicode_git::git::actor::{
    CheckoutAtArgs, CherryPickArgs, CreateBranchArgs, DeleteBranchEverywhereArgs, DeleteRemoteBranchArgs, ResetToCommitArgs,
    ResetToRefArgs, TagOp,
};
pub use lazyaicode_git::git::branch::{BranchInfo, TagInfo};
pub use lazyaicode_git::git::cherry_pick::CherryPickResult;
pub use lazyaicode_git::git::merge::MergeResult;
pub use lazyaicode_git::git::repo::RepoStatus;
pub use lazyaicode_git::git::stash::StashPopResult;
pub use lazyaicode_git::state::AppState;

pub async fn list_branches(repo_id: String, state: &AppState) -> Result<Vec<BranchInfo>, GitCommandError> {
    state.actor(&repo_id)?.list_branches().await
}

pub async fn list_tags(repo_id: String, state: &AppState) -> Result<Vec<TagInfo>, GitCommandError> {
    state.actor(&repo_id)?.list_tags().await
}

pub async fn create_branch(
    repo_id: String,
    name: String,
    start_point: Option<String>,
    checkout: bool,
    state: &AppState,
) -> Result<BranchInfo, GitCommandError> {
    state.actor(&repo_id)?.create_branch(CreateBranchArgs { name, start_point, checkout }).await
}

pub async fn checkout_branch(repo_id: String, name: String, state: &AppState) -> Result<RepoStatus, GitCommandError> {
    state.actor(&repo_id)?.checkout_branch(name).await
}

pub async fn checkout_at(
    repo_id: String,
    name: String,
    target_sha: String,
    remote_name: Option<String>,
    state: &AppState,
) -> Result<RepoStatus, GitCommandError> {
    state.actor(&repo_id)?.checkout_at(CheckoutAtArgs { name, target_sha, remote_name }).await
}

pub async fn checkout_commit_detached(repo_id: String, sha: String, state: &AppState) -> Result<RepoStatus, GitCommandError> {
    state.actor(&repo_id)?.checkout_commit_detached(sha).await
}

pub async fn cherry_pick(repo_id: String, sha: String, create_commit: bool, state: &AppState) -> Result<CherryPickResult, GitCommandError> {
    state.actor(&repo_id)?.cherry_pick(CherryPickArgs { sha, create_commit }).await
}

pub async fn reset_to_commit(repo_id: String, sha: String, hard: bool, state: &AppState) -> Result<RepoStatus, GitCommandError> {
    state.actor(&repo_id)?.reset_to_commit(ResetToCommitArgs { sha, hard }).await
}

pub async fn reset_to_ref(repo_id: String, ref_name: String, hard: bool, state: &AppState) -> Result<RepoStatus, GitCommandError> {
    state.actor(&repo_id)?.reset_to_ref(ResetToRefArgs { ref_name, hard }).await
}

pub async fn sync_branch_to_remote(repo_id: String, target_sha: String, state: &AppState) -> Result<StashPopResult, GitCommandError> {
    state.actor(&repo_id)?.sync_branch_to_remote(target_sha).await
}

pub async fn delete_local_branch(repo_id: String, name: String, state: &AppState) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.delete_local_branch(name).await
}

pub async fn delete_remote_branch(repo_id: String, remote_name: String, name: String, state: &AppState) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.delete_remote_branch(DeleteRemoteBranchArgs { remote_name, branch: name }).await
}

pub async fn delete_branch_both(repo_id: String, remote_names: Vec<String>, name: String, state: &AppState) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.delete_branch_both(DeleteBranchEverywhereArgs { remote_names, branch: name }).await
}

pub async fn merge_branch(repo_id: String, name: String, state: &AppState) -> Result<MergeResult, GitCommandError> {
    state.actor(&repo_id)?.merge_branch(name).await
}

pub async fn create_tag(repo_id: String, name: String, sha: String, state: &AppState) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.tag(TagOp::Create { name, sha }).await.map(|_| ())
}

pub async fn delete_local_tag(repo_id: String, name: String, state: &AppState) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.tag(TagOp::DeleteLocal(name)).await.map(|_| ())
}

pub async fn push_tag(repo_id: String, remote_name: String, name: String, state: &AppState) -> Result<(), GitCommandError> {
    let op = TagOp::Push { remote: remote_name, name };
    state.actor(&repo_id)?.tag(op).await.map(|_| ())
}

pub async fn delete_remote_tag(repo_id: String, remote_name: String, name: String, state: &AppState) -> Result<(), GitCommandError> {
    let op = TagOp::DeleteRemote { remote: remote_name, name };
    state.actor(&repo_id)?.tag(op).await.map(|_| ())
}

pub async fn list_remote_tags(repo_id: String, remote_name: String, state: &AppState) -> Result<Vec<String>, GitCommandError> {
    state.actor(&repo_id)?.tag(TagOp::ListRemote(remote_name)).await
}

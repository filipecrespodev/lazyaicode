pub use lazyaicode_git::error::GitCommandError;
pub use lazyaicode_git::git::actor::StashSaveArgs;
pub use lazyaicode_git::git::stash::{StashInfo, StashPopResult};
pub use lazyaicode_git::state::AppState;

pub async fn stash_save(
    repo_id: String,
    message: Option<String>,
    include_untracked: bool,
    state: &AppState,
) -> Result<StashInfo, GitCommandError> {
    state.actor(&repo_id)?.stash_save(StashSaveArgs { message, include_untracked }).await
}

pub async fn stash_list(repo_id: String, state: &AppState) -> Result<Vec<StashInfo>, GitCommandError> {
    state.actor(&repo_id)?.stash_list().await
}

pub async fn stash_pop(repo_id: String, index: usize, state: &AppState) -> Result<StashPopResult, GitCommandError> {
    state.actor(&repo_id)?.stash_pop(index).await
}

pub async fn stash_apply(repo_id: String, index: usize, state: &AppState) -> Result<StashPopResult, GitCommandError> {
    state.actor(&repo_id)?.stash_apply(index).await
}

pub async fn stash_drop(repo_id: String, index: usize, state: &AppState) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.stash_drop(index).await
}

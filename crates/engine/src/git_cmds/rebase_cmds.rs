pub use lazyaicode_git::error::GitCommandError;
pub use lazyaicode_git::git::rebase::{ConflictResolution, RebaseStatus};
pub use lazyaicode_git::state::AppState;

pub async fn start_rebase(repo_id: String, onto_branch: String, state: &AppState) -> Result<RebaseStatus, GitCommandError> {
    state.actor(&repo_id)?.start_rebase(onto_branch).await
}

pub async fn get_rebase_status(repo_id: String, state: &AppState) -> Result<Option<RebaseStatus>, GitCommandError> {
    state.actor(&repo_id)?.rebase_status().await
}

pub async fn resolve_rebase_conflict(
    repo_id: String,
    path: String,
    resolution: ConflictResolution,
    state: &AppState,
) -> Result<RebaseStatus, GitCommandError> {
    state.actor(&repo_id)?.resolve_rebase_conflict(path, resolution).await
}

pub async fn continue_rebase(repo_id: String, state: &AppState) -> Result<RebaseStatus, GitCommandError> {
    state.actor(&repo_id)?.continue_rebase().await
}

pub async fn abort_rebase(repo_id: String, state: &AppState) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.abort_rebase().await
}

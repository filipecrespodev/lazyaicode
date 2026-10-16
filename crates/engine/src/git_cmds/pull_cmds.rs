pub use lazyaicode_git::error::GitCommandError;
pub use lazyaicode_git::git::pull::{PullResult, PullStrategy, UpstreamStatus};
pub use lazyaicode_git::state::AppState;

/// `strategy`: overrides the repo's pull config for this one pull.
pub async fn pull_branch(repo_id: String, strategy: Option<PullStrategy>, state: &AppState) -> Result<PullResult, GitCommandError> {
    state.actor(&repo_id)?.pull(strategy).await
}

pub async fn get_upstream_status(repo_id: String, state: &AppState) -> Result<Option<UpstreamStatus>, GitCommandError> {
    state.actor(&repo_id)?.upstream_status().await
}

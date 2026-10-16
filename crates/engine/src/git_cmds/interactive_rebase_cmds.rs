pub use lazyaicode_git::error::GitCommandError;
pub use lazyaicode_git::git::actor::InteractiveRebaseArgs;
pub use lazyaicode_git::git::interactive_rebase::{RebasePlan, TodoItem};
pub use lazyaicode_git::git::rebase::RebaseStatus;
pub use lazyaicode_git::state::AppState;

/// Commits an interactive rebase starting at `from_sha` would rewrite.
pub async fn get_rebase_plan(repo_id: String, from_sha: String, state: &AppState) -> Result<RebasePlan, GitCommandError> {
    state.actor(&repo_id)?.rebase_plan(from_sha).await
}

pub async fn start_interactive_rebase(
    repo_id: String,
    upstream: Option<String>,
    items: Vec<TodoItem>,
    state: &AppState,
) -> Result<RebaseStatus, GitCommandError> {
    state.actor(&repo_id)?.start_interactive_rebase(InteractiveRebaseArgs { upstream, items }).await
}

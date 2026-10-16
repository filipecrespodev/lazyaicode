use tauri::State;

use crate::git_client::error::GitCommandError;
use crate::git_client::git::actor::InteractiveRebaseArgs;
use crate::git_client::git::interactive_rebase::{RebasePlan, TodoItem};
use crate::git_client::git::rebase::RebaseStatus;
use crate::git_client::state::AppState;

/// Commits an interactive rebase starting at `from_sha` would rewrite.
#[tauri::command]
pub async fn get_rebase_plan(
    repo_id: String,
    from_sha: String,
    state: State<'_, AppState>,
) -> Result<RebasePlan, GitCommandError> {
    state.actor(&repo_id)?.rebase_plan(from_sha).await
}

#[tauri::command]
pub async fn start_interactive_rebase(
    repo_id: String,
    upstream: Option<String>,
    items: Vec<TodoItem>,
    state: State<'_, AppState>,
) -> Result<RebaseStatus, GitCommandError> {
    state
        .actor(&repo_id)?
        .start_interactive_rebase(InteractiveRebaseArgs { upstream, items })
        .await
}

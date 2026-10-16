pub use lazyaicode_git::error::GitCommandError;
pub use lazyaicode_git::git::undo::UndoStatus;
pub use lazyaicode_git::state::AppState;

/// Reverts the last local action; returns its label.
pub async fn undo_last_action(repo_id: String, state: &AppState) -> Result<String, GitCommandError> {
    state.actor(&repo_id)?.undo().await
}

/// Re-applies the last undone action; returns its label.
pub async fn redo_last_action(repo_id: String, state: &AppState) -> Result<String, GitCommandError> {
    state.actor(&repo_id)?.redo().await
}

pub async fn get_undo_status(repo_id: String, state: &AppState) -> Result<UndoStatus, GitCommandError> {
    state.actor(&repo_id)?.undo_status().await
}

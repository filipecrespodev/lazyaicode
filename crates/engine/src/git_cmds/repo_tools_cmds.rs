//! Worktrees, submodules and Git LFS.

pub use lazyaicode_git::error::GitCommandError;
pub use lazyaicode_git::git::actor::WorktreeOp;
pub use lazyaicode_git::git::external_git;
pub use lazyaicode_git::git::lfs::{self, LfsStatus};
pub use lazyaicode_git::git::submodule::SubmoduleInfo;
pub use lazyaicode_git::git::worktree::{AddWorktreeArgs, WorktreeInfo};
pub use lazyaicode_git::state::AppState;

pub async fn list_worktrees(repo_id: String, state: &AppState) -> Result<Vec<WorktreeInfo>, GitCommandError> {
    state.actor(&repo_id)?.worktree(WorktreeOp::List).await
}

pub async fn add_worktree(repo_id: String, args: AddWorktreeArgs, state: &AppState) -> Result<Vec<WorktreeInfo>, GitCommandError> {
    state.actor(&repo_id)?.worktree(WorktreeOp::Add(args)).await
}

pub async fn remove_worktree(repo_id: String, name: String, force: bool, state: &AppState) -> Result<Vec<WorktreeInfo>, GitCommandError> {
    state.actor(&repo_id)?.worktree(WorktreeOp::Remove { name, force }).await
}

pub async fn prune_worktrees(repo_id: String, state: &AppState) -> Result<Vec<WorktreeInfo>, GitCommandError> {
    state.actor(&repo_id)?.worktree(WorktreeOp::Prune).await
}

pub async fn list_submodules(repo_id: String, state: &AppState) -> Result<Vec<SubmoduleInfo>, GitCommandError> {
    state.actor(&repo_id)?.submodules(None).await
}

/// Decides whether the repo menu offers Submodules at all; cheap enough to
/// ask on every repo open, unlike `list_submodules`.
pub async fn get_submodule_count(repo_id: String, state: &AppState) -> Result<usize, GitCommandError> {
    state.actor(&repo_id)?.submodule_count().await
}

/// `git submodule update --init` for `paths` (every submodule when empty).
pub async fn update_submodules(repo_id: String, paths: Vec<String>, state: &AppState) -> Result<Vec<SubmoduleInfo>, GitCommandError> {
    state.actor(&repo_id)?.submodules(Some(paths)).await
}

/// Off the actor: the first call may spawn `git lfs version`.
pub async fn get_lfs_status(repo_id: String, state: &AppState) -> Result<LfsStatus, GitCommandError> {
    if let Some(agent) = state.agent_of(&repo_id)? {
        return agent.call_as("lfs_status", serde_json::json!({ "repoId": repo_id })).await;
    }
    let workdir = state.actor(&repo_id)?.workdir().await?;
    tokio::task::spawn_blocking(move || lfs::status(&workdir)).await.map_err(|e| GitCommandError::Internal(e.to_string()))
}

/// Runs off the actor so a long download doesn't hold up every other
/// command for this repo; git-lfs does its own locking.
pub async fn run_lfs_pull(repo_id: String, state: &AppState) -> Result<(), GitCommandError> {
    if let Some(agent) = state.agent_of(&repo_id)? {
        agent.call("lfs_pull", serde_json::json!({ "repoId": repo_id })).await?;
        return Ok(());
    }
    let workdir = state.actor(&repo_id)?.workdir().await?;
    tokio::task::spawn_blocking(move || external_git::lfs_pull(&workdir)).await.map_err(|e| GitCommandError::Internal(e.to_string()))?
}

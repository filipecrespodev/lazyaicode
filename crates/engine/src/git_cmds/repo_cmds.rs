use uuid::Uuid;

pub use lazyaicode_git::error::GitCommandError;
pub use lazyaicode_git::git::repo::{RepoStatus, RepoSummary};
pub use lazyaicode_git::state::AppState;

/// A `\\wsl.localhost\<distro>\...` path opens through that distro's agent
/// (see `git_client::wsl`); anything else in this process.
pub async fn open_repository(path: String, state: &AppState) -> Result<RepoSummary, GitCommandError> {
    if let Some((distro, linux)) = lazyaicode_platform::wslpath::parse_unc(&path) {
        return state.open_remote(&distro, linux).await;
    }
    state.open_local(Uuid::new_v4().to_string(), path)
}

pub async fn get_repo_status(repo_id: String, state: &AppState) -> Result<RepoStatus, GitCommandError> {
    state.actor(&repo_id)?.status().await
}

pub async fn close_repository(repo_id: String, state: &AppState) -> Result<(), GitCommandError> {
    let removed = state.repos.lock().unwrap().remove(&repo_id);
    if let Some(agent) = removed.and_then(|h| h.agent) {
        agent.call("close_repository", serde_json::json!({ "repoId": repo_id })).await?;
    }
    Ok(())
}

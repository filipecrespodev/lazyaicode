pub use lazyaicode_git::error::GitCommandError;
pub use lazyaicode_git::git::log::{CommitLogPage, CommitNode, LogQuery};
pub use lazyaicode_git::git::reflog::ReflogEntry;
pub use lazyaicode_git::git::search::SearchMode;
pub use lazyaicode_git::git::signing::CommitSignature;
pub use lazyaicode_git::git::stats::RepoStats;
pub use lazyaicode_git::state::AppState;

pub async fn get_commit_log(repo_id: String, query: LogQuery, state: &AppState) -> Result<CommitLogPage, GitCommandError> {
    state.actor(&repo_id)?.log(query).await
}

pub async fn get_head_commit(repo_id: String, state: &AppState) -> Result<Option<CommitNode>, GitCommandError> {
    state.actor(&repo_id)?.head_commit().await
}

/// Whether a commit is signed (and how), for the commit details panel.
/// Asked for the selected commit only, never per graph row.
pub async fn get_commit_signature(repo_id: String, sha: String, state: &AppState) -> Result<Option<CommitSignature>, GitCommandError> {
    state.actor(&repo_id)?.commit_signature(sha).await
}

pub async fn get_repo_stats(repo_id: String, state: &AppState) -> Result<RepoStats, GitCommandError> {
    state.actor(&repo_id)?.stats().await
}

pub async fn get_reflog(repo_id: String, ref_name: String, state: &AppState) -> Result<Vec<ReflogEntry>, GitCommandError> {
    state.actor(&repo_id)?.reflog(ref_name).await
}

pub async fn search_commits(
    repo_id: String,
    query: String,
    mode: SearchMode,
    state: &AppState,
) -> Result<Vec<CommitNode>, GitCommandError> {
    state.actor(&repo_id)?.search_commits(query, mode).await
}

/// Token that changes when refs/HEAD/stash move (see `git::fingerprint`). Read
/// straight from disk, not through the repo actor: it must answer even while
/// the actor is busy.
pub async fn get_repo_fingerprint(repo_id: String, state: &AppState) -> Result<String, GitCommandError> {
    if let Some(agent) = state.agent_of(&repo_id)? {
        return agent.call_as("fingerprint", serde_json::json!({ "repoId": repo_id })).await;
    }
    let path = state.repo_path(&repo_id)?;
    lazyaicode_git::git::fingerprint::fingerprint(std::path::Path::new(&path))
}

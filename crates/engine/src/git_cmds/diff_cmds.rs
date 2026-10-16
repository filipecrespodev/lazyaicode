pub use lazyaicode_git::error::GitCommandError;
pub use lazyaicode_git::git::actor::{FileContentArgs, RangeDiffArgs};
pub use lazyaicode_git::git::diff::CommitDiff;
pub use lazyaicode_git::state::AppState;

pub async fn get_commit_diff(repo_id: String, sha: String, state: &AppState) -> Result<CommitDiff, GitCommandError> {
    state.actor(&repo_id)?.diff(sha).await
}

pub async fn get_range_diff(repo_id: String, from_sha: String, to_sha: String, state: &AppState) -> Result<CommitDiff, GitCommandError> {
    state.actor(&repo_id)?.range_diff(RangeDiffArgs { from_sha, to_sha }).await
}

pub async fn get_file_content(repo_id: String, sha: Option<String>, path: String, state: &AppState) -> Result<String, GitCommandError> {
    state.actor(&repo_id)?.file_content(FileContentArgs { sha, path }).await
}

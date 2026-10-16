pub use lazyaicode_git::error::GitCommandError;
pub use lazyaicode_git::git::actor::{AmendMessageArgs, ApplyLinesArgs, CommitChangesArgs, IgnoreArgs};
pub use lazyaicode_git::git::diff::CommitDiff;
pub use lazyaicode_git::git::log::CommitNode;
pub use lazyaicode_git::git::partial::{LineAction, LineSelection};
pub use lazyaicode_git::git::working::WorkingStatus;
pub use lazyaicode_git::state::AppState;

pub async fn get_working_status(repo_id: String, state: &AppState) -> Result<WorkingStatus, GitCommandError> {
    state.actor(&repo_id)?.working_status().await
}

pub async fn stage_path(repo_id: String, path: String, state: &AppState) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.stage_path(path).await
}

pub async fn stage_all(repo_id: String, state: &AppState) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.stage_all().await
}

pub async fn unstage_path(repo_id: String, path: String, state: &AppState) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.unstage_path(path).await
}

pub async fn unstage_all(repo_id: String, state: &AppState) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.unstage_all().await
}

pub async fn discard_path(repo_id: String, path: String, state: &AppState) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.discard_path(path).await
}

pub async fn discard_all(repo_id: String, state: &AppState) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.discard_all().await
}

/// Stages, unstages or discards only the selected changed lines of `path`
/// (a hunk is just all of its lines).
pub async fn apply_lines(
    repo_id: String,
    path: String,
    action: LineAction,
    selection: LineSelection,
    state: &AppState,
) -> Result<(), GitCommandError> {
    state.actor(&repo_id)?.apply_lines(ApplyLinesArgs { path, action, selection }).await
}

/// Appends `pattern` to `.gitignore`, or to `.git/info/exclude` when `local`.
/// Resolves to `false` when the pattern was already there.
pub async fn add_ignore_pattern(repo_id: String, pattern: String, local: bool, state: &AppState) -> Result<bool, GitCommandError> {
    state.actor(&repo_id)?.ignore_pattern(IgnoreArgs { pattern, local }).await
}

pub async fn get_workdir_diff(repo_id: String, staged: bool, state: &AppState) -> Result<CommitDiff, GitCommandError> {
    state.actor(&repo_id)?.workdir_diff(staged).await
}

#[allow(clippy::too_many_arguments)]
pub async fn git_commit_changes(
    repo_id: String,
    summary: String,
    body: String,
    author_name: Option<String>,
    author_email: Option<String>,
    amend: bool,
    no_verify: Option<bool>,
    state: &AppState,
) -> Result<String, GitCommandError> {
    state
        .actor(&repo_id)?
        .commit_changes(CommitChangesArgs { summary, body, author_name, author_email, amend, no_verify: no_verify.unwrap_or(false) })
        .await
}

pub async fn amend_commit_message(
    repo_id: String,
    sha: String,
    summary: String,
    body: String,
    no_verify: Option<bool>,
    state: &AppState,
) -> Result<CommitNode, GitCommandError> {
    state.actor(&repo_id)?.amend_message(AmendMessageArgs { sha, summary, body, no_verify: no_verify.unwrap_or(false) }).await
}

pub use crate::ai;
pub use lazyaicode_git::error::GitCommandError;
pub use lazyaicode_git::git::actor::ComposeArgs;
pub use lazyaicode_git::git::compose::ComposedCommitInput;
pub use lazyaicode_git::state::AppState;

pub async fn ai_generate_commit_message(
    model: String,
    diff: String,
    custom_instructions: String,
    app: &crate::Engine,
) -> Result<ai::CommitMessageResult, GitCommandError> {
    let dir = app.active_claude_dir();
    ai::generate_commit_message(&dir, &model, &diff, &custom_instructions).await
}

pub async fn ai_compose_commits(
    repo_id: String,
    model: String,
    custom_instructions: String,
    answers: String,
    state: &AppState,
    app: &crate::Engine,
) -> Result<ai::ComposeResult, GitCommandError> {
    let staged = state.actor(&repo_id)?.staged_units().await?;
    let units: Vec<(String, String)> = staged.units.into_iter().map(|u| (u.id, u.text)).collect();
    let dir = app.active_claude_dir();
    let mut result = ai::compose_commits(&dir, &model, &units, &custom_instructions, &answers).await?;
    result.tree_id = staged.tree_id;
    Ok(result)
}

pub async fn apply_composed_commits(
    repo_id: String,
    tree_id: String,
    commits: Vec<ComposedCommitInput>,
    author_name: Option<String>,
    author_email: Option<String>,
    no_verify: Option<bool>,
    state: &AppState,
) -> Result<Vec<String>, GitCommandError> {
    state
        .actor(&repo_id)?
        .apply_compose(ComposeArgs { tree_id, commits, author_name, author_email, no_verify: no_verify.unwrap_or(false) })
        .await
}

pub async fn ai_resolve_conflict(
    model: String,
    path: String,
    ancestor: Option<String>,
    ours: String,
    theirs: String,
    custom_instructions: String,
    app: &crate::Engine,
) -> Result<String, GitCommandError> {
    let dir = app.active_claude_dir();
    ai::resolve_conflict(&dir, &model, &path, ancestor.as_deref(), &ours, &theirs, &custom_instructions).await
}

pub async fn ai_generate_pr_description(
    model: String,
    commits: Vec<String>,
    diff: String,
    app: &crate::Engine,
) -> Result<ai::CommitMessageResult, GitCommandError> {
    let dir = app.active_claude_dir();
    ai::generate_pr_description(&dir, &model, &commits, &diff).await
}

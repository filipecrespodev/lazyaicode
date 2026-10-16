use tauri::State;

use crate::git_client::error::GitCommandError;
use crate::git_client::git::actor::{PushArgs, RemoteOp};
use crate::git_client::git::remote::{AuthMode, FetchSummary, PushSummary, RemoteInfo};
use crate::git_client::state::AppState;

/// `remote_name`: `None` fetches every remote. `background`: a fetch the user
/// didn't ask for (on open, periodic), which must not prompt for an SSH key
/// passphrase or HTTPS credentials.
#[tauri::command]
pub async fn fetch_remote(
    repo_id: String,
    remote_name: Option<String>,
    background: Option<bool>,
    state: State<'_, AppState>,
) -> Result<FetchSummary, GitCommandError> {
    state
        .actor(&repo_id)?
        .fetch(remote_name, auth_mode(background))
        .await
}

pub fn auth_mode(background: Option<bool>) -> AuthMode {
    if background.unwrap_or(false) {
        AuthMode::Background
    } else {
        AuthMode::Interactive
    }
}

#[tauri::command]
pub async fn get_remote_url(
    repo_id: String,
    remote_name: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, GitCommandError> {
    state.actor(&repo_id)?.get_remote_url(remote_name).await
}

/// `remote_name`: `None` pushes where `git push` would (upstream, else
/// `origin`, else the only remote), failing with `RemoteRequired` when that's
/// ambiguous so the UI can ask.
#[tauri::command]
pub async fn push_branch(
    repo_id: String,
    remote_name: Option<String>,
    branch: String,
    force: bool,
    state: State<'_, AppState>,
) -> Result<PushSummary, GitCommandError> {
    state
        .actor(&repo_id)?
        .push(PushArgs {
            remote_name,
            branch,
            force,
        })
        .await
}

#[tauri::command]
pub async fn list_remotes(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<RemoteInfo>, GitCommandError> {
    state.actor(&repo_id)?.list_remotes().await
}

#[tauri::command]
pub async fn add_remote(
    repo_id: String,
    name: String,
    url: String,
    state: State<'_, AppState>,
) -> Result<Vec<RemoteInfo>, GitCommandError> {
    state.actor(&repo_id)?.remote(RemoteOp::Add { name, url }).await
}

#[tauri::command]
pub async fn remove_remote(
    repo_id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<Vec<RemoteInfo>, GitCommandError> {
    state.actor(&repo_id)?.remote(RemoteOp::Remove(name)).await
}

#[tauri::command]
pub async fn rename_remote(
    repo_id: String,
    name: String,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<Vec<RemoteInfo>, GitCommandError> {
    state
        .actor(&repo_id)?
        .remote(RemoteOp::Rename { from: name, to: new_name })
        .await
}

#[tauri::command]
pub async fn set_remote_url(
    repo_id: String,
    name: String,
    url: String,
    state: State<'_, AppState>,
) -> Result<Vec<RemoteInfo>, GitCommandError> {
    state.actor(&repo_id)?.remote(RemoteOp::SetUrl { name, url }).await
}

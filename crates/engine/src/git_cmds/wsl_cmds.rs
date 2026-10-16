//! Commands whose answer depends on which machine (this one or a WSL distro)
//! the question is about, plus the WSL-only ones. Off Windows there are no
//! distros, so these just answer for this machine.

use serde_json::json;

pub use lazyaicode_git::error::GitCommandError;
pub use lazyaicode_git::host::DirEntry;
pub use lazyaicode_git::https_credentials::{self, CredentialsRequest};
pub use lazyaicode_git::passphrase;
pub use lazyaicode_git::ssh_keys::{self, SshKeyInfo};
pub use lazyaicode_git::state::AppState;
pub use lazyaicode_git::{wsl, wslpath};

/// Installed WSL distros (empty off Windows).
pub async fn list_wsl_distros() -> Vec<String> {
    wsl::blocking(wsl::list_distros).await.unwrap_or_default()
}

/// The distro's home folder, as a UNC path: where the WSL folder picker starts.
pub async fn wsl_home(distro: String, state: &AppState) -> Result<String, GitCommandError> {
    let agent = state.agent(&distro).await?;
    Ok(wslpath::to_unc(&distro, &agent.home))
}

/// Subfolders of a WSL UNC path, for the in-app folder picker (the native
/// dialog crawls over 9P).
pub async fn list_wsl_dirs(path: String, state: &AppState) -> Result<Vec<DirEntry>, GitCommandError> {
    let (agent, linux) = state.route(&path).await?.ok_or_else(|| GitCommandError::Internal(format!("não é um caminho do WSL: {path}")))?;
    let mut dirs: Vec<DirEntry> = agent.call_as("list_dirs", json!({ "path": linux })).await?;
    for d in &mut dirs {
        d.path = wslpath::to_unc(&agent.distro, &d.path);
    }
    Ok(dirs)
}

/// Keys in `~/.ssh` of this machine, or of `distro` when given.
pub async fn list_ssh_keys(distro: Option<String>, state: &AppState) -> Result<Vec<SshKeyInfo>, GitCommandError> {
    match distro {
        Some(d) => state.agent(&d).await?.call_as("list_ssh_keys", json!({})).await,
        None => Ok(ssh_keys::list_ssh_keys()),
    }
}

/// Answers the passphrase prompt, wherever it came from (this process or an
/// agent: the one that asked is not tracked).
pub async fn submit_ssh_passphrase(passphrase: Option<String>, state: &AppState) -> Result<(), ()> {
    passphrase::submit_ssh_passphrase(passphrase.clone());
    state.broadcast("submit_ssh_passphrase", json!({ "passphrase": passphrase })).await;
    Ok(())
}

pub async fn pending_ssh_passphrase(state: &AppState) -> Result<Option<String>, ()> {
    Ok(match passphrase::pending_ssh_passphrase() {
        Some(key) => Some(key),
        None => state.first_answer("pending_ssh_passphrase").await,
    })
}

pub async fn submit_https_credentials(username: Option<String>, password: Option<String>, state: &AppState) -> Result<(), ()> {
    https_credentials::submit_https_credentials(username.clone(), password.clone());
    state.broadcast("submit_https_credentials", json!({ "username": username, "password": password })).await;
    Ok(())
}

pub async fn pending_https_credentials(state: &AppState) -> Result<Option<CredentialsRequest>, ()> {
    Ok(match https_credentials::pending_https_credentials() {
        Some(request) => Some(request),
        None => state.first_answer("pending_https_credentials").await,
    })
}

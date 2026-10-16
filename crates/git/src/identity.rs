use serde::{Deserialize, Serialize};

use crate::error::GitCommandError;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalGitIdentity {
    pub name: Option<String>,
    pub email: Option<String>,
}

/// The machine's global git identity (`user.name`/`user.email`).
pub fn global() -> Result<GlobalGitIdentity, GitCommandError> {
    let cfg = git2::Config::open_default()?;
    Ok(GlobalGitIdentity { name: cfg.get_string("user.name").ok(), email: cfg.get_string("user.email").ok() })
}

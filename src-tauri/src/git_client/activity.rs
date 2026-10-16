//! What the backend does under the hood (git CLI runs, network round trips,
//! SSH auth steps), streamed to the frontend's Activity Logs as it happens.
//! The frontend logs each command it invokes itself; this adds the detail.

use std::path::Path;

use serde::Serialize;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Info,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    /// Repo working directory, or `None` for app-wide activity.
    pub repo_path: Option<String>,
    pub level: Level,
    pub message: String,
}

impl ActivityEvent {
    fn new(repo: Option<&Path>, level: Level, message: String) -> Self {
        Self {
            repo_path: repo.map(|p| p.display().to_string()),
            level,
            message,
        }
    }
}

/// No-op without a UI (tests).
pub fn log(repo: Option<&Path>, level: Level, message: impl Into<String>) {
    crate::git_client::events::emit("activity://log", ActivityEvent::new(repo, level, message.into()));
}

pub fn info(repo: Option<&Path>, message: impl Into<String>) {
    log(repo, Level::Info, message);
}

pub fn error(repo: Option<&Path>, message: impl Into<String>) {
    log(repo, Level::Error, message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_for_the_frontend() {
        let event = ActivityEvent::new(Some(Path::new("/r")), Level::Error, "boom".into());
        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({ "repoPath": "/r", "level": "error", "message": "boom" })
        );
    }

    #[test]
    fn logging_without_a_ui_is_a_no_op() {
        info(None, "nobody listening");
    }
}

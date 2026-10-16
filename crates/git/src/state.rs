use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::GitCommandError;
use crate::git::actor::RepoActor;
use crate::git::repo::{self, RepoSummary};
use crate::remote_agent::AgentConn;

pub struct RepoHandle {
    pub actor: RepoActor,
    pub path: String,
    /// Set for repos living in WSL: the agent that hosts them.
    pub agent: Option<Arc<AgentConn>>,
}

#[derive(Default)]
pub struct AppState {
    pub repos: Mutex<HashMap<String, RepoHandle>>,
    /// Live `lazyaicode-agent` connections by WSL distro name (see `wsl`).
    pub agents: Mutex<HashMap<String, Arc<AgentConn>>>,
}

impl AppState {
    /// Working directory the repo was opened at.
    pub fn repo_path(&self, repo_id: &str) -> Result<String, GitCommandError> {
        self.repos
            .lock()
            .unwrap()
            .get(repo_id)
            .map(|handle| handle.path.clone())
            .ok_or_else(|| GitCommandError::RepoNotFound { path: repo_id.to_string() })
    }

    pub fn actor(&self, repo_id: &str) -> Result<RepoActor, GitCommandError> {
        self.repos
            .lock()
            .unwrap()
            .get(repo_id)
            .map(|handle| handle.actor.clone())
            .ok_or_else(|| GitCommandError::RepoNotFound { path: repo_id.to_string() })
    }

    /// The agent hosting `repo_id`, `None` for a repo on this machine.
    pub fn agent_of(&self, repo_id: &str) -> Result<Option<Arc<AgentConn>>, GitCommandError> {
        self.repos
            .lock()
            .unwrap()
            .get(repo_id)
            .map(|handle| handle.agent.clone())
            .ok_or_else(|| GitCommandError::RepoNotFound { path: repo_id.to_string() })
    }

    /// Opens `path` in this process (the app for local repos, the agent for
    /// WSL ones) and starts its actor.
    pub fn open_local(&self, repo_id: String, path: String) -> Result<RepoSummary, GitCommandError> {
        let repository = repo::open(&path)?;
        let path = repo::root_path(path, &repository);
        let summary = repo::summary(repo_id.clone(), path.clone(), &repository);
        let actor = RepoActor::spawn(repository, repo_id.clone());
        self.repos.lock().unwrap().insert(repo_id, RepoHandle { actor, path, agent: None });
        Ok(summary)
    }
}

//! Cópia local do que veio do motor (listas, transcrições, status). A UI
//! desenha daqui; cada resposta ou evento atualiza a parte que mudou.

use std::collections::{HashMap, HashSet};

use lazyaicode_doc::transcript::Transcript;
use lazyaicode_doc::usage::ContextUse;
use lazyaicode_doc::{Project, Settings, Status, ThreadView, Workspace};
use lazyaicode_protocol::CommandInfo;
use serde_json::Value;

/// Uma imagem anexada ao rascunho.
#[derive(Clone)]
pub struct Attachment {
    pub mime: String,
    /// base64
    pub data: String,
    pub name: String,
}

#[derive(Default)]
pub struct Data {
    /// Comandos do motor (`engine.commands`) mais os de tela.
    pub commands: Vec<CommandInfo>,
    pub settings: Option<Settings>,
    pub claude_binary: Option<String>,
    pub default_claude_dir: String,
    pub suggested_claude_dir: String,
    pub data_dir: String,
    pub workspaces: Vec<Workspace>,
    pub active_workspace: Option<String>,
    pub projects: Vec<Project>,
    pub threads: Vec<ThreadView>,
    pub transcripts: HashMap<String, Transcript>,
    pub loading_transcript: HashSet<String>,
    /// `thread.diff-stats` por thread
    pub diff_stats: HashMap<String, Value>,
    /// `thread.branch-info` por thread
    pub branch: HashMap<String, Value>,
    pub context: HashMap<String, ContextUse>,
    /// `capabilities.get` por `workspace|pasta`
    pub caps: HashMap<String, Value>,
    pub caps_loading: HashSet<String>,
    pub profile: Option<Value>,
    /// `thread.changes` por thread (da base escolhida)
    pub changes: HashMap<String, Value>,
    pub providers: HashMap<String, Value>,
    pub accounts: Vec<Value>,
    pub auth: HashMap<String, Value>,
    /// repositórios das pastas da tela de workspace
    pub workspace_repos: HashMap<String, Vec<Value>>,
    /// MCP da sessão aberta de cada thread
    pub thread_mcp: HashMap<String, Value>,
    /// sessão ACP (modos/modelos/comandos) por thread
    pub acp_session: HashMap<String, Value>,
    pub version: Option<Value>,
    pub connected: bool,
    pub connection_error: Option<String>,
    pub drafts: HashMap<String, String>,
    pub attachments: HashMap<String, Vec<Attachment>>,
    pub prefs: serde_json::Map<String, Value>,
    pub booted: bool,
}

impl Data {
    pub fn thread(&self, id: &str) -> Option<&ThreadView> {
        self.threads.iter().find(|t| t.thread.id == id)
    }

    pub fn thread_mut(&mut self, id: &str) -> Option<&mut ThreadView> {
        self.threads.iter_mut().find(|t| t.thread.id == id)
    }

    pub fn project(&self, id: &str) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }

    pub fn workspace(&self, id: &str) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.id == id)
    }

    pub fn active(&self) -> Option<&Workspace> {
        self.active_workspace.as_deref().and_then(|id| self.workspace(id))
    }

    /// Troca a thread pela versão nova, ou acrescenta.
    pub fn upsert(&mut self, t: ThreadView) {
        match self.thread_mut(&t.thread.id) {
            Some(old) => *old = t,
            None => self.threads.push(t),
        }
    }

    pub fn set_status(&mut self, id: &str, s: Status) {
        if let Some(t) = self.thread_mut(id) {
            t.status = s;
        }
    }

    pub fn command(&self, id: &str) -> Option<&CommandInfo> {
        self.commands.iter().find(|c| c.id == id)
    }

    pub fn caps_key(&self, path: Option<&str>) -> String {
        format!("{}|{}", self.active_workspace.as_deref().unwrap_or(""), path.unwrap_or(""))
    }
}

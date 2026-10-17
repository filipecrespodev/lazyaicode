//! O estado persistente do app: workspaces, projetos, threads e preferências.
//! Só dados (serde); quem lê e grava o `state.json` é o motor.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: i64,
    /// Tem `.git` (recalculado a cada listagem): sem ele não há abas git
    /// nem workspace isolado.
    #[serde(default)]
    pub is_git: bool,
}

/// Uma pasta que agrupa repositórios, com a sua própria conta Claude
/// (diretório de configuração). Aparece como uma aba na barra de título.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: String,
    /// `None` = `~/.claude`.
    pub claude_dir: Option<String>,
    #[serde(default)]
    pub ignored_paths: Vec<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Thread {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    /// Pasta onde o agente trabalha: um checkout isolado ou o próprio projeto.
    pub cwd: String,
    pub isolated: bool,
    pub branch: Option<String>,
    /// Ref de onde o checkout isolado partiu (ex.: `origin/main`).
    pub base_ref: Option<String>,
    pub session_id: Option<String>,
    pub model: String,
    pub permission_mode: String,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub bookmarked: bool,
    /// Arquivo → impressão digital do diff no momento em que foi marcado
    /// como visto. Um diff diferente desmarca sozinho.
    #[serde(default)]
    pub seen: HashMap<String, String>,
    /// Árvore git capturada no início do último turno (diff "Último turno").
    #[serde(default)]
    pub turn_tree: Option<String>,
    #[serde(default)]
    pub unread: bool,
    /// Diretório Claude dono da sessão: uma sessão só pode ser retomada
    /// com as credenciais e o histórico do diretório onde nasceu.
    #[serde(default)]
    pub claude_dir: Option<String>,
    /// Sessões anteriores desta thread (quando um resume gera id novo), para
    /// a importação não trazê-las de volta como threads separadas.
    #[serde(default)]
    pub past_sessions: Vec<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub add_dirs: Vec<String>,
    /// claude (Claude Code nativo) | codex (OpenAI) | gemini (Google)
    #[serde(default = "claude_provider")]
    pub provider: String,
    /// roda com `--dangerously-skip-permissions` (só Claude Code); vale em
    /// toda abertura do processo, inclusive ao retomar a sessão
    #[serde(default)]
    pub skip_permissions: bool,
}

fn claude_provider() -> String {
    "claude".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub default_model: String,
    pub default_permission_mode: String,
    pub send_with_modifier: bool,
    #[serde(default = "yes")]
    pub notify_on_finish: bool,
    #[serde(default = "yes")]
    pub sound_on_finish: bool,
    #[serde(default = "claude_provider")]
    pub default_provider: String,
    #[serde(default)]
    pub default_skip_permissions: bool,
    /// idioma da interface: en, pt, es, zh
    #[serde(default = "english")]
    pub language: String,
    /// avisar quando sair uma versão nova (consulta o último release)
    #[serde(default = "yes")]
    pub check_updates: bool,
    /// Preferências da interface e do cliente git (tema, fontes, `sshKeys`,
    /// `signCommits`…), chave → JSON. Ficam junto do estado para que a CLI,
    /// o MCP e a versão web vejam as mesmas.
    #[serde(default)]
    pub prefs: Map<String, Value>,
}

fn yes() -> bool {
    true
}

fn english() -> String {
    "en".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_model: "default".into(),
            default_permission_mode: "default".into(),
            send_with_modifier: false,
            notify_on_finish: true,
            sound_on_finish: true,
            default_provider: claude_provider(),
            default_skip_permissions: false,
            language: english(),
            check_updates: true,
            prefs: Map::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct State {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub workspaces: Vec<Workspace>,
    #[serde(default)]
    pub active_workspace: Option<String>,
    #[serde(default)]
    pub projects: Vec<Project>,
    #[serde(default)]
    pub threads: Vec<Thread>,
}

impl State {
    pub fn thread(&self, id: &str) -> Option<&Thread> {
        self.threads.iter().find(|t| t.id == id)
    }

    pub fn thread_mut(&mut self, id: &str) -> Option<&mut Thread> {
        self.threads.iter_mut().find(|t| t.id == id)
    }

    pub fn workspace(&self, id: &str) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.id == id)
    }

    pub fn project(&self, id: &str) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }
}

/// Situação de execução de uma thread.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Idle,
    Running,
    Awaiting,
    Error,
}

impl Status {
    pub fn parse(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "awaiting" => Self::Awaiting,
            "error" => Self::Error,
            _ => Self::Idle,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Awaiting => "awaiting",
            Self::Error => "error",
        }
    }
}

/// Thread como a UI recebe: os dados persistidos mais a situação ao vivo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreadView {
    #[serde(flatten)]
    pub thread: Thread,
    #[serde(default)]
    pub status: Status,
}

/// Provedores de agente.
pub const PROVIDERS: [(&str, &str, &str); 3] =
    [("claude", "Claude Code", "Anthropic"), ("codex", "Codex", "OpenAI"), ("gemini", "Gemini CLI", "Google")];

/// Modelos do Claude Code oferecidos no seletor (`default` = o das settings).
pub const MODELS: [(&str, &str); 7] = [
    ("default", "api.defaultSettings"),
    ("opus", "Opus"),
    ("opus[1m]", "Opus 1M"),
    ("sonnet", "Sonnet"),
    ("sonnet[1m]", "Sonnet 1M"),
    ("haiku", "Haiku"),
    ("fable", "Fable"),
];

pub const PERMISSION_MODES: [(&str, &str); 6] = [
    ("default", "api.defaultSettings"),
    ("manual", "api.modeManual"),
    ("acceptEdits", "api.modeAcceptEdits"),
    ("auto", "api.modeAuto"),
    ("plan", "api.modePlan"),
    ("bypassPermissions", "api.modeBypass"),
];

pub const EFFORTS: [(&str, &str); 6] =
    [("", "api.effortDefault"), ("low", "Low"), ("medium", "Medium"), ("high", "High"), ("xhigh", "XHigh"), ("max", "Max")];

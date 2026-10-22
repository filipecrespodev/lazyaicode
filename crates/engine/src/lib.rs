//! O motor do LazyAICode: a sessão do app (estado, agentes, terminais,
//! repositórios abertos) e o registro de comandos.
//!
//! **Tudo é um comando.** A UI egui, a CLI, o canal de controle JSON e o
//! servidor MCP chamam [`Engine::run`] com um id (`thread.send`,
//! `git.working.stage-path`…) e parâmetros JSON; nenhum deles mexe no estado
//! por fora. O que o motor quer contar de volta (stream do agente, status das
//! threads, saída dos terminais) sai pelo barramento de eventos
//! ([`Engine::subscribe`]).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod ai;
mod app_cmds;
mod extra_cmds;
mod git_cmds;
mod git_registry;
mod journal;
pub mod registry;
mod session;
pub mod store;
mod system_cmds;
mod thread_cmds;
pub mod workspace;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use lazyaicode_agent::{acp, agent, sessions, terminal};
use lazyaicode_doc::{Status, Thread, ThreadView, Workspace};
use lazyaicode_protocol::{events, CommandInfo, Event};
use parking_lot::Mutex;
use serde_json::{json, Value};
use tokio::sync::broadcast;

pub use journal::JournalEntry;
pub use registry::{Registry, Spec};
pub use store::Store;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("comando desconhecido: {0}")]
    UnknownCommand(String),
    #[error("parâmetros inválidos: {0}")]
    BadParams(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Disabled(String),
    #[error("{0}")]
    Failed(String),
    #[error("{0}")]
    Git(#[from] lazyaicode_git::error::GitCommandError),
    #[error("o comando falhou de forma inesperada: {0}")]
    Panicked(String),
}

impl EngineError {
    /// Código no canal de controle (`ErrorBody.code`).
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownCommand(_) => "unknown_method",
            Self::BadParams(_) => "bad_request",
            Self::NotFound(_) => "not_found",
            Self::Disabled(_) => "disabled",
            Self::Failed(_) => "failed",
            Self::Git(_) => "git",
            Self::Panicked(_) => "internal",
        }
    }

    /// Detalhe estruturado (o erro do git como `{kind, details}`).
    pub fn data(&self) -> Option<Value> {
        match self {
            Self::Git(g) => serde_json::to_value(g).ok(),
            _ => None,
        }
    }
}

impl From<anyhow::Error> for EngineError {
    fn from(e: anyhow::Error) -> Self {
        Self::Failed(e.to_string())
    }
}

impl From<String> for EngineError {
    fn from(e: String) -> Self {
        Self::Failed(e)
    }
}

impl From<&str> for EngineError {
    fn from(e: &str) -> Self {
        Self::Failed(e.to_string())
    }
}

pub type Result<T, E = EngineError> = std::result::Result<T, E>;

#[derive(Debug, Clone, Default)]
pub struct EngineOpts {
    /// Pasta do `state.json`, dos checkouts isolados e das transcrições ACP.
    /// `None` = a do sistema (`LAZYAICODE_DATA_DIR` ou `~/.local/share/lazyaicode`).
    pub data_dir: Option<PathBuf>,
    /// Estado só em memória (testes, `--ephemeral`).
    pub ephemeral: bool,
}

pub(crate) struct Core {
    pub store: Mutex<Store>,
    pub agents: agent::AgentManager,
    /// agentes ACP (Codex, Gemini)
    pub acp: acp::AcpManager,
    /// situação de cada provedor ACP (login, versão), com o instante
    pub provider_status: Mutex<HashMap<String, (Instant, Value)>>,
    pub terminals: terminal::Terminals,
    /// estado de execução por thread
    pub status: Mutex<HashMap<String, Status>>,
    /// pedido de permissão aberto por thread (`thread.pending-permission`)
    pub permissions: Mutex<HashMap<String, Value>>,
    /// threads com um `thread.interrupt` à espera do fim do turno: o
    /// `result` com erro que vem depois é a parada, não uma falha
    pub interrupted: Mutex<std::collections::HashSet<String>>,
    /// o que foi mandado ao CLI e ele ainda não tirou da fila, por thread
    pub queued: Mutex<HashMap<String, session::Queue>>,
    pub data_dir: PathBuf,
    pub sessions: Mutex<sessions::SessionCache>,
    /// resposta da sondagem do CLI por (conta, pasta), com o instante
    pub capabilities: Mutex<HashMap<(PathBuf, PathBuf), (Instant, Value)>>,
    /// `system.avatar`: foto do Gravatar por hash do e-mail (base64), ou
    /// `None` quando não há foto
    pub avatars: Mutex<HashMap<String, Option<String>>>,
    pub git: lazyaicode_git::state::AppState,
    pub bus: broadcast::Sender<Event>,
    pub journal: Mutex<journal::Journal>,
}

/// A sessão do app. Barata de clonar (um `Arc`).
#[derive(Clone)]
pub struct Engine {
    pub(crate) core: Arc<Core>,
}

fn registry() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(|| {
        let mut r = Registry::default();
        system_cmds::register(&mut r);
        app_cmds::register(&mut r);
        thread_cmds::register(&mut r);
        extra_cmds::register(&mut r);
        git_registry::register(&mut r);
        r
    })
}

/// Os comandos do motor (sem os de tela da UI, que a casca acrescenta).
pub fn commands() -> Vec<CommandInfo> {
    registry().infos().cloned().collect()
}

pub fn find_command(id: &str) -> Option<CommandInfo> {
    registry().get(id).map(|s| s.info.clone())
}

impl Engine {
    pub fn new(opts: EngineOpts) -> Self {
        let data_dir = opts.data_dir.unwrap_or_else(store::data_dir);
        let mut store = if opts.ephemeral { Store::memory() } else { Store::load(&data_dir) };
        fix_isolated_bases(&mut store);
        let (bus, _) = broadcast::channel(8192);
        let core = Arc::new(Core {
            store: Mutex::new(store),
            agents: agent::AgentManager::default(),
            acp: acp::AcpManager::default(),
            provider_status: Mutex::new(HashMap::new()),
            terminals: terminal::Terminals::default(),
            status: Mutex::new(HashMap::new()),
            permissions: Mutex::new(HashMap::new()),
            interrupted: Mutex::new(std::collections::HashSet::new()),
            queued: Mutex::new(HashMap::new()),
            data_dir,
            sessions: Mutex::new(sessions::SessionCache::default()),
            capabilities: Mutex::new(HashMap::new()),
            avatars: Mutex::new(HashMap::new()),
            git: lazyaicode_git::state::AppState::default(),
            bus,
            journal: Mutex::new(journal::Journal::default()),
        });
        let engine = Self { core };
        engine.wire_git();
        engine
    }

    /// O cliente git emite eventos e lê preferências por funções globais
    /// (o mesmo código roda no agente WSL, sem motor): aponta-as para cá.
    fn wire_git(&self) {
        let bus = self.core.bus.clone();
        lazyaicode_git::events::set_sink(move |name, payload| {
            let _ = bus.send(Event { event: name.to_string(), payload });
        });
        let weak = Arc::downgrade(&self.core);
        lazyaicode_git::settings::set_provider(move |key| {
            weak.upgrade().and_then(|c| c.store.lock().state.settings.prefs.get(key).cloned())
        });
    }

    /// Executa um comando pelo id. Valida os parâmetros contra a descrição,
    /// registra no journal e transforma um pânico escapado em erro (rede de
    /// segurança; nenhum comando deve entrar em pânico).
    pub async fn run(&self, id: &str, params: Value) -> Result<Value> {
        let spec = registry().get(id).ok_or_else(|| EngineError::UnknownCommand(id.to_string()))?;
        lazyaicode_protocol::check_params(&spec.info.params, &params).map_err(EngineError::BadParams)?;
        check_absolute_paths(&spec.info, &params)?;
        let Some(run) = spec.run.clone() else {
            return Err(EngineError::Disabled(format!("{id} não tem implementação neste motor")));
        };
        let started = Instant::now();
        let fut = run(self.clone(), params.clone());
        // o comando roda numa task própria: um pânico vira JoinError, não derruba quem chamou
        let out = match tokio::spawn(fut).await {
            Ok(r) => r,
            Err(e) => Err(EngineError::Panicked(e.to_string())),
        };
        if !spec.info.read_only {
            self.core.journal.lock().push(id, &params, &out, started.elapsed());
        }
        out
    }

    /// Eventos do motor (agentes, status, terminais, git).
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.core.bus.subscribe()
    }

    pub fn emit(&self, event: &str, payload: Value) {
        let _ = self.core.bus.send(Event { event: event.to_string(), payload });
    }

    pub fn data_dir(&self) -> &std::path::Path {
        &self.core.data_dir
    }

    pub fn git(&self) -> &lazyaicode_git::state::AppState {
        &self.core.git
    }

    pub fn journal(&self) -> Vec<JournalEntry> {
        self.core.journal.lock().entries()
    }

    /// Para todos os agentes e terminais (ao fechar o app).
    pub async fn shutdown(&self) {
        let ids: Vec<String> = self.core.store.lock().state.threads.iter().map(|t| t.id.clone()).collect();
        for id in ids {
            self.stop_agent(&id).await;
        }
        self.core.terminals.kill_all();
    }

    // ---------- ajudantes usados pelos comandos ----------

    /// Conta (diretório Claude) do workspace aberto; usada pelas ações que
    /// não pertencem a uma thread, como a IA de commit do git.
    pub fn active_claude_dir(&self) -> PathBuf {
        let s = self.core.store.lock();
        s.state
            .active_workspace
            .as_deref()
            .and_then(|id| s.state.workspace(id))
            .map(workspace::claude_dir)
            .unwrap_or_else(|| lazyaicode_agent::claude::resolve_active(None))
    }

    /// Conta de uma thread: a sessão só pode continuar no diretório onde
    /// nasceu, mesmo que o workspace troque de conta depois.
    pub(crate) fn thread_claude_dir(&self, t: &Thread) -> PathBuf {
        if let Some(d) = t.claude_dir.as_deref().filter(|d| !d.is_empty()) {
            return PathBuf::from(d);
        }
        let s = self.core.store.lock();
        t.workspace_id
            .as_deref()
            .and_then(|id| s.state.workspace(id))
            .map(workspace::claude_dir)
            .unwrap_or_else(lazyaicode_agent::claude::default_claude_dir)
    }

    pub(crate) async fn stop_agent(&self, id: &str) {
        self.core.agents.stop(id).await;
        self.core.acp.stop(id).await;
    }

    pub(crate) fn workspace(&self, id: &str) -> Result<Workspace> {
        self.core.store.lock().state.workspace(id).cloned().ok_or_else(|| EngineError::NotFound("Workspace não encontrado".into()))
    }

    pub(crate) fn thread(&self, id: &str) -> Result<Thread> {
        self.core.store.lock().state.thread(id).cloned().ok_or_else(|| EngineError::NotFound("Thread não encontrada".into()))
    }

    pub(crate) fn update_thread(&self, id: &str, f: impl FnOnce(&mut Thread)) -> Result<Thread> {
        let mut s = self.core.store.lock();
        let t = s.state.thread_mut(id).ok_or_else(|| EngineError::NotFound("Thread não encontrada".into()))?;
        f(t);
        let out = t.clone();
        s.save()?;
        Ok(out)
    }

    pub(crate) fn status_of(&self, id: &str) -> Status {
        self.core.status.lock().get(id).copied().unwrap_or_default()
    }

    pub(crate) fn set_status(&self, id: &str, status: Status) {
        self.core.status.lock().insert(id.to_string(), status);
        self.emit(events::THREAD_STATUS, json!({ "threadId": id, "status": status }));
    }

    pub(crate) fn view(&self, t: &Thread) -> ThreadView {
        ThreadView { status: self.status_of(&t.id), thread: t.clone() }
    }
}

/// Parâmetros que são caminhos no disco além dos de tipo `path` (o gerador
/// dos comandos git só conhece texto).
const PATH_PARAMS: &[(&str, &str)] = &[("git.repo.open", "path"), ("thread.set-options", "addDirs")];

/// Um caminho relativo seria resolvido a partir da pasta onde o processo foi
/// aberto — um `git.path.commit-all {path: "../x"}` commitava no repositório
/// em volta do app. Todo caminho que entra precisa ser absoluto (`""` segue
/// valendo onde o comando dá sentido a ele, como `claudeDir`).
fn check_absolute_paths(info: &CommandInfo, params: &Value) -> Result<()> {
    let is_path = |name: &str, ty: &lazyaicode_protocol::ParamType| {
        matches!(ty, lazyaicode_protocol::ParamType::Path) || PATH_PARAMS.contains(&(info.id.as_str(), name))
    };
    for p in info.params.iter().filter(|p| is_path(&p.name, &p.ty)) {
        let values: Vec<&str> = match params.get(&p.name) {
            Some(Value::String(s)) => vec![s.as_str()],
            Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
            _ => vec![],
        };
        if let Some(rel) = values.into_iter().find(|v| !v.is_empty() && !is_absolute(v)) {
            return Err(EngineError::BadParams(format!("`{}` precisa ser um caminho absoluto: {rel}", p.name)));
        }
    }
    Ok(())
}

/// Absoluto na plataforma, ou um caminho do WSL (`\\wsl.localhost\…`).
fn is_absolute(p: &str) -> bool {
    std::path::Path::new(p).is_absolute() || p.starts_with("\\\\") || (cfg!(windows) && p.starts_with('/'))
}

/// Roda trabalho bloqueante (git, disco) fora das threads do runtime.
pub(crate) async fn blocking<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> Result<T> {
    match tokio::task::spawn_blocking(f).await {
        Ok(r) => r.map_err(EngineError::from),
        Err(e) => Err(EngineError::Panicked(e.to_string())),
    }
}

#[cfg(test)]
mod tests;

/// Threads isoladas criadas num repositório sem remoto gravavam `HEAD` como
/// base: no checkout isolado, é a branch da própria thread, e a revisão, o
/// "à frente/atrás" e o integrar deixavam de ver o que o agente commitava.
/// A base passa a ser a branch do checkout principal.
fn fix_isolated_bases(store: &mut Store) {
    let st = &mut store.state;
    let mut fixed = false;
    for t in st.threads.iter_mut().filter(|t| t.isolated && t.base_ref.as_deref() == Some("HEAD")) {
        if let Some(p) = st.projects.iter().find(|p| p.id == t.project_id) {
            t.base_ref = Some(lazyaicode_git::thread::default_base(std::path::Path::new(&p.path)));
            fixed = true;
        }
    }
    if fixed {
        let _ = store.save();
    }
}

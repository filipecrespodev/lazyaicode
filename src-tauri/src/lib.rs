// O app desktop inteiro depende do Tauri; sem a feature `gui` a lib fica
// vazia e só o `lazyaicode-agent` (src/bin) é compilado.
#![cfg(feature = "gui")]

mod acp;
mod acp_translate;
mod agent;
mod catalog;
mod claude;
mod git;
pub mod git_client;
mod platform;
mod sessions;
mod store;
mod terminal;
mod transcript;
mod workspace;

use agent::{AgentManager, AgentOut, Sink, SpawnOpts};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use store::{Project, Settings, Store, Thread, Workspace};
use tauri::{AppHandle, Emitter, Manager, State};

type Res<T> = Result<T, String>;

fn e<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

pub struct App {
    store: parking_lot::Mutex<Store>,
    agents: AgentManager,
    /// agentes ACP (Codex, Gemini)
    acp: acp::AcpManager,
    /// situação de cada provedor ACP (login, versão), com o instante
    provider_status: parking_lot::Mutex<HashMap<String, (std::time::Instant, Value)>>,
    terminals: terminal::Terminals,
    /// Estado de execução por thread: idle | running | awaiting | error.
    status: parking_lot::Mutex<HashMap<String, String>>,
    data_dir: PathBuf,
    sessions: parking_lot::Mutex<sessions::SessionCache>,
    /// resposta da sondagem do CLI por (conta, pasta), com o instante
    capabilities: parking_lot::Mutex<HashMap<(PathBuf, PathBuf), (std::time::Instant, Value)>>,
}

impl App {
    /// Conta (diretório Claude) de uma thread: a sessão só pode continuar
    /// no diretório onde nasceu, mesmo que o workspace troque de conta depois.
    fn thread_claude_dir(&self, t: &Thread) -> PathBuf {
        if let Some(d) = t.claude_dir.as_deref().filter(|d| !d.is_empty()) {
            return PathBuf::from(d);
        }
        let s = self.store.lock();
        t.workspace_id
            .as_deref()
            .and_then(|id| s.workspace(id))
            .map(workspace::claude_dir)
            .unwrap_or_else(claude::default_claude_dir)
    }

    /// Conta (diretório Claude) do workspace aberto; usada pelas ações que
    /// não pertencem a uma thread, como a IA de commit do git.
    pub(crate) fn active_claude_dir(&self) -> PathBuf {
        let s = self.store.lock();
        s.state
            .active_workspace
            .as_deref()
            .and_then(|id| s.workspace(id))
            .map(workspace::claude_dir)
            .unwrap_or_else(claude::default_claude_dir)
    }

    fn is_acp(t: &Thread) -> bool {
        t.provider != "claude"
    }

    async fn stop_agent(&self, id: &str) {
        self.agents.stop(id).await;
        self.acp.stop(id).await;
    }

    /// Transcrição das threads ACP (o app grava; não há .jsonl comum entre
    /// os provedores).
    fn acp_transcript(&self, id: &str) -> PathBuf {
        self.data_dir.join("transcripts").join(format!("{id}.jsonl"))
    }

    fn append_transcript(&self, id: &str, msg: &Value) {
        use std::io::Write;
        let path = self.acp_transcript(id);
        if let Some(p) = path.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{msg}");
        }
    }

    fn workspace(&self, id: &str) -> Res<Workspace> {
        self.store.lock().workspace(id).cloned().ok_or_else(|| "Workspace não encontrado".into())
    }

    fn thread(&self, id: &str) -> Res<Thread> {
        self.store.lock().thread(id).cloned().ok_or_else(|| "Thread não encontrada".into())
    }

    fn update_thread(&self, id: &str, f: impl FnOnce(&mut Thread)) -> Res<Thread> {
        let mut s = self.store.lock();
        let t = s.thread_mut(id).ok_or("Thread não encontrada")?;
        f(t);
        let out = t.clone();
        s.save().map_err(e)?;
        Ok(out)
    }

    fn set_status(&self, app: &AppHandle, id: &str, status: &str) {
        self.status.lock().insert(id.to_string(), status.to_string());
        let _ = app.emit("thread-status", json!({ "threadId": id, "status": status }));
    }

    fn view(&self, t: &Thread) -> ThreadView {
        ThreadView {
            status: self.status.lock().get(&t.id).cloned().unwrap_or_else(|| "idle".into()),
            thread: t.clone(),
        }
    }
}

#[derive(Serialize, Clone)]
struct ThreadView {
    #[serde(flatten)]
    thread: Thread,
    status: String,
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(e)?.map_err(e)
}

// ---------- configurações e contas ----------

#[tauri::command]
fn get_settings(app: State<'_, Arc<App>>) -> Value {
    let s = app.store.lock();
    json!({
        "settings": s.state.settings,
        "claudeBinary": claude::find_claude_binary(),
        "dataDir": app.data_dir,
        "defaultClaudeDir": claude::default_claude_dir(),
        // conta sugerida para um workspace novo: a do ambiente (CLAUDE_CONFIG_DIR)
        "suggestedClaudeDir": claude::resolve_active(None),
    })
}

#[tauri::command]
fn save_settings(app: State<'_, Arc<App>>, settings: Settings) -> Res<()> {
    let mut s = app.store.lock();
    s.state.settings = settings;
    s.save().map_err(e)
}

#[tauri::command]
fn list_claude_dirs() -> Vec<claude::ClaudeDirInfo> {
    claude::discover_dirs(&claude::home())
}

/// Login de uma conta (diretório Claude); `None` = `~/.claude`.
#[tauri::command]
async fn auth_status(claude_dir: Option<String>) -> Res<Value> {
    let dir = claude_dir.filter(|d| !d.is_empty()).map(PathBuf::from).unwrap_or_else(claude::default_claude_dir);
    blocking(move || Ok(claude::auth_status(&dir))).await
}

// ---------- workspaces ----------

#[tauri::command]
fn list_workspaces(app: State<'_, Arc<App>>) -> Value {
    let s = app.store.lock();
    json!({ "workspaces": s.state.workspaces, "active": s.state.active_workspace })
}

#[tauri::command]
fn create_workspace(
    app: State<'_, Arc<App>>,
    path: String,
    name: Option<String>,
    claude_dir: Option<String>,
) -> Res<Workspace> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err(format!("Pasta não encontrada: {path}"));
    }
    let w = Workspace {
        id: uuid::Uuid::new_v4().to_string(),
        name: name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(path.clone())),
        path,
        claude_dir: claude_dir.filter(|d| !d.is_empty() && PathBuf::from(d) != claude::default_claude_dir()),
        ignored_paths: vec![],
        created_at: store::now(),
    };
    let mut s = app.store.lock();
    s.state.workspaces.push(w.clone());
    s.state.active_workspace = Some(w.id.clone());
    s.save().map_err(e)?;
    Ok(w)
}

#[tauri::command]
async fn update_workspace(
    app: State<'_, Arc<App>>,
    id: String,
    name: Option<String>,
    claude_dir: Option<Option<String>>,
    ignored_paths: Option<Vec<String>>,
) -> Res<Workspace> {
    let (w, account_changed) = {
        let mut s = app.store.lock();
        let w = s.state.workspaces.iter_mut().find(|w| w.id == id).ok_or("Workspace não encontrado")?;
        if let Some(n) = name.filter(|n| !n.trim().is_empty()) {
            w.name = n.trim().to_string();
        }
        let mut changed = false;
        if let Some(d) = claude_dir {
            let d = d.filter(|d| !d.is_empty() && PathBuf::from(d) != claude::default_claude_dir());
            changed = d != w.claude_dir;
            w.claude_dir = d;
        }
        if let Some(i) = ignored_paths {
            w.ignored_paths = i;
        }
        let out = w.clone();
        s.save().map_err(e)?;
        (out, changed)
    };
    if account_changed {
        // threads abertas continuam na conta delas; só encerramos os processos
        // para que nada siga rodando com a conta antiga sem o usuário ver
        let ids: Vec<String> = app.store.lock().state.threads.iter()
            .filter(|t| t.workspace_id.as_deref() == Some(id.as_str()))
            .map(|t| t.id.clone())
            .collect();
        for tid in ids {
            app.stop_agent(&tid).await;
        }
    }
    Ok(w)
}

#[tauri::command]
async fn remove_workspace(app: State<'_, Arc<App>>, id: String) -> Res<()> {
    let ids: Vec<String> = app.store.lock().state.threads.iter()
        .filter(|t| t.workspace_id.as_deref() == Some(id.as_str()))
        .map(|t| t.id.clone())
        .collect();
    for tid in &ids {
        app.stop_agent(tid).await;
    }
    let mut s = app.store.lock();
    // threads isoladas guardam trabalho numa branch própria: não somem com o
    // workspace — ficam sem dono e voltam se a pasta for adicionada de novo
    for t in s.state.threads.iter_mut().filter(|t| t.workspace_id.as_deref() == Some(id.as_str())) {
        t.workspace_id = None;
    }
    s.state.workspaces.retain(|w| w.id != id);
    if s.state.active_workspace.as_deref() == Some(id.as_str()) {
        s.state.active_workspace = s.state.workspaces.first().map(|w| w.id.clone());
    }
    s.save().map_err(e)
}

#[tauri::command]
fn set_active_workspace(app: State<'_, Arc<App>>, id: String) -> Res<()> {
    let mut s = app.store.lock();
    s.state.active_workspace = Some(id);
    s.save().map_err(e)
}

/// Todos os repositórios da pasta do workspace, inclusive os ignorados
/// (a tela de edição mostra a lista completa para marcar/desmarcar).
#[tauri::command]
async fn workspace_repos(
    app: State<'_, Arc<App>>,
    git: State<'_, git_client::state::AppState>,
    id: String,
) -> Res<Vec<workspace::RepoEntry>> {
    let w = app.workspace(&id)?;
    Ok(workspace::scan_repos_routed(&git, &w.path).await)
}

// ---------- skills, agentes e MCP ----------

const CAPABILITIES_TTL: std::time::Duration = std::time::Duration::from_secs(120);

/// O que o Claude Code carrega numa pasta com a conta do workspace: skills e
/// comandos (com o arquivo de origem), agentes, MCPs com status e modelos.
/// `path` = projeto do workspace (escopo projeto) ou nada (só global).
#[tauri::command]
async fn get_capabilities(
    app: State<'_, Arc<App>>,
    workspace_id: String,
    path: Option<String>,
    force: Option<bool>,
) -> Res<Value> {
    let w = app.workspace(&workspace_id)?;
    let dir = workspace::claude_dir(&w);
    let cwd = PathBuf::from(path.clone().unwrap_or_else(|| w.path.clone()));
    let key = (dir.clone(), cwd.clone());
    let cached = app.capabilities.lock().get(&key).cloned();
    let probe = match cached {
        Some((at, v)) if !force.unwrap_or(false) && at.elapsed() < CAPABILITIES_TTL => v,
        _ => {
            let bin = claude::find_claude_binary().ok_or("Claude CLI não encontrado")?;
            let v = agent::probe(&bin, &dir, &cwd).await.map_err(e)?;
            app.capabilities.lock().insert(key, (std::time::Instant::now(), v.clone()));
            v
        }
    };
    let project = path.map(PathBuf::from);
    let (d, p) = (dir.clone(), project.clone());
    let fs = blocking(move || Ok(catalog::scan(&d, p.as_deref()))).await?;
    let (commands, agents) = catalog::merge(&probe["init"], fs);
    Ok(json!({
        "commands": commands,
        "agents": agents,
        "mcp": catalog::redact_mcp(&probe["mcp"]),
        "models": probe["init"]["models"],
        "account": probe["init"]["account"],
        "version": probe["init"]["claude_code_version"],
        "claudeDir": dir,
        "cwd": cwd,
    }))
}

/// MCP da sessão de uma thread em execução: status, ligar/desligar (vale só
/// para a sessão, como o `/mcp` do Claude Code) e reconectar.
#[tauri::command]
async fn thread_mcp(
    app: State<'_, Arc<App>>,
    id: String,
    action: String,
    server: Option<String>,
    enabled: Option<bool>,
) -> Res<Value> {
    if !app.agents.is_running(&id) {
        return Err("A thread não está com o agente aberto. Envie uma mensagem para iniciar a sessão.".into());
    }
    let name = server.unwrap_or_default();
    match action.as_str() {
        "toggle" => {
            app.agents
                .request(&id, json!({ "subtype": "mcp_toggle", "serverName": name, "enabled": enabled.unwrap_or(true) }))
                .await
                .map_err(e)?;
        }
        "reconnect" => {
            app.agents.request(&id, json!({ "subtype": "mcp_reconnect", "serverName": name })).await.map_err(e)?;
        }
        _ => {}
    }
    let st = app.agents.request(&id, json!({ "subtype": "mcp_status" })).await.map_err(e)?;
    Ok(catalog::redact_mcp(&st["mcpServers"]))
}

/// Move um skill (a pasta inteira) ou um agente/comando para a lixeira.
/// Só itens da conta ou de `.claude/` de um projeto — nunca de plugins, que
/// o instalador recriaria.
#[tauri::command]
async fn trash_item(app: State<'_, Arc<App>>, workspace_id: String, path: String) -> Res<()> {
    let w = app.workspace(&workspace_id)?;
    let dir = workspace::claude_dir(&w);
    let p = PathBuf::from(&path);
    let in_account = ["skills", "agents", "commands"].iter().any(|d| p.starts_with(dir.join(d)));
    let in_project = p.components().any(|c| c.as_os_str() == ".claude" || c.as_os_str() == ".agents")
        && workspace::contains(&w, &p);
    if p.starts_with(dir.join("plugins")) || !(in_account || in_project) || !p.is_file() {
        return Err("Este item não pode ser apagado pelo app".into());
    }
    let target = if p.file_name().is_some_and(|n| n == "SKILL.md") { p.parent().unwrap().to_path_buf() } else { p };
    trash::delete(&target).map_err(|e| format!("Não foi possível mover para a lixeira: {e}"))?;
    app.capabilities.lock().clear();
    Ok(())
}

// ---------- projetos ----------

/// Projeto para uma pasta: a raiz do repositório git, ou a própria pasta
/// quando não é git (o chat funciona; diff e workspace isolado não).
/// `plain`: a varredura já sabe que a pasta não tem `.git` — não sobe até um
/// repositório pai (o workspace pode ser ele mesmo um repo).
fn ensure_project(store: &mut Store, cwd: &Path, plain: bool) -> String {
    // Repo dentro de uma distro WSL: a varredura (feita lá pelo agente) já
    // deu a raiz; rodar `git rev-parse` sobre o 9P seria lento e devolveria
    // `//wsl.localhost/...` em vez do caminho UNC.
    let (root, is_git) = if plain {
        (cwd.to_path_buf(), false)
    } else if git_client::wslpath::parse_unc(&cwd.to_string_lossy()).is_some() {
        (cwd.to_path_buf(), cwd.join(".git").exists())
    } else {
        match git::repo_root(cwd) {
            Ok(r) => (r, true),
            Err(_) => (cwd.to_path_buf(), false),
        }
    };
    let root_s = root.to_string_lossy().to_string();
    if let Some(p) = store.state.projects.iter().find(|p| p.path == root_s) {
        return p.id.clone();
    }
    let p = Project {
        id: uuid::Uuid::new_v4().to_string(),
        name: root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(root_s.clone()),
        path: root_s,
        created_at: store::now(),
        is_git,
    };
    let id = p.id.clone();
    store.state.projects.push(p);
    id
}

/// Repositórios do workspace (menos os ignorados) mais qualquer projeto em
/// que uma thread do workspace esteja, mesmo fora da varredura.
#[tauri::command]
async fn list_projects(
    app: State<'_, Arc<App>>,
    git: State<'_, git_client::state::AppState>,
    workspace_id: String,
) -> Res<Vec<Project>> {
    let w = app.workspace(&workspace_id)?;
    let repos = workspace::scan_repos_routed(&git, &w.path).await;
    let mut s = app.store.lock();
    let mut ids: Vec<String> = Vec::new();
    let mut git_dirs: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    for r in repos.iter().filter(|r| !workspace::is_ignored(&w, Path::new(&r.path))) {
        let id = ensure_project(&mut s, Path::new(&r.path), !r.is_repo);
        git_dirs.insert(id.clone(), r.is_repo);
        ids.push(id);
    }
    let from_threads: Vec<String> = s.state.threads.iter()
        .filter(|t| t.workspace_id.as_deref() == Some(w.id.as_str()))
        .map(|t| t.project_id.clone())
        .collect();
    for id in from_threads {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    // projetos de threads fora da varredura: confere o `.git` direto
    for p in s.state.projects.iter_mut().filter(|p| ids.contains(&p.id)) {
        p.is_git = git_dirs.get(&p.id).copied().unwrap_or_else(|| Path::new(&p.path).join(".git").exists());
    }
    s.save().map_err(e)?;
    Ok(ids.iter().filter_map(|id| s.project(id).cloned()).collect())
}

// ---------- threads ----------

#[tauri::command]
fn list_threads(app: State<'_, Arc<App>>, workspace_id: String) -> Vec<ThreadView> {
    let threads: Vec<Thread> = app.store.lock().state.threads.iter()
        .filter(|t| t.workspace_id.as_deref() == Some(workspace_id.as_str()))
        .cloned()
        .collect();
    threads.iter().map(|t| app.view(t)).collect()
}

/// Traz as sessões do Claude Code da conta do workspace, que rodaram dentro
/// da pasta dele, como threads (no checkout onde rodaram). Atualiza a
/// atividade das já conhecidas. Devolve quantas threads novas surgiram.
#[tauri::command]
async fn sync_sessions(app: State<'_, Arc<App>>, workspace_id: String) -> Res<usize> {
    let w = app.workspace(&workspace_id)?;
    let dir = workspace::claude_dir(&w);
    let a = app.inner().clone();
    let d = dir.clone();
    let found = blocking(move || Ok(a.sessions.lock().scan(&d))).await?;
    let dir_s = dir.to_string_lossy().to_string();
    let worktrees = app.data_dir.join("worktrees");
    let status = app.status.lock().clone();
    let mut s = app.store.lock();
    let mut added = 0;
    for info in found {
        let cwd = PathBuf::from(&info.cwd);
        if !workspace::owns_session_cwd(&w, &cwd) || cwd.starts_with(&worktrees) || !cwd.is_dir() {
            continue;
        }
        let existing = s.state.threads.iter_mut().find(|t| {
            t.session_id.as_deref() == Some(info.session_id.as_str()) || t.past_sessions.contains(&info.session_id)
        });
        if let Some(t) = existing {
            // threads de antes dos workspaces (ou de um workspace removido)
            // são adotadas por quem tem a mesma conta e contém a pasta
            if t.workspace_id.is_none() && t.claude_dir.as_deref().map_or(true, |c| c == dir_s) {
                t.workspace_id = Some(w.id.clone());
                t.claude_dir = Some(dir_s.clone());
            }
            let busy = status.get(&t.id).is_some_and(|st| st != "idle");
            if !busy && info.updated_at > t.updated_at && t.session_id.as_deref() == Some(info.session_id.as_str()) {
                t.updated_at = info.updated_at;
            }
            continue;
        }
        let project_id = ensure_project(&mut s, &cwd, false);
        let settings = s.state.settings.clone();
        s.state.threads.push(Thread {
            id: uuid::Uuid::new_v4().to_string(),
            project_id,
            title: info.title,
            created_at: info.created_at,
            updated_at: info.updated_at,
            cwd: info.cwd,
            isolated: false,
            branch: None,
            base_ref: None,
            session_id: Some(info.session_id),
            model: settings.default_model,
            permission_mode: settings.default_permission_mode,
            archived: false,
            bookmarked: false,
            seen: HashMap::new(),
            turn_tree: None,
            unread: false,
            claude_dir: Some(dir_s.clone()),
            past_sessions: vec![],
            workspace_id: Some(w.id.clone()),
            effort: None,
            agent: None,
            add_dirs: vec![],
            provider: "claude".into(),
            skip_permissions: false,
        });
        added += 1;
    }
    // threads criadas no app antes dos workspaces, com checkout isolado
    let ws_dir = dir_s.clone();
    let orphans: Vec<(usize, String)> = s.state.threads.iter().enumerate()
        .filter(|(_, t)| t.workspace_id.is_none() && t.claude_dir.as_deref().map_or(true, |c| c == ws_dir))
        .map(|(i, t)| (i, t.project_id.clone()))
        .collect();
    for (i, pid) in orphans {
        if s.project(&pid).is_some_and(|p| workspace::owns_session_cwd(&w, Path::new(&p.path))) {
            s.state.threads[i].workspace_id = Some(w.id.clone());
            s.state.threads[i].claude_dir = Some(dir_s.clone());
        }
    }
    s.save().map_err(e)?;
    Ok(added)
}

#[tauri::command]
async fn create_thread(
    app: State<'_, Arc<App>>,
    workspace_id: String,
    project_id: String,
    isolated: bool,
    model: Option<String>,
    permission_mode: Option<String>,
    provider: Option<String>,
    skip_permissions: Option<bool>,
) -> Res<ThreadView> {
    let w = app.workspace(&workspace_id)?;
    let provider = provider.filter(|p| !p.is_empty()).unwrap_or_else(|| "claude".into());
    if provider != "claude" && !acp::PROVIDERS.contains(&provider.as_str()) {
        return Err(format!("Provedor desconhecido: {provider}"));
    }
    let (project, settings) = {
        let s = app.store.lock();
        (s.project(&project_id).cloned().ok_or("Projeto não encontrado")?, s.state.settings.clone())
    };
    // sem `.git` não há worktree: a thread roda na própria pasta
    let isolated = isolated && Path::new(&project.path).join(".git").exists();
    let id = uuid::Uuid::new_v4().to_string();
    let short = id[..8].to_string();
    let (cwd, branch, base_ref) = if isolated {
        let dest = app.data_dir.join("worktrees").join(&project.id).join(&short);
        let repo = PathBuf::from(&project.path);
        let iso = blocking(move || git::create_isolated(&repo, &dest, &short, true)).await?;
        (iso.path.to_string_lossy().to_string(), Some(iso.branch), Some(iso.base_ref))
    } else {
        (project.path.clone(), None, None)
    };
    let now = store::now();
    let t = Thread {
        id,
        project_id,
        title: "Nova thread".into(),
        created_at: now,
        updated_at: now,
        cwd,
        isolated,
        branch,
        base_ref,
        session_id: None,
        // ACP: vazio = padrão do próprio agente
        model: model.unwrap_or(if provider == "claude" { settings.default_model } else { String::new() }),
        permission_mode: permission_mode.unwrap_or(if provider == "claude" { settings.default_permission_mode } else { String::new() }),
        archived: false,
        bookmarked: false,
        seen: HashMap::new(),
        turn_tree: None,
        unread: false,
        claude_dir: Some(workspace::claude_dir(&w).to_string_lossy().to_string()),
        past_sessions: vec![],
        workspace_id: Some(w.id),
        effort: None,
        agent: None,
        add_dirs: vec![],
        skip_permissions: provider == "claude" && skip_permissions.unwrap_or(settings.default_skip_permissions),
        provider,
    };
    {
        let mut s = app.store.lock();
        s.state.threads.push(t.clone());
        s.save().map_err(e)?;
    }
    Ok(app.view(&t))
}

#[tauri::command]
fn update_thread_meta(
    app: State<'_, Arc<App>>,
    id: String,
    title: Option<String>,
    bookmarked: Option<bool>,
    unread: Option<bool>,
) -> Res<ThreadView> {
    let t = app.update_thread(&id, |t| {
        if let Some(v) = title {
            t.title = v;
        }
        if let Some(v) = bookmarked {
            t.bookmarked = v;
        }
        if let Some(v) = unread {
            t.unread = v;
        }
    })?;
    Ok(app.view(&t))
}

#[tauri::command]
async fn archive_thread(app: State<'_, Arc<App>>, id: String, archived: bool) -> Res<ThreadView> {
    let t = app.thread(&id)?;
    app.stop_agent(&id).await;
    if t.isolated {
        let repo = PathBuf::from(&app.store.lock().project(&t.project_id).ok_or("Projeto não encontrado")?.path);
        let cwd = PathBuf::from(&t.cwd);
        let branch = t.branch.clone().unwrap_or_default();
        if archived {
            // a branch fica: reabrir recria o checkout a partir dela
            blocking(move || git::remove_isolated(&repo, &cwd)).await?;
        } else if !cwd.exists() {
            blocking(move || {
                git::git(&repo, &["worktree", "add", &cwd.to_string_lossy(), &branch]).map(|_| ())
            })
            .await?;
        }
    }
    let t = app.update_thread(&id, |t| t.archived = archived)?;
    Ok(app.view(&t))
}

#[tauri::command]
async fn delete_thread(app: State<'_, Arc<App>>, id: String) -> Res<()> {
    let t = app.thread(&id)?;
    app.stop_agent(&id).await;
    if t.isolated {
        let repo = PathBuf::from(&app.store.lock().project(&t.project_id).ok_or("Projeto não encontrado")?.path);
        let cwd = PathBuf::from(&t.cwd);
        let branch = t.branch.clone();
        blocking(move || {
            git::remove_isolated(&repo, &cwd)?;
            if let Some(b) = branch {
                let _ = git::git(&repo, &["branch", "-D", &b]);
            }
            Ok(())
        })
        .await?;
    }
    let mut s = app.store.lock();
    s.state.threads.retain(|x| x.id != id);
    s.save().map_err(e)
}

#[tauri::command]
fn load_transcript(app: State<'_, Arc<App>>, id: String) -> Res<Vec<Value>> {
    let t = app.thread(&id)?;
    if App::is_acp(&t) {
        return Ok(transcript::load(&app.acp_transcript(&id)));
    }
    let Some(sid) = t.session_id.clone() else { return Ok(vec![]) };
    Ok(transcript::load(&claude::session_file(&app.thread_claude_dir(&t), &t.cwd, &sid)))
}

fn make_sink(handle: AppHandle) -> Sink {
    Arc::new(move |tid: &str, out: AgentOut| {
        let app = handle.state::<Arc<App>>();
        match out {
            AgentOut::Message(msg) => {
                if app.acp.is_running(tid)
                    && matches!(msg["type"].as_str(), Some("assistant" | "user" | "result" | "lazy_notice"))
                {
                    app.append_transcript(tid, &msg);
                }
                match (msg["type"].as_str(), msg["subtype"].as_str()) {
                    (Some("system"), Some("init")) => {
                        if let Some(sid) = msg["session_id"].as_str() {
                            let sid = sid.to_string();
                            let _ = app.update_thread(tid, |t| {
                                if let Some(old) = t.session_id.take() {
                                    if old != sid && !t.past_sessions.contains(&old) {
                                        t.past_sessions.push(old);
                                    }
                                }
                                t.session_id = Some(sid);
                            });
                        }
                    }
                    (Some("result"), _) => {
                        let _ = app.update_thread(tid, |t| {
                            t.unread = true;
                            t.updated_at = store::now();
                        });
                        let failed = msg["is_error"].as_bool().unwrap_or(false);
                        app.set_status(&handle, tid, if failed { "error" } else { "idle" });
                    }
                    (Some("control_request"), _)
                        if msg["request"]["subtype"] == "can_use_tool" =>
                    {
                        app.set_status(&handle, tid, "awaiting");
                    }
                    _ => {}
                }
                let _ = handle.emit("agent-event", json!({ "threadId": tid, "msg": msg }));
            }
            AgentOut::Exit { code, stderr } => {
                let running = app.status.lock().get(tid).map(|s| s != "idle").unwrap_or(false);
                let failed = code.map(|c| c != 0).unwrap_or(false) && running;
                app.set_status(&handle, tid, if failed { "error" } else { "idle" });
                let _ = handle.emit(
                    "agent-exit",
                    json!({ "threadId": tid, "code": code, "stderr": if failed { stderr } else { String::new() } }),
                );
            }
        }
    })
}

/// Bytes de uma imagem do disco, para anexar no chat quando o WebView entrega
/// só o caminho (arrastar ou colar um arquivo no Linux). Acima de 5 MB nem lê:
/// o Claude recusaria.
#[tauri::command]
async fn read_image_file(path: String) -> Res<tauri::ipc::Response> {
    blocking(move || {
        let len = std::fs::metadata(&path)?.len();
        if len > 5 * 1024 * 1024 {
            anyhow::bail!("too_large");
        }
        Ok(tauri::ipc::Response::new(std::fs::read(&path)?))
    })
    .await
}

#[tauri::command]
async fn send_message(
    handle: AppHandle,
    app: State<'_, Arc<App>>,
    id: String,
    text: String,
    images: Option<Vec<agent::ImageInput>>,
) -> Res<()> {
    let images = images.unwrap_or_default();
    if text.trim().is_empty() && images.is_empty() {
        return Err("Mensagem vazia".into());
    }
    if let Some(i) = images.iter().find(|i| !agent::IMAGE_MIMES.contains(&i.mime.as_str())) {
        return Err(format!("Formato de imagem não suportado: {} (use PNG, JPEG, GIF ou WebP)", i.mime));
    }
    let t = app.thread(&id)?;
    if t.archived {
        return Err("Thread arquivada".into());
    }
    let cwd = PathBuf::from(&t.cwd);
    if !cwd.is_dir() {
        return Err(format!("A pasta da thread não existe: {}", t.cwd));
    }
    // foto da pasta no início do turno para o diff "Último turno"
    let snap_cwd = cwd.clone();
    let turn_tree = blocking(move || git::snapshot_tree(&snap_cwd)).await.ok();
    let title = if t.title == "Nova thread" {
        let first: String = text.lines().next().unwrap_or("").chars().take(60).collect();
        Some(if first.trim().is_empty() { "Imagem".to_string() } else { first })
    } else {
        None
    };
    let updated = app.update_thread(&id, |t| {
        if turn_tree.is_some() {
            t.turn_tree = turn_tree;
        }
        if let Some(title) = title {
            t.title = title;
        }
        t.updated_at = store::now();
        t.unread = false;
    })?;
    let _ = handle.emit("thread-updated", app.view(&updated));

    if App::is_acp(&t) {
        let sink = make_sink(handle.clone());
        app.append_transcript(&id, &json!({ "type": "user", "message": { "role": "user", "content": agent::user_content(&text, &images) } }));
        app.set_status(&handle, &id, "running");
        let ensured = app
            .acp
            .ensure(&id, acp::AcpOpts {
                provider: t.provider.clone(),
                cwd,
                session_id: t.session_id.clone(),
                model: Some(t.model.clone()),
                mode: Some(t.permission_mode.clone()),
            }, sink.clone())
            .await;
        if let Err(err) = ensured {
            app.set_status(&handle, &id, "error");
            return Err(acp_hint(&t.provider, &err.to_string()));
        }
        return app.acp.prompt(&id, &text, &images, sink).await.map_err(|err| {
            app.set_status(&handle, &id, "idle");
            e(err)
        });
    }

    let bin = claude::find_claude_binary()
        .ok_or("Claude CLI não encontrado. Instale o Claude Code ou defina LAZYAICODE_CLAUDE_BIN.")?;
    app.agents
        .ensure(
            &id,
            SpawnOpts {
                bin,
                claude_dir: app.thread_claude_dir(&t),
                cwd,
                model: t.model.clone(),
                permission_mode: t.permission_mode.clone(),
                resume: t.session_id.clone(),
                effort: t.effort.clone(),
                agent: t.agent.clone(),
                add_dirs: t.add_dirs.clone(),
                skip_permissions: t.skip_permissions,
                ..Default::default()
            },
            make_sink(handle.clone()),
        )
        .await
        .map_err(e)?;
    app.set_status(&handle, &id, "running");
    app.agents.send_user(&id, &text, &images).await.map_err(e)
}

#[tauri::command]
async fn interrupt(app: State<'_, Arc<App>>, id: String) -> Res<()> {
    if app.acp.is_running(&id) {
        return app.acp.cancel(&id).await.map_err(e);
    }
    if app.agents.is_running(&id) {
        app.agents.control(&id, json!({ "subtype": "interrupt" })).await.map_err(e)?;
    }
    Ok(())
}

#[tauri::command]
async fn respond_permission(
    handle: AppHandle,
    app: State<'_, Arc<App>>,
    id: String,
    request_id: String,
    allow: bool,
    input: Value,
    message: Option<String>,
    tool_use_id: Option<String>,
    updated_permissions: Option<Vec<Value>>,
) -> Res<()> {
    if app.acp.is_running(&id) {
        app.acp.respond_permission(&id, &request_id, allow, updated_permissions.is_some()).await.map_err(e)?;
        app.set_status(&handle, &id, "running");
        return Ok(());
    }
    app.agents
        .respond_permission(&id, &request_id, allow, input, message, tool_use_id, updated_permissions)
        .await
        .map_err(e)?;
    app.set_status(&handle, &id, "running");
    Ok(())
}

#[tauri::command]
async fn set_thread_model(app: State<'_, Arc<App>>, id: String, model: String) -> Res<ThreadView> {
    let t = app.update_thread(&id, |t| t.model = model.clone())?;
    if app.acp.is_running(&id) && !model.is_empty() {
        app.acp.set_option(&id, "model", &model).await.map_err(e)?;
    }
    if app.agents.is_running(&id) {
        let m = if model == "default" { Value::Null } else { json!(model) };
        let _ = app.agents.control(&id, json!({ "subtype": "set_model", "model": m })).await;
    }
    Ok(app.view(&t))
}

#[tauri::command]
async fn set_thread_permission_mode(app: State<'_, Arc<App>>, id: String, mode: String) -> Res<ThreadView> {
    let t = app.update_thread(&id, |t| t.permission_mode = mode.clone())?;
    if app.acp.is_running(&id) && !mode.is_empty() {
        app.acp.set_option(&id, "mode", &mode).await.map_err(e)?;
    }
    if app.agents.is_running(&id) {
        if mode == "default" {
            // "padrão das settings" não tem equivalente em tempo de execução:
            // o próximo processo nasce sem a flag
            app.agents.stop(&id).await;
        } else {
            let _ = app.agents.control(&id, json!({ "subtype": "set_permission_mode", "mode": mode })).await;
        }
    }
    Ok(app.view(&t))
}

/// Opções da sessão que só valem ao abrir o processo (esforço, agente,
/// pastas extras). Se o agente estiver aberto, é encerrado: o próximo turno
/// retoma a mesma conversa (`--resume`) já com as opções novas.
#[tauri::command]
async fn set_thread_options(
    app: State<'_, Arc<App>>,
    id: String,
    effort: Option<Option<String>>,
    agent: Option<Option<String>>,
    add_dirs: Option<Vec<String>>,
    skip_permissions: Option<bool>,
) -> Res<ThreadView> {
    if let Some(dirs) = &add_dirs {
        if let Some(bad) = dirs.iter().find(|d| !Path::new(d).is_dir()) {
            return Err(format!("Pasta não encontrada: {bad}"));
        }
    }
    let t = app.update_thread(&id, |t| {
        if let Some(v) = effort {
            t.effort = v.filter(|s| !s.is_empty());
        }
        if let Some(v) = agent {
            t.agent = v.filter(|s| !s.is_empty());
        }
        if let Some(v) = add_dirs {
            t.add_dirs = v;
        }
        if let Some(v) = skip_permissions {
            t.skip_permissions = v && t.provider == "claude";
        }
    })?;
    let busy = app.status.lock().get(&id).is_some_and(|s| s != "idle");
    if app.agents.is_running(&id) && !busy {
        app.agents.stop(&id).await;
    }
    Ok(app.view(&t))
}

/// Mensagem de erro com o próximo passo quando um agente ACP não abre
/// (quase sempre: falta login).
fn acp_hint(provider: &str, err: &str) -> String {
    let low = err.to_lowercase();
    let auth = ["auth", "api key", "login", "credential", "unauthorized"].iter().any(|k| low.contains(k));
    match (provider, auth) {
        ("codex", true) => format!("{err}\n\nFaça login no Codex: Configurações → Provedores → OpenAI → Entrar com ChatGPT, ou rode `codex login` no terminal (ou defina OPENAI_API_KEY)."),
        ("gemini", true) => format!("{err}\n\nFaça login no Gemini: Configurações → Provedores → Google → Entrar com Google, ou rode `gemini` no terminal e escolha o login (ou defina GEMINI_API_KEY)."),
        _ => err.to_string(),
    }
}

/// Troca o provedor de uma thread que ainda não conversou.
#[tauri::command]
async fn set_thread_provider(app: State<'_, Arc<App>>, id: String, provider: String, model: Option<String>) -> Res<ThreadView> {
    if provider != "claude" && !acp::PROVIDERS.contains(&provider.as_str()) {
        return Err(format!("Provedor desconhecido: {provider}"));
    }
    let t = app.thread(&id)?;
    if t.session_id.is_some() {
        return Err("Esta thread já tem conversa; abra uma thread nova para usar outro provedor".into());
    }
    app.stop_agent(&id).await;
    let defaults = app.store.lock().state.settings.clone();
    let t = app.update_thread(&id, |t| {
        let claude = provider == "claude";
        t.provider = provider.clone();
        t.model = model.unwrap_or(if claude { defaults.default_model.clone() } else { String::new() });
        t.permission_mode = if claude { defaults.default_permission_mode.clone() } else { String::new() };
        if !claude {
            t.effort = None;
            t.agent = None;
            t.add_dirs.clear();
            t.skip_permissions = false;
        }
    })?;
    Ok(app.view(&t))
}

/// Situação dos provedores OpenAI (Codex) e Google (Gemini) na máquina.
#[tauri::command]
async fn provider_status(app: State<'_, Arc<App>>, provider: String, force: Option<bool>) -> Res<Value> {
    if !acp::PROVIDERS.contains(&provider.as_str()) {
        return Err(format!("Provedor desconhecido: {provider}"));
    }
    if !force.unwrap_or(false) {
        if let Some((at, v)) = app.provider_status.lock().get(&provider) {
            if at.elapsed() < std::time::Duration::from_secs(300) {
                return Ok(v.clone());
            }
        }
    }
    let dir = std::env::temp_dir().join("lazyaicode-provider-check");
    let _ = std::fs::create_dir_all(&dir);
    let v = acp::status(&provider, &dir).await;
    app.provider_status.lock().insert(provider, (std::time::Instant::now(), v.clone()));
    Ok(v)
}

#[tauri::command]
async fn provider_auth_methods(provider: String) -> Res<Value> {
    let dir = std::env::temp_dir();
    acp::auth_methods(&provider, &dir).await.map_err(e)
}

/// Login pelo próprio agente (abre o navegador).
#[tauri::command]
async fn provider_login(app: State<'_, Arc<App>>, provider: String, method: String) -> Res<()> {
    let dir = std::env::temp_dir();
    acp::login(&provider, &method, &dir).await.map_err(e)?;
    app.provider_status.lock().remove(&provider);
    Ok(())
}

/// Linhas adicionadas/removidas desde a base da thread, para o rodapé.
#[tauri::command]
async fn diff_stats(app: State<'_, Arc<App>>, id: String) -> Res<Value> {
    let t = app.thread(&id)?;
    blocking(move || {
        let cwd = PathBuf::from(&t.cwd);
        let (base, _) = git::resolve_base(&cwd, "upstream", t.base_ref.as_deref(), None)?;
        let tree = git::snapshot_tree(&cwd)?;
        let out = git::git(&cwd, &["diff", "--numstat", &base, &tree])?;
        let (mut add, mut del, mut files) = (0u64, 0u64, 0u64);
        for l in out.lines() {
            let mut it = l.split_whitespace();
            add += it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
            del += it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
            files += 1;
        }
        let branch = git::git(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"]).map(|b| b.trim().to_string()).ok();
        Ok(json!({ "additions": add, "deletions": del, "files": files, "branch": branch }))
    })
    .await
}

/// Nome e e-mail do git (global) para o avatar das mensagens do usuário.
#[tauri::command]
async fn user_profile() -> Res<Value> {
    blocking(|| {
        let get = |k: &str| {
            platform::command("git")
                .args(["config", "--global", k])
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty())
        };
        let name = get("user.name");
        let email = get("user.email");
        let gravatar = email.as_ref().map(|e| {
            format!("https://www.gravatar.com/avatar/{:x}?s=96&d=404", md5::compute(e.trim().to_lowercase()))
        });
        Ok(json!({ "name": name, "email": email, "avatar": gravatar }))
    })
    .await
}

// ---------- revisão e sincronização ----------

#[tauri::command]
async fn get_changes(app: State<'_, Arc<App>>, id: String, base: String) -> Res<Value> {
    let t = app.thread(&id)?;
    let cwd = PathBuf::from(&t.cwd);
    let ch = blocking(move || {
        let (b, label) = git::resolve_base(&cwd, &base, t.base_ref.as_deref(), t.turn_tree.as_deref())?;
        git::changes(&cwd, &b, &label)
    })
    .await?;
    let seen = app.thread(&id)?.seen;
    let files: Vec<Value> = ch
        .files
        .iter()
        .map(|f| {
            let mut v = serde_json::to_value(f).unwrap_or_default();
            v["seen"] = json!(seen.get(&f.path) == Some(&f.fingerprint));
            v
        })
        .collect();
    Ok(json!({ "base": ch.base, "baseLabel": ch.base_label, "files": files }))
}

#[tauri::command]
fn set_seen(app: State<'_, Arc<App>>, id: String, entries: Vec<(String, Option<String>)>) -> Res<()> {
    app.update_thread(&id, |t| {
        for (path, fp) in entries {
            match fp {
                Some(fp) => {
                    t.seen.insert(path, fp);
                }
                None => {
                    t.seen.remove(&path);
                }
            }
        }
    })
    .map(|_| ())
}

#[tauri::command]
async fn branch_info(app: State<'_, Arc<App>>, id: String) -> Res<git::BranchInfo> {
    let t = app.thread(&id)?;
    blocking(move || git::branch_info(Path::new(&t.cwd), t.base_ref.as_deref())).await
}

#[tauri::command]
async fn land_changes(app: State<'_, Arc<App>>, id: String, mode: String) -> Res<String> {
    let t = app.thread(&id)?;
    if !t.isolated {
        return Err("Esta thread já trabalha direto no projeto".into());
    }
    let repo = PathBuf::from(&app.store.lock().project(&t.project_id).ok_or("Projeto não encontrado")?.path);
    blocking(move || {
        let cwd = PathBuf::from(&t.cwd);
        match mode.as_str() {
            "merge" => {
                if git::branch_info(&cwd, t.base_ref.as_deref())?.dirty {
                    anyhow::bail!("Há mudanças não commitadas na thread. Faça commit antes do merge.");
                }
                git::merge_into(&repo, t.branch.as_deref().unwrap_or_default())
            }
            _ => {
                let (base, _) = git::resolve_base(&cwd, "upstream", t.base_ref.as_deref(), None)?;
                git::apply_into(&cwd, &repo, &base)
            }
        }
    })
    .await
}

#[tauri::command]
fn open_path(path: String) -> Res<()> {
    platform::open_command(Path::new(&path)).spawn().map(|_| ()).map_err(e)
}

// ---------- terminais ----------

#[tauri::command]
fn term_spawn(handle: AppHandle, app: State<'_, Arc<App>>, term_id: String, cwd: String, cols: u16, rows: u16) -> Res<()> {
    let sink: terminal::DataSink = Arc::new(move |id: &str, data: Option<String>| {
        let _ = handle.emit("term-data", json!({ "termId": id, "data": data }));
    });
    app.terminals.spawn(&term_id, &cwd, cols, rows, sink).map_err(e)
}

#[tauri::command]
fn term_write(app: State<'_, Arc<App>>, term_id: String, data: String) -> Res<()> {
    app.terminals.write(&term_id, &data).map_err(e)
}

#[tauri::command]
fn term_resize(app: State<'_, Arc<App>>, term_id: String, cols: u16, rows: u16) -> Res<()> {
    app.terminals.resize(&term_id, cols, rows).map_err(e)
}

#[tauri::command]
fn term_kill(app: State<'_, Arc<App>>, term_id: String) {
    app.terminals.kill(&term_id);
}

/// Verdadeiro quando o processo foi iniciado como helper askpass do ssh-keygen
/// (assinatura de commits): a resposta já foi impressa e o processo deve sair.
pub fn answer_askpass() -> bool {
    git_client::answer_askpass()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    platform::prepare_env();
    let data_dir = store::data_dir();
    let app = Arc::new(App {
        store: parking_lot::Mutex::new(Store::load(&data_dir)),
        agents: AgentManager::default(),
        acp: acp::AcpManager::default(),
        provider_status: parking_lot::Mutex::new(HashMap::new()),
        terminals: terminal::Terminals::default(),
        status: parking_lot::Mutex::new(HashMap::new()),
        data_dir,
        sessions: parking_lot::Mutex::new(sessions::SessionCache::default()),
        capabilities: parking_lot::Mutex::new(HashMap::new()),
    });
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            git_client::setup(app.handle());
            Ok(())
        })
        .manage(app)
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            list_claude_dirs,
            auth_status,
            list_workspaces,
            create_workspace,
            update_workspace,
            remove_workspace,
            set_active_workspace,
            workspace_repos,
            get_capabilities,
            thread_mcp,
            trash_item,
            list_projects,
            list_threads,
            sync_sessions,
            create_thread,
            update_thread_meta,
            archive_thread,
            delete_thread,
            load_transcript,
            send_message,
            read_image_file,
            interrupt,
            respond_permission,
            set_thread_model,
            set_thread_permission_mode,
            get_changes,
            set_thread_options,
            set_thread_provider,
            provider_status,
            provider_auth_methods,
            provider_login,
            diff_stats,
            user_profile,
            set_seen,
            branch_info,
            land_changes,
            open_path,
            term_spawn,
            term_write,
            term_resize,
            term_kill,
            // cliente git (visão "Git")
            git_client::commands::wsl_cmds::submit_ssh_passphrase,
            git_client::commands::wsl_cmds::pending_ssh_passphrase,
            git_client::commands::wsl_cmds::submit_https_credentials,
            git_client::commands::wsl_cmds::pending_https_credentials,
            git_client::commands::wsl_cmds::list_ssh_keys,
            git_client::commands::wsl_cmds::list_wsl_distros,
            git_client::commands::wsl_cmds::list_wsl_dirs,
            git_client::commands::wsl_cmds::wsl_home,
            git_client::commands::repo_cmds::open_repository,
            git_client::commands::repo_cmds::get_repo_status,
            git_client::commands::repo_cmds::close_repository,
            git_client::commands::log_cmds::get_commit_log,
            git_client::commands::log_cmds::search_commits,
            git_client::commands::log_cmds::get_repo_fingerprint,
            git_client::commands::log_cmds::get_head_commit,
            git_client::commands::log_cmds::get_commit_signature,
            git_client::commands::diff_cmds::get_commit_diff,
            git_client::commands::diff_cmds::get_range_diff,
            git_client::commands::diff_cmds::get_file_content,
            git_client::commands::branch_cmds::list_branches,
            git_client::commands::branch_cmds::list_tags,
            git_client::commands::ai_cmds::ai_compose_commits,
            git_client::commands::ai_cmds::ai_resolve_conflict,
            git_client::commands::ai_cmds::ai_generate_pr_description,
            git_client::commands::ai_cmds::apply_composed_commits,
            git_client::commands::branch_cmds::create_tag,
            git_client::commands::branch_cmds::delete_local_tag,
            git_client::commands::branch_cmds::push_tag,
            git_client::commands::branch_cmds::delete_remote_tag,
            git_client::commands::branch_cmds::list_remote_tags,
            git_client::commands::branch_cmds::create_branch,
            git_client::commands::branch_cmds::checkout_branch,
            git_client::commands::branch_cmds::checkout_at,
            git_client::commands::branch_cmds::checkout_commit_detached,
            git_client::commands::branch_cmds::cherry_pick,
            git_client::commands::branch_cmds::reset_to_commit,
            git_client::commands::branch_cmds::reset_to_ref,
            git_client::commands::branch_cmds::sync_branch_to_remote,
            git_client::commands::branch_cmds::delete_local_branch,
            git_client::commands::branch_cmds::delete_remote_branch,
            git_client::commands::branch_cmds::delete_branch_both,
            git_client::commands::branch_cmds::merge_branch,
            git_client::commands::history_cmds::revert_commit,
            git_client::commands::history_cmds::get_file_history,
            git_client::commands::history_cmds::get_commit_file_diff,
            git_client::commands::history_cmds::get_blame,
            git_client::commands::history_cmds::get_branch_compare,
            git_client::commands::history_cmds::get_tree_diff,
            git_client::commands::rebase_cmds::start_rebase,
            git_client::commands::rebase_cmds::get_rebase_status,
            git_client::commands::rebase_cmds::resolve_rebase_conflict,
            git_client::commands::rebase_cmds::continue_rebase,
            git_client::commands::rebase_cmds::abort_rebase,
            git_client::commands::interactive_rebase_cmds::get_rebase_plan,
            git_client::commands::interactive_rebase_cmds::start_interactive_rebase,
            git_client::commands::repo_tools_cmds::list_worktrees,
            git_client::commands::repo_tools_cmds::add_worktree,
            git_client::commands::repo_tools_cmds::remove_worktree,
            git_client::commands::repo_tools_cmds::prune_worktrees,
            git_client::commands::repo_tools_cmds::list_submodules,
            git_client::commands::repo_tools_cmds::get_submodule_count,
            git_client::commands::repo_tools_cmds::update_submodules,
            git_client::commands::repo_tools_cmds::get_lfs_status,
            git_client::commands::repo_tools_cmds::run_lfs_pull,
            git_client::commands::remote_cmds::fetch_remote,
            git_client::commands::remote_cmds::push_branch,
            git_client::commands::pull_cmds::pull_branch,
            git_client::commands::pull_cmds::get_upstream_status,
            git_client::commands::remote_cmds::get_remote_url,
            git_client::commands::remote_cmds::list_remotes,
            git_client::commands::remote_cmds::add_remote,
            git_client::commands::remote_cmds::remove_remote,
            git_client::commands::remote_cmds::rename_remote,
            git_client::commands::remote_cmds::set_remote_url,
            git_client::commands::gitlab_cmds::gitlab_get_project,
            git_client::commands::gitlab_cmds::gitlab_list_open_merge_requests,
            git_client::commands::gitlab_cmds::gitlab_create_merge_request,
            git_client::commands::gitlab_cmds::gitlab_search_members,
            git_client::commands::gitlab_cmds::gitlab_list_project_events,
            git_client::commands::github_cmds::github_list_open_pull_requests,
            git_client::commands::github_cmds::github_create_pull_request,
            git_client::commands::github_cmds::github_list_assignees,
            git_client::commands::github_cmds::github_list_repo_events,
            git_client::commands::log_cmds::get_repo_stats,
            git_client::commands::log_cmds::get_reflog,
            git_client::commands::undo_cmds::undo_last_action,
            git_client::commands::undo_cmds::redo_last_action,
            git_client::commands::undo_cmds::get_undo_status,
            git_client::commands::stash_cmds::stash_save,
            git_client::commands::stash_cmds::stash_list,
            git_client::commands::stash_cmds::stash_pop,
            git_client::commands::stash_cmds::stash_apply,
            git_client::commands::stash_cmds::stash_drop,
            git_client::commands::working_cmds::get_working_status,
            git_client::commands::working_cmds::stage_path,
            git_client::commands::working_cmds::stage_all,
            git_client::commands::working_cmds::unstage_path,
            git_client::commands::working_cmds::unstage_all,
            git_client::commands::working_cmds::discard_path,
            git_client::commands::working_cmds::discard_all,
            git_client::commands::working_cmds::apply_lines,
            git_client::commands::working_cmds::add_ignore_pattern,
            git_client::commands::working_cmds::get_workdir_diff,
            git_client::commands::working_cmds::git_commit_changes,
            git_client::commands::working_cmds::amend_commit_message,
            git_client::commands::config_cmds::get_global_git_identity,
            git_client::commands::config_cmds::list_installed_editors,
            git_client::commands::config_cmds::list_system_fonts,
            git_client::commands::config_cmds::repo_file_path,
            git_client::commands::config_cmds::open_in_editor,
            git_client::commands::ai_cmds::ai_generate_commit_message,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o LazyAICode");
}

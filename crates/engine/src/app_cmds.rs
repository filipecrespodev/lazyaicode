//! Comandos do app: preferências, contas Claude, workspaces, projetos,
//! skills/agentes/MCP e provedores.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use lazyaicode_agent::{acp, agent, catalog, claude};
use lazyaicode_doc::{Project, Settings, Workspace};
use lazyaicode_protocol::events;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::registry::{json_of, p, parse, Registry, Spec};
use crate::store::{self, Store};
use crate::{blocking, workspace, Engine, EngineError, Result};

const CAPABILITIES_TTL: Duration = Duration::from_secs(120);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Id {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WsId {
    workspace_id: String,
}

/// Preferências com comando próprio (`settings.<chave>`), além do
/// `settings.save` com o objeto inteiro.
const SETTINGS_KEYS: [(&str, &str, &str); 9] = [
    ("default_model", "default-model", "string"),
    ("default_permission_mode", "default-permission-mode", "string"),
    ("send_with_modifier", "send-with-modifier", "bool"),
    ("notify_on_finish", "notify-on-finish", "bool"),
    ("sound_on_finish", "sound-on-finish", "bool"),
    ("default_provider", "default-provider", "string"),
    ("default_skip_permissions", "default-skip-permissions", "bool"),
    ("language", "language", "string"),
    ("check_updates", "check-updates", "bool"),
];

pub fn register(r: &mut Registry) {
    // ---------- preferências ----------
    r.add(
        Spec::new("settings.get", "Settings", "settings")
            .read_only()
            .doc("Preferências, binário do Claude encontrado e pastas do app.")
            .run(|e, _| async move {
                let s = e.core.store.lock();
                Ok(json!({
                    "settings": s.state.settings,
                    "claudeBinary": claude::find_claude_binary(),
                    "dataDir": e.core.data_dir,
                    "defaultClaudeDir": claude::default_claude_dir(),
                    // conta sugerida para um workspace novo: a do ambiente (CLAUDE_CONFIG_DIR)
                    "suggestedClaudeDir": claude::resolve_active(None),
                }))
            }),
    );
    r.add(Spec::new("settings.save", "Save settings", "settings").params(vec![p::json("settings")]).run(|e, v| async move {
        #[derive(Deserialize)]
        struct P {
            settings: Settings,
        }
        let p: P = parse(v)?;
        let mut s = e.core.store.lock();
        // as prefs da UI não vêm no objeto de settings antigo: preserva
        let prefs = std::mem::take(&mut s.state.settings.prefs);
        s.state.settings = p.settings;
        if s.state.settings.prefs.is_empty() {
            s.state.settings.prefs = prefs;
        }
        s.save()?;
        Ok(Value::Null)
    }));
    for (field, id, ty) in SETTINGS_KEYS {
        let cid = format!("settings.{id}");
        let param = if ty == "bool" { p::b("value") } else { p::s("value") };
        // rótulo legível na paleta ("Set default model"), como os outros comandos do motor
        let label = format!("Set {}", field.replace('_', " "));
        r.add(Spec::new(&cid, &label, "settings").params(vec![param]).doc("Muda uma preferência.").run(move |e, v| async move {
            let value = v.get("value").cloned().unwrap_or(Value::Null);
            let mut s = e.core.store.lock();
            let mut obj = serde_json::to_value(&s.state.settings).map_err(|e| EngineError::Failed(e.to_string()))?;
            obj[field] = value;
            s.state.settings = serde_json::from_value(obj).map_err(|e| EngineError::BadParams(e.to_string()))?;
            s.save()?;
            drop(s);
            e.emit(events::STATE_CHANGED, json!({ "what": "settings" }));
            Ok(Value::Null)
        }));
    }
    r.add(
        Spec::new("prefs.get", "Read a preference", "settings")
            .params(vec![p::os("key")])
            .read_only()
            .doc("Preferência da interface ou do cliente git (`theme`, `sshKeys`, `signCommits`…); sem `key`, todas.")
            .run(|e, v| async move {
                let s = e.core.store.lock();
                Ok(match v.get("key").and_then(Value::as_str) {
                    Some(k) => s.state.settings.prefs.get(k).cloned().unwrap_or(Value::Null),
                    None => Value::Object(s.state.settings.prefs.clone()),
                })
            }),
    );
    r.add(
        Spec::new("prefs.set", "Change a preference", "settings")
            .params(vec![p::s("key"), p::ojson("value")])
            .doc("Grava uma preferência; `value: null` apaga.")
            .run(|e, v| async move {
                let key = v.get("key").and_then(Value::as_str).unwrap_or_default().to_string();
                let value = v.get("value").cloned().unwrap_or(Value::Null);
                if key.is_empty() || key.len() > 128 {
                    return Err(EngineError::BadParams("chave inválida".into()));
                }
                let mut s = e.core.store.lock();
                if value.is_null() {
                    s.state.settings.prefs.remove(&key);
                } else {
                    s.state.settings.prefs.insert(key.clone(), value);
                }
                s.save()?;
                drop(s);
                e.emit(events::STATE_CHANGED, json!({ "what": "prefs", "key": key }));
                Ok(Value::Null)
            }),
    );

    // ---------- contas Claude ----------
    r.add(
        Spec::new("account.list", "Claude accounts", "account")
            .read_only()
            .doc("Diretórios de configuração do Claude Code encontrados.")
            .run(|_, _| async move { json_of(claude::discover_dirs(&claude::home())) }),
    );
    r.add(
        Spec::new("account.status", "Account login", "account")
            .params(vec![p::opath("claudeDir")])
            .read_only()
            .doc("Login de uma conta (diretório Claude); sem `claudeDir`, a ativa (`$CLAUDE_CONFIG_DIR` ou `~/.claude`).")
            .run(|_, v| async move {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct P {
                    claude_dir: Option<String>,
                }
                let p: P = parse(v)?;
                let dir = p.claude_dir.filter(|d| !d.is_empty()).map(PathBuf::from).unwrap_or_else(|| claude::resolve_active(None));
                blocking(move || Ok(claude::auth_status(&dir))).await
            }),
    );

    // ---------- workspaces ----------
    r.add(Spec::new("workspace.list", "Workspaces", "workspace").read_only().returns("{workspaces, active}").run(|e, _| async move {
        let s = e.core.store.lock();
        Ok(json!({ "workspaces": s.state.workspaces, "active": s.state.active_workspace }))
    }));
    r.add(
        Spec::new("workspace.create", "New workspace", "workspace")
            .params(vec![p::path("path"), p::os("name"), p::opath("claudeDir")])
            .menu(&["File", "New workspace"])
            .doc("Cria um workspace a partir de uma pasta e o torna o ativo.")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct P {
                    path: String,
                    name: Option<String>,
                    claude_dir: Option<String>,
                }
                let p: P = parse(v)?;
                let w = create_workspace(&mut e.core.store.lock(), &p.path, p.name, p.claude_dir)?;
                e.emit(events::STATE_CHANGED, json!({ "what": "workspaces" }));
                json_of(w)
            }),
    );
    r.add(
        Spec::new("workspace.update", "Edit workspace", "workspace")
            .params(vec![p::s("id"), p::os("name"), p::opath("claudeDir"), p::olist("ignoredPaths")])
            .doc("Renomeia, troca a conta (`claudeDir: \"\"` = ~/.claude) ou a lista de pastas ignoradas.")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct P {
                    id: String,
                    name: Option<String>,
                    #[serde(default, deserialize_with = "double_option")]
                    claude_dir: Option<Option<String>>,
                    ignored_paths: Option<Vec<String>>,
                }
                let p: P = parse(v)?;
                let (w, account_changed) = {
                    let mut s = e.core.store.lock();
                    let w = s
                        .state
                        .workspaces
                        .iter_mut()
                        .find(|w| w.id == p.id)
                        .ok_or_else(|| EngineError::NotFound("Workspace não encontrado".into()))?;
                    if let Some(n) = p.name.filter(|n| !n.trim().is_empty()) {
                        w.name = n.trim().to_string();
                    }
                    let mut changed = false;
                    if let Some(d) = p.claude_dir {
                        let d = d.filter(|d| !d.is_empty() && *d != claude::default_claude_dir());
                        changed = d != w.claude_dir;
                        w.claude_dir = d;
                    }
                    if let Some(i) = p.ignored_paths {
                        w.ignored_paths = i;
                    }
                    let out = w.clone();
                    s.save()?;
                    (out, changed)
                };
                if account_changed {
                    // threads abertas continuam na conta delas; só encerramos os
                    // processos para que nada siga rodando com a conta antiga
                    let ids: Vec<String> = e
                        .core
                        .store
                        .lock()
                        .state
                        .threads
                        .iter()
                        .filter(|t| t.workspace_id.as_deref() == Some(p.id.as_str()))
                        .map(|t| t.id.clone())
                        .collect();
                    for tid in ids {
                        e.stop_agent(&tid).await;
                    }
                }
                e.emit(events::STATE_CHANGED, json!({ "what": "workspaces" }));
                json_of(w)
            }),
    );
    r.add(
        Spec::new("workspace.remove", "Remove workspace", "workspace")
            .params(vec![p::s("id")])
            .destructive()
            .doc("Tira o workspace do app (as pastas e as branches das threads ficam).")
            .run(|e, v| async move {
                let Id { id } = parse(v)?;
                let ids: Vec<String> = e
                    .core
                    .store
                    .lock()
                    .state
                    .threads
                    .iter()
                    .filter(|t| t.workspace_id.as_deref() == Some(id.as_str()))
                    .map(|t| t.id.clone())
                    .collect();
                for tid in &ids {
                    e.stop_agent(tid).await;
                }
                let mut s = e.core.store.lock();
                // threads isoladas guardam trabalho numa branch própria: não somem com o
                // workspace — ficam sem dono e voltam se a pasta for adicionada de novo
                for t in s.state.threads.iter_mut().filter(|t| t.workspace_id.as_deref() == Some(id.as_str())) {
                    t.workspace_id = None;
                }
                s.state.workspaces.retain(|w| w.id != id);
                if s.state.active_workspace.as_deref() == Some(id.as_str()) {
                    s.state.active_workspace = s.state.workspaces.first().map(|w| w.id.clone());
                }
                s.save()?;
                drop(s);
                e.emit(events::STATE_CHANGED, json!({ "what": "workspaces" }));
                Ok(Value::Null)
            }),
    );
    r.add(Spec::new("workspace.activate", "Switch workspace", "workspace").params(vec![p::s("id")]).run(|e, v| async move {
        let Id { id } = parse(v)?;
        e.workspace(&id)?;
        let mut s = e.core.store.lock();
        s.state.active_workspace = Some(id);
        s.save()?;
        Ok(Value::Null)
    }));
    r.add(
        Spec::new("workspace.repos", "Workspace folders", "workspace")
            .params(vec![p::s("id")])
            .read_only()
            .doc("Todas as pastas do workspace, inclusive as ignoradas, marcando as que são repositórios.")
            .run(|e, v| async move {
                let Id { id } = parse(v)?;
                let w = e.workspace(&id)?;
                json_of(workspace::scan_repos_routed(e.git(), &w.path).await)
            }),
    );

    // ---------- projetos ----------
    r.add(
        Spec::new("project.list", "Projects", "project")
            .params(vec![p::s("workspaceId")])
            .read_only()
            .doc("Repositórios do workspace (menos os ignorados) mais qualquer projeto onde uma thread dele esteja.")
            .run(|e, v| async move {
                let WsId { workspace_id } = parse(v)?;
                json_of(list_projects(&e, &workspace_id).await?)
            }),
    );

    // ---------- skills, agentes e MCP ----------
    r.add(
        Spec::new("capabilities.get", "Skills, agents and MCP", "capabilities")
            .params(vec![p::s("workspaceId"), p::opath("path"), p::ob("force")])
            .read_only()
            .doc("O que o Claude Code carrega numa pasta com a conta do workspace: skills e comandos, agentes, MCPs e modelos.")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct P {
                    workspace_id: String,
                    path: Option<String>,
                    force: Option<bool>,
                }
                let p: P = parse(v)?;
                let w = e.workspace(&p.workspace_id)?;
                let dir = workspace::claude_dir(&w);
                let cwd = PathBuf::from(p.path.clone().unwrap_or_else(|| w.path.clone()));
                let key = (dir.clone(), cwd.clone());
                let cached = e.core.capabilities.lock().get(&key).cloned();
                let probe = match cached {
                    Some((at, v)) if !p.force.unwrap_or(false) && at.elapsed() < CAPABILITIES_TTL => v,
                    _ => {
                        let bin = claude::find_claude_binary().ok_or("Claude CLI não encontrado")?;
                        let v = agent::probe(&bin, &dir, &cwd).await?;
                        e.core.capabilities.lock().insert(key, (Instant::now(), v.clone()));
                        v
                    }
                };
                let project = p.path.map(PathBuf::from);
                let (d, pr) = (dir.clone(), project.clone());
                let fs = blocking(move || Ok(catalog::scan(&d, pr.as_deref()))).await?;
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
            }),
    );
    r.add(
        Spec::new("capabilities.trash", "Move skill/agent to trash", "capabilities")
            .params(vec![p::s("workspaceId"), p::path("path")])
            .destructive()
            .doc("Move um skill (a pasta inteira) ou um agente/comando para a lixeira. Só itens da conta ou do `.claude/` de um projeto, nunca de plugins.")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct P {
                    workspace_id: String,
                    path: String,
                }
                let p: P = parse(v)?;
                let w = e.workspace(&p.workspace_id)?;
                let dir = workspace::claude_dir(&w);
                let path = PathBuf::from(&p.path);
                let in_account = ["skills", "agents", "commands"].iter().any(|d| path.starts_with(dir.join(d)));
                let in_project = path.components().any(|c| c.as_os_str() == ".claude" || c.as_os_str() == ".agents")
                    && workspace::contains(&w, &path);
                if path.starts_with(dir.join("plugins")) || !(in_account || in_project) || !path.is_file() {
                    return Err(EngineError::Disabled("Este item não pode ser apagado pelo app".into()));
                }
                let target = match (path.file_name().is_some_and(|n| n == "SKILL.md"), path.parent()) {
                    (true, Some(parent)) => parent.to_path_buf(),
                    _ => path,
                };
                trash::delete(&target).map_err(|e| EngineError::Failed(format!("Não foi possível mover para a lixeira: {e}")))?;
                e.core.capabilities.lock().clear();
                Ok(Value::Null)
            }),
    );

    // ---------- provedores ----------
    let providers: Vec<&str> = acp::PROVIDERS.to_vec();
    r.add(
        Spec::new("provider.status", "Provider status", "provider")
            .params(vec![p::one_of("provider", &providers, true), p::ob("force")])
            .read_only()
            .doc("Situação do Codex (OpenAI) ou do Gemini CLI (Google) na máquina: instalado, login, versão, modos e modelos.")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    provider: String,
                    force: Option<bool>,
                }
                let p: P = parse(v)?;
                if !p.force.unwrap_or(false) {
                    if let Some((at, v)) = e.core.provider_status.lock().get(&p.provider) {
                        if at.elapsed() < Duration::from_secs(300) {
                            return Ok(v.clone());
                        }
                    }
                }
                let dir = std::env::temp_dir().join("lazyaicode-provider-check");
                let _ = std::fs::create_dir_all(&dir);
                let v = acp::status(&p.provider, &dir).await;
                e.core.provider_status.lock().insert(p.provider, (Instant::now(), v.clone()));
                Ok(v)
            }),
    );
    r.add(
        Spec::new("provider.auth-methods", "Provider login methods", "provider")
            .params(vec![p::one_of("provider", &providers, true)])
            .read_only()
            .run(|_, v| async move {
                #[derive(Deserialize)]
                struct P {
                    provider: String,
                }
                let p: P = parse(v)?;
                Ok(acp::auth_methods(&p.provider, &std::env::temp_dir()).await?)
            }),
    );
    r.add(
        Spec::new("provider.login", "Provider login", "provider")
            .params(vec![p::one_of("provider", &providers, true), p::s("method")])
            .doc("Login pelo próprio agente (abre o navegador).")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    provider: String,
                    method: String,
                }
                let p: P = parse(v)?;
                acp::login(&p.provider, &p.method, &std::env::temp_dir()).await?;
                e.core.provider_status.lock().remove(&p.provider);
                Ok(Value::Null)
            }),
    );

    // ---------- sistema ----------
    r.add(
        Spec::new("system.user-profile", "Git profile", "system")
            .read_only()
            .doc("Nome e e-mail do git (global) e o avatar do Gravatar.")
            .run(|_, _| async move {
                blocking(|| {
                    let get = |k: &str| {
                        lazyaicode_platform::command("git")
                            .args(["config", "--global", k])
                            .output()
                            .ok()
                            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                            .filter(|s| !s.is_empty())
                    };
                    let name = get("user.name");
                    let email = get("user.email");
                    let gravatar = email
                        .as_ref()
                        .map(|e| format!("https://www.gravatar.com/avatar/{:x}?s=96&d=404", md5::compute(e.trim().to_lowercase())));
                    Ok(json!({ "name": name, "email": email, "avatar": gravatar }))
                })
                .await
            }),
    );
    r.add(
        Spec::new("system.open-path", "Open in file manager", "system")
            .params(vec![p::path("path")])
            .doc("Abre um arquivo ou pasta no programa padrão do sistema.")
            .run(|_, v| async move {
                #[derive(Deserialize)]
                struct P {
                    path: String,
                }
                let p: P = parse(v)?;
                if !Path::new(&p.path).exists() && lazyaicode_platform::wslpath::parse_unc(&p.path).is_none() {
                    return Err(EngineError::NotFound(format!("Não existe: {}", p.path)));
                }
                lazyaicode_platform::open_command(Path::new(&p.path)).spawn().map_err(|e| EngineError::Failed(e.to_string()))?;
                Ok(Value::Null)
            }),
    );
    r.add(
        Spec::new("system.read-image", "Read image file", "system")
            .params(vec![p::path("path")])
            .read_only()
            .returns("{mime, data (base64)}")
            .doc("Bytes de uma imagem do disco para anexar no chat (até 5 MB; PNG, JPEG, GIF ou WebP).")
            .run(|_, v| async move {
                #[derive(Deserialize)]
                struct P {
                    path: String,
                }
                let p: P = parse(v)?;
                blocking(move || {
                    let len = std::fs::metadata(&p.path)?.len();
                    if len > 5 * 1024 * 1024 {
                        anyhow::bail!("too_large");
                    }
                    let bytes = std::fs::read(&p.path)?;
                    let mime = sniff_image(&bytes).ok_or_else(|| anyhow::anyhow!("não é PNG, JPEG, GIF nem WebP"))?;
                    use base64::Engine as _;
                    Ok(json!({ "mime": mime, "data": base64::engine::general_purpose::STANDARD.encode(&bytes) }))
                })
                .await
            }),
    );
}

/// Tipo de imagem pelos primeiros bytes (não pela extensão).
pub fn sniff_image(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.starts_with(b"GIF8") {
        Some("image/gif")
    } else if b.len() >= 12 && b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}

/// `Some(None)` para `null` explícito, `None` para ausente.
fn double_option<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}

/// A conta guardada no workspace: sem `claudeDir`, a ativa
/// (`$CLAUDE_CONFIG_DIR` ou `~/.claude`), a mesma que a tela sugere; `~/.claude`
/// (ou `""`) fica `None`.
fn account_to_store(claude_dir: Option<String>) -> Option<String> {
    let d = claude_dir.unwrap_or_else(|| claude::resolve_active(None).to_string_lossy().to_string());
    Some(d).filter(|d| !d.is_empty() && Path::new(d) != claude::default_claude_dir())
}

pub(crate) fn create_workspace(s: &mut Store, path: &str, name: Option<String>, claude_dir: Option<String>) -> Result<Workspace> {
    let p = PathBuf::from(path);
    if !p.is_dir() {
        return Err(EngineError::NotFound(format!("Pasta não encontrada: {path}")));
    }
    let w = Workspace {
        id: uuid::Uuid::new_v4().to_string(),
        name: name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(path.to_string())),
        path: path.to_string(),
        claude_dir: account_to_store(claude_dir),
        ignored_paths: vec![],
        created_at: store::now(),
    };
    s.state.workspaces.push(w.clone());
    s.state.active_workspace = Some(w.id.clone());
    s.save()?;
    Ok(w)
}

/// Projeto para uma pasta: a raiz do repositório git, ou a própria pasta
/// quando não é git (o chat funciona; diff e workspace isolado não).
/// `plain`: a varredura já sabe que a pasta não tem `.git` — não sobe até um
/// repositório pai (o workspace pode ser ele mesmo um repo).
pub(crate) fn ensure_project(store: &mut Store, cwd: &Path, plain: bool) -> String {
    // Repo dentro de uma distro WSL: a varredura (feita lá pelo agente) já
    // deu a raiz; rodar `git rev-parse` sobre o 9P seria lento.
    let (root, is_git) = if plain {
        (cwd.to_path_buf(), false)
    } else if lazyaicode_platform::wslpath::parse_unc(&cwd.to_string_lossy()).is_some() {
        (cwd.to_path_buf(), cwd.join(".git").exists())
    } else {
        match lazyaicode_git::thread::repo_root(cwd) {
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

pub(crate) async fn list_projects(e: &Engine, workspace_id: &str) -> Result<Vec<Project>> {
    let w = e.workspace(workspace_id)?;
    let repos = workspace::scan_repos_routed(e.git(), &w.path).await;
    let mut s = e.core.store.lock();
    let mut ids: Vec<String> = Vec::new();
    let mut git_dirs = std::collections::HashMap::new();
    for r in repos.iter().filter(|r| !workspace::is_ignored(&w, Path::new(&r.path))) {
        let id = ensure_project(&mut s, Path::new(&r.path), !r.is_repo);
        git_dirs.insert(id.clone(), r.is_repo);
        ids.push(id);
    }
    let from_threads: Vec<String> =
        s.state.threads.iter().filter(|t| t.workspace_id.as_deref() == Some(w.id.as_str())).map(|t| t.project_id.clone()).collect();
    for id in from_threads {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    // projetos de threads fora da varredura: confere o `.git` direto
    for p in s.state.projects.iter_mut().filter(|p| ids.contains(&p.id)) {
        p.is_git = git_dirs.get(&p.id).copied().unwrap_or_else(|| Path::new(&p.path).join(".git").exists());
    }
    s.save()?;
    Ok(ids.iter().filter_map(|id| s.state.project(id).cloned()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_images_by_magic_bytes() {
        assert_eq!(sniff_image(&[0x89, b'P', b'N', b'G', 0]), Some("image/png"));
        assert_eq!(sniff_image(b"GIF89a"), Some("image/gif"));
        assert_eq!(sniff_image(b"RIFF\0\0\0\0WEBPVP8"), Some("image/webp"));
        assert_eq!(sniff_image(b"RIFF"), None);
        assert_eq!(sniff_image(b"hello"), None);
    }
}

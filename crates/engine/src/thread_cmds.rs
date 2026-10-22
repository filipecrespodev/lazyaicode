//! Comandos das threads: criar, conversar, permissões, opções da sessão,
//! revisão das mudanças, integração ao projeto e terminais.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lazyaicode_agent::agent::{ImageInput, SpawnOpts, IMAGE_MIMES};
use lazyaicode_agent::{acp, agent, catalog, claude, terminal, transcript};
use lazyaicode_doc::{Status, Thread};
use lazyaicode_git::thread as git;
use lazyaicode_protocol::events;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::app_cmds::ensure_project;
use crate::registry::{json_of, p, parse, Registry, Spec};
use crate::{blocking, store, workspace, Engine, EngineError, Result};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Id {
    id: String,
}

fn is_acp(t: &Thread) -> bool {
    t.provider != "claude"
}

fn providers() -> Vec<&'static str> {
    let mut v = vec!["claude"];
    v.extend(acp::PROVIDERS.iter().copied());
    v
}

pub fn register(r: &mut Registry) {
    r.add(Spec::new("thread.list", "Threads", "thread").params(vec![p::s("workspaceId")]).read_only().run(|e, v| async move {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P {
            workspace_id: String,
        }
        let p: P = parse(v)?;
        let threads: Vec<Thread> = e
            .core
            .store
            .lock()
            .state
            .threads
            .iter()
            .filter(|t| t.workspace_id.as_deref() == Some(p.workspace_id.as_str()))
            .cloned()
            .collect();
        json_of(threads.iter().map(|t| e.view(t)).collect::<Vec<_>>())
    }));
    r.add(Spec::new("thread.get", "Thread", "thread").params(vec![p::s("id")]).read_only().run(|e, v| async move {
        let Id { id } = parse(v)?;
        json_of(e.view(&e.thread(&id)?))
    }));
    r.add(
        Spec::new("thread.sync-sessions", "Import Claude Code sessions", "thread")
            .params(vec![p::s("workspaceId")])
            .doc("Traz as sessões do Claude Code da conta do workspace, que rodaram dentro da pasta dele, como threads. Devolve quantas surgiram.")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct P {
                    workspace_id: String,
                }
                let p: P = parse(v)?;
                let n = sync_sessions(&e, &p.workspace_id).await?;
                if n > 0 {
                    e.emit(events::STATE_CHANGED, json!({ "what": "threads" }));
                }
                Ok(json!(n))
            }),
    );
    r.add(
        Spec::new("thread.create", "New thread", "thread")
            .params(vec![
                p::s("workspaceId"),
                p::s("projectId"),
                p::b("isolated").doc("checkout isolado (worktree numa branch própria)"),
                p::os("model"),
                p::os("permissionMode"),
                p::one_of("provider", &providers(), false),
                p::ob("skipPermissions"),
            ])
            .menu(&["Thread", "New thread"])
            .shortcut("Ctrl+N")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct P {
                    workspace_id: String,
                    project_id: String,
                    isolated: bool,
                    model: Option<String>,
                    permission_mode: Option<String>,
                    provider: Option<String>,
                    skip_permissions: Option<bool>,
                }
                let p: P = parse(v)?;
                let t =
                    create_thread(&e, p.workspace_id, p.project_id, p.isolated, p.model, p.permission_mode, p.provider, p.skip_permissions)
                        .await?;
                e.emit(events::STATE_CHANGED, json!({ "what": "threads" }));
                json_of(e.view(&t))
            }),
    );
    r.add(
        Spec::new("thread.update", "Rename / favorite thread", "thread")
            .params(vec![p::s("id"), p::os("title"), p::ob("bookmarked"), p::ob("unread")])
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    title: Option<String>,
                    bookmarked: Option<bool>,
                    unread: Option<bool>,
                }
                let p: P = parse(v)?;
                let t = e.update_thread(&p.id, |t| {
                    if let Some(v) = p.title.filter(|s| !s.trim().is_empty()) {
                        t.title = v.trim().chars().take(200).collect();
                    }
                    if let Some(v) = p.bookmarked {
                        t.bookmarked = v;
                    }
                    if let Some(v) = p.unread {
                        t.unread = v;
                    }
                })?;
                let view = e.view(&t);
                e.emit(events::THREAD_UPDATED, json!(view));
                json_of(view)
            }),
    );
    r.add(
        Spec::new("thread.archive", "Archive thread", "thread")
            .params(vec![p::s("id"), p::b("archived")])
            .menu(&["Thread", "Archive"])
            .doc("Arquiva (o checkout isolado sai do disco; a branch fica) ou desarquiva (recria o checkout a partir da branch).")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    archived: bool,
                }
                let p: P = parse(v)?;
                let t = e.thread(&p.id)?;
                e.stop_agent(&p.id).await;
                if t.isolated {
                    let repo = project_path(&e, &t)?;
                    let cwd = PathBuf::from(&t.cwd);
                    let branch = t.branch.clone().unwrap_or_default();
                    if p.archived {
                        blocking(move || git::remove_isolated(&repo, &cwd)).await?;
                    } else if !cwd.exists() {
                        blocking(move || git::git(&repo, &["worktree", "add", &cwd.to_string_lossy(), &branch]).map(|_| ())).await?;
                    }
                }
                let t = e.update_thread(&p.id, |t| t.archived = p.archived)?;
                e.emit(events::STATE_CHANGED, json!({ "what": "threads" }));
                json_of(e.view(&t))
            }),
    );
    r.add(
        Spec::new("thread.delete", "Delete thread", "thread")
            .params(vec![p::s("id")])
            .destructive()
            .doc("Apaga a thread; numa isolada, também o checkout e a branch dela.")
            .run(|e, v| async move {
                let Id { id } = parse(v)?;
                let t = e.thread(&id)?;
                e.stop_agent(&id).await;
                if t.isolated {
                    let repo = project_path(&e, &t)?;
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
                let mut s = e.core.store.lock();
                s.state.threads.retain(|x| x.id != id);
                s.save()?;
                drop(s);
                e.emit(events::STATE_CHANGED, json!({ "what": "threads" }));
                Ok(Value::Null)
            }),
    );
    r.add(
        Spec::new("thread.transcript", "Thread history", "thread")
            .params(vec![p::s("id")])
            .read_only()
            .returns("lista de mensagens no formato do stream (para lazyaicode_doc::transcript)")
            .run(|e, v| async move {
                let Id { id } = parse(v)?;
                let t = e.thread(&id)?;
                if is_acp(&t) {
                    let f = e.acp_transcript(&id);
                    return json_of(blocking(move || Ok(transcript::load(&f))).await?);
                }
                let Some(sid) = t.session_id.clone() else { return Ok(json!([])) };
                let f = claude::session_file(&e.thread_claude_dir(&t), &t.cwd, &sid);
                json_of(blocking(move || Ok(transcript::load(&f))).await?)
            }),
    );
    r.add(
        Spec::new("thread.send", "Send message", "thread")
            .params(vec![p::s("id"), p::s("text"), p::ojson("images").doc("[{mime, data(base64)}]")])
            .doc("Manda uma mensagem ao agente da thread, abrindo o processo se preciso. A resposta chega pelos eventos `agent-event`.")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    text: String,
                    images: Option<Vec<ImageInput>>,
                }
                let p: P = parse(v)?;
                send_message(&e, &p.id, p.text, p.images.unwrap_or_default()).await?;
                Ok(Value::Null)
            }),
    );
    r.add(
        Spec::new("thread.interrupt", "Stop the agent", "thread")
            .params(vec![p::s("id")])
            .shortcut("Escape")
            .doc("Interrompe o turno em curso.")
            .run(|e, v| async move {
                let Id { id } = parse(v)?;
                if e.core.acp.is_running(&id) {
                    e.core.acp.cancel(&id).await?;
                } else if e.core.agents.is_running(&id) {
                    e.core.interrupted.lock().insert(id.clone());
                    e.core.agents.control(&id, json!({ "subtype": "interrupt" })).await?;
                }
                Ok(Value::Null)
            }),
    );
    r.add(
        Spec::new("thread.respond-permission", "Answer a permission request", "thread")
            .params(vec![
                p::s("id"),
                p::s("requestId"),
                p::b("allow"),
                p::ojson("input"),
                p::os("message"),
                p::os("toolUseId"),
                p::ojson("updatedPermissions").doc("regras para não perguntar de novo (as `suggestions` do pedido)"),
            ])
            .run(|e, v| async move {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct P {
                    id: String,
                    request_id: String,
                    allow: bool,
                    #[serde(default)]
                    input: Value,
                    message: Option<String>,
                    tool_use_id: Option<String>,
                    updated_permissions: Option<Vec<Value>>,
                }
                let mut p: P = parse(v)?;
                // o CLI só aceita a resposta com o `toolUseID` (e a entrada) do pedido: quem
                // manda só id + requestId + allow (CLI, MCP) recebe os do pedido pendente
                let pending = e.core.permissions.lock().get(&p.id).filter(|r| r["requestId"] == p.request_id.as_str()).cloned();
                // uma resposta a outro pedido (atrasada, de outro cliente, já cancelado)
                // o CLI ignoraria; contá-la como respondida tiraria o pedido de verdade
                // da tela e deixaria a thread "rodando" para sempre
                let Some(r) = pending else {
                    return Err(EngineError::NotFound(format!("O pedido de permissão {} não está mais pendente", p.request_id)));
                };
                if p.tool_use_id.is_none() {
                    p.tool_use_id = r["toolUseId"].as_str().map(str::to_string);
                }
                if p.input.is_null() {
                    p.input = r["input"].clone();
                }
                if e.core.acp.is_running(&p.id) {
                    e.core.acp.respond_permission(&p.id, &p.request_id, p.allow, p.updated_permissions.is_some()).await?;
                } else {
                    e.core
                        .agents
                        .respond_permission(&p.id, &p.request_id, p.allow, p.input, p.message, p.tool_use_id, p.updated_permissions)
                        .await?;
                }
                e.core.permissions.lock().remove(&p.id);
                e.set_status(&p.id, Status::Running);
                Ok(Value::Null)
            }),
    );
    r.add(
        Spec::new("thread.pending-permission", "Pending permission request", "thread")
            .params(vec![p::s("id")])
            .read_only()
            .returns("{requestId, toolName, input, toolUseId, suggestions} ou null")
            .doc("O pedido de permissão que o agente da thread espera responder (status `awaiting`), para quem não acompanha os eventos: responda com `thread.respond-permission`.")
            .run(|e, v| async move {
                let Id { id } = parse(v)?;
                e.thread(&id)?;
                Ok(e.core.permissions.lock().get(&id).cloned().unwrap_or(Value::Null))
            }),
    );
    r.add(Spec::new("thread.set-model", "Change model", "thread").params(vec![p::s("id"), p::s("model")]).run(|e, v| async move {
        #[derive(Deserialize)]
        struct P {
            id: String,
            model: String,
        }
        let p: P = parse(v)?;
        let t = e.update_thread(&p.id, |t| t.model = p.model.clone())?;
        if e.core.acp.is_running(&p.id) && !p.model.is_empty() {
            e.core.acp.set_option(&p.id, "model", &p.model).await?;
        }
        if e.core.agents.is_running(&p.id) {
            let m = if p.model == "default" { Value::Null } else { json!(p.model) };
            let _ = e.core.agents.control(&p.id, json!({ "subtype": "set_model", "model": m })).await;
        }
        json_of(e.view(&t))
    }));
    r.add(
        Spec::new("thread.set-permission-mode", "Change permission mode", "thread")
            .params(vec![p::s("id"), p::s("mode")])
            .shortcut("Shift+Tab")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    mode: String,
                }
                let p: P = parse(v)?;
                let t = e.update_thread(&p.id, |t| t.permission_mode = p.mode.clone())?;
                if e.core.acp.is_running(&p.id) && !p.mode.is_empty() {
                    e.core.acp.set_option(&p.id, "mode", &p.mode).await?;
                }
                if e.core.agents.is_running(&p.id) {
                    if p.mode == "default" {
                        // "padrão das settings" não tem equivalente em execução:
                        // o próximo processo nasce sem a flag
                        e.core.agents.stop(&p.id).await;
                    } else {
                        let _ = e.core.agents.control(&p.id, json!({ "subtype": "set_permission_mode", "mode": p.mode })).await;
                    }
                }
                json_of(e.view(&t))
            }),
    );
    r.add(
        Spec::new("thread.set-options", "Session options", "thread")
            .params(vec![p::s("id"), p::os("effort"), p::os("agent"), p::olist("addDirs"), p::ob("skipPermissions")])
            .doc("Opções que só valem ao abrir o processo (esforço, agente, pastas extras). Com o agente aberto e parado, ele é encerrado: o próximo turno retoma a mesma conversa já com elas. `\"\"` limpa.")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct P {
                    id: String,
                    effort: Option<String>,
                    agent: Option<String>,
                    add_dirs: Option<Vec<String>>,
                    skip_permissions: Option<bool>,
                }
                let p: P = parse(v)?;
                if let Some(bad) = p.add_dirs.as_ref().and_then(|d| d.iter().find(|d| !Path::new(d).is_dir())) {
                    return Err(EngineError::NotFound(format!("Pasta não encontrada: {bad}")));
                }
                let t = e.update_thread(&p.id, |t| {
                    if let Some(v) = p.effort {
                        t.effort = Some(v).filter(|s| !s.is_empty());
                    }
                    if let Some(v) = p.agent {
                        t.agent = Some(v).filter(|s| !s.is_empty());
                    }
                    if let Some(v) = p.add_dirs {
                        t.add_dirs = v;
                    }
                    if let Some(v) = p.skip_permissions {
                        t.skip_permissions = v && t.provider == "claude";
                    }
                })?;
                if e.core.agents.is_running(&p.id) && e.status_of(&p.id) == Status::Idle {
                    e.core.agents.stop(&p.id).await;
                }
                json_of(e.view(&t))
            }),
    );
    r.add(
        Spec::new("thread.set-provider", "Change provider", "thread")
            .params(vec![p::s("id"), p::one_of("provider", &providers(), true), p::os("model")])
            .doc("Troca o provedor de uma thread que ainda não conversou.")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    provider: String,
                    model: Option<String>,
                }
                let p: P = parse(v)?;
                let t = e.thread(&p.id)?;
                if t.session_id.is_some() {
                    return Err(EngineError::Disabled("Esta thread já tem conversa; abra uma thread nova para usar outro provedor".into()));
                }
                e.stop_agent(&p.id).await;
                let defaults = e.core.store.lock().state.settings.clone();
                let t = e.update_thread(&p.id, |t| {
                    let claude = p.provider == "claude";
                    t.provider = p.provider.clone();
                    t.model = p.model.unwrap_or(if claude { defaults.default_model.clone() } else { String::new() });
                    t.permission_mode = if claude { defaults.default_permission_mode.clone() } else { String::new() };
                    if !claude {
                        t.effort = None;
                        t.agent = None;
                        t.add_dirs.clear();
                        t.skip_permissions = false;
                    }
                })?;
                json_of(e.view(&t))
            }),
    );
    r.add(
        Spec::new("thread.mcp", "Session MCP servers", "thread")
            .params(vec![p::s("id"), p::one_of("action", &["status", "toggle", "reconnect"], true), p::os("server"), p::ob("enabled")])
            .doc("MCP da sessão aberta: status, ligar/desligar (só nesta sessão, como o `/mcp`) e reconectar.")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    action: String,
                    server: Option<String>,
                    enabled: Option<bool>,
                }
                let p: P = parse(v)?;
                if !e.core.agents.is_running(&p.id) {
                    return Err(EngineError::Disabled(
                        "A thread não está com o agente aberto. Envie uma mensagem para iniciar a sessão.".into(),
                    ));
                }
                let name = p.server.unwrap_or_default();
                match p.action.as_str() {
                    "toggle" => {
                        e.core
                            .agents
                            .request(&p.id, json!({ "subtype": "mcp_toggle", "serverName": name, "enabled": p.enabled.unwrap_or(true) }))
                            .await?;
                    }
                    "reconnect" => {
                        e.core.agents.request(&p.id, json!({ "subtype": "mcp_reconnect", "serverName": name })).await?;
                    }
                    _ => {}
                }
                let st = e.core.agents.request(&p.id, json!({ "subtype": "mcp_status" })).await?;
                Ok(catalog::redact_mcp(&st["mcpServers"]))
            }),
    );

    // ---------- revisão ----------
    r.add(
        Spec::new("thread.diff-stats", "Diff summary", "review")
            .params(vec![p::s("id")])
            .read_only()
            .returns("{additions, deletions, files, branch}")
            .doc("Linhas adicionadas/removidas desde a base da thread.")
            .run(|e, v| async move {
                let Id { id } = parse(v)?;
                let t = e.thread(&id)?;
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
            }),
    );
    r.add(
        Spec::new("thread.changes", "Changes for review", "review")
            .params(vec![p::s("id"), p::one_of("base", &["upstream", "turn", "head"], true)])
            .read_only()
            .returns("{base, baseLabel, files: [{path, status, additions, deletions, patch, fingerprint, seen}]}")
            .doc("Arquivos mudados desde a base (`upstream` = de onde a thread partiu, `turn` = início do último turno, `head` = último commit).")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    base: String,
                }
                let p: P = parse(v)?;
                let t = e.thread(&p.id)?;
                let cwd = PathBuf::from(&t.cwd);
                let ch = blocking(move || {
                    let (b, label) = git::resolve_base(&cwd, &p.base, t.base_ref.as_deref(), t.turn_tree.as_deref())?;
                    git::changes(&cwd, &b, &label)
                })
                .await?;
                let seen = e.thread(&p.id)?.seen;
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
            }),
    );
    r.add(
        Spec::new("thread.set-seen", "Mark files as reviewed", "review")
            .params(vec![p::s("id"), p::json("entries").doc("[[caminho, impressão digital | null]]")])
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    entries: Vec<(String, Option<String>)>,
                }
                let p: P = parse(v)?;
                e.update_thread(&p.id, |t| {
                    for (path, fp) in p.entries {
                        match fp {
                            Some(fp) => {
                                t.seen.insert(path, fp);
                            }
                            None => {
                                t.seen.remove(&path);
                            }
                        }
                    }
                })?;
                Ok(Value::Null)
            }),
    );
    r.add(
        Spec::new("thread.branch-info", "Branch status", "review")
            .params(vec![p::s("id")])
            .read_only()
            .returns("{branch, ahead, behind, dirty, base_ref}")
            .run(|e, v| async move {
                let Id { id } = parse(v)?;
                let t = e.thread(&id)?;
                json_of(blocking(move || git::branch_info(Path::new(&t.cwd), t.base_ref.as_deref())).await?)
            }),
    );
    r.add(
        Spec::new("thread.land", "Bring changes into the project", "review")
            .params(vec![p::s("id"), p::one_of("mode", &["merge", "apply"], true)])
            .menu(&["Thread", "Integrate"])
            .doc("`apply` copia as mudanças como não commitadas no checkout principal; `merge` faz merge da branch da thread (precisa de commit).")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    mode: String,
                }
                let p: P = parse(v)?;
                let t = e.thread(&p.id)?;
                if !t.isolated {
                    return Err(EngineError::Disabled("Esta thread já trabalha direto no projeto".into()));
                }
                let repo = project_path(&e, &t)?;
                let out = blocking(move || {
                    let cwd = PathBuf::from(&t.cwd);
                    match p.mode.as_str() {
                        "merge" => {
                            // só o rastreado conta: o que rodar os testes deixa (`__pycache__`,
                            // build) não entra no merge, e não pode impedi-lo
                            if !git::git(&cwd, &["status", "--porcelain", "--untracked-files=no"])?.trim().is_empty() {
                                anyhow::bail!("Há mudanças não commitadas na thread. Faça commit antes do merge.");
                            }
                            let out = git::merge_into(&repo, t.branch.as_deref().unwrap_or_default())?;
                            let left = git::git(&cwd, &["ls-files", "--others", "--exclude-standard"])?.lines().count();
                            Ok(match left {
                                0 => out,
                                n => format!("{out}\n{n} arquivo(s) não rastreado(s) da thread ficaram de fora (sem commit)."),
                            })
                        }
                        _ => {
                            let (base, _) = git::resolve_base(&cwd, "upstream", t.base_ref.as_deref(), None)?;
                            git::apply_into(&cwd, &repo, &base)
                        }
                    }
                })
                .await?;
                Ok(json!(out))
            }),
    );

    // ---------- terminais ----------
    r.add(
        Spec::new("terminal.spawn", "Open terminal", "terminal")
            .params(vec![p::s("termId"), p::path("cwd"), p::i("cols"), p::i("rows")])
            .doc("Abre um shell num PTY. A saída chega pelos eventos `term-data` (base64).")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct P {
                    term_id: String,
                    cwd: String,
                    cols: u16,
                    rows: u16,
                }
                let p: P = parse(v)?;
                let weak = Arc::downgrade(&e.core);
                let sink: terminal::DataSink = Arc::new(move |id: &str, data: Option<String>| {
                    if let Some(core) = weak.upgrade() {
                        Engine { core }.emit(events::TERM_DATA, json!({ "termId": id, "data": data }));
                    }
                });
                let (cols, rows) = (p.cols.clamp(2, 1000), p.rows.clamp(1, 500));
                e.core.terminals.spawn(&p.term_id, &p.cwd, cols, rows, sink)?;
                Ok(Value::Null)
            }),
    );
    r.add(Spec::new("terminal.write", "Type into terminal", "terminal").params(vec![p::s("termId"), p::s("data")]).run(
        |e, v| async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct P {
                term_id: String,
                data: String,
            }
            let p: P = parse(v)?;
            e.core.terminals.write(&p.term_id, &p.data)?;
            Ok(Value::Null)
        },
    ));
    r.add(Spec::new("terminal.resize", "Resize terminal", "terminal").params(vec![p::s("termId"), p::i("cols"), p::i("rows")]).run(
        |e, v| async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct P {
                term_id: String,
                cols: u16,
                rows: u16,
            }
            let p: P = parse(v)?;
            e.core.terminals.resize(&p.term_id, p.cols.clamp(2, 1000), p.rows.clamp(1, 500))?;
            Ok(Value::Null)
        },
    ));
    r.add(Spec::new("terminal.kill", "Close terminal", "terminal").params(vec![p::s("termId")]).run(|e, v| async move {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P {
            term_id: String,
        }
        let p: P = parse(v)?;
        e.core.terminals.kill(&p.term_id);
        Ok(Value::Null)
    }));
    r.add(Spec::new("terminal.list", "Open terminals", "terminal").read_only().run(|e, _| async move { json_of(e.core.terminals.ids()) }));
}

fn project_path(e: &Engine, t: &Thread) -> Result<PathBuf> {
    Ok(PathBuf::from(
        &e.core.store.lock().state.project(&t.project_id).ok_or_else(|| EngineError::NotFound("Projeto não encontrado".into()))?.path,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_thread(
    e: &Engine,
    workspace_id: String,
    project_id: String,
    isolated: bool,
    model: Option<String>,
    permission_mode: Option<String>,
    provider: Option<String>,
    skip_permissions: Option<bool>,
) -> Result<Thread> {
    let w = e.workspace(&workspace_id)?;
    let provider = provider.filter(|p| !p.is_empty()).unwrap_or_else(|| "claude".into());
    if provider != "claude" && !acp::PROVIDERS.contains(&provider.as_str()) {
        return Err(EngineError::BadParams(format!("Provedor desconhecido: {provider}")));
    }
    let (project, settings) = {
        let s = e.core.store.lock();
        (
            s.state.project(&project_id).cloned().ok_or_else(|| EngineError::NotFound("Projeto não encontrado".into()))?,
            s.state.settings.clone(),
        )
    };
    // sem `.git` não há worktree: a thread roda na própria pasta
    let isolated = isolated && Path::new(&project.path).join(".git").exists();
    let id = uuid::Uuid::new_v4().to_string();
    let short: String = id.chars().take(8).collect();
    let (cwd, branch, base_ref) = if isolated {
        let dest = e.core.data_dir.join("worktrees").join(&project.id).join(&short);
        let repo = PathBuf::from(&project.path);
        let iso = blocking(move || git::create_isolated(&repo, &dest, &short, true)).await?;
        (iso.path.to_string_lossy().to_string(), Some(iso.branch), Some(iso.base_ref))
    } else {
        (project.path.clone(), None, None)
    };
    let now = store::now();
    let claude = provider == "claude";
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
        model: model.unwrap_or(if claude { settings.default_model.clone() } else { String::new() }),
        permission_mode: permission_mode.unwrap_or(if claude { settings.default_permission_mode.clone() } else { String::new() }),
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
        skip_permissions: claude && skip_permissions.unwrap_or(settings.default_skip_permissions),
        provider,
    };
    let mut s = e.core.store.lock();
    s.state.threads.push(t.clone());
    s.save()?;
    Ok(t)
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

pub(crate) async fn send_message(e: &Engine, id: &str, text: String, images: Vec<ImageInput>) -> Result<()> {
    if text.trim().is_empty() && images.is_empty() {
        return Err(EngineError::BadParams("Mensagem vazia".into()));
    }
    if let Some(i) = images.iter().find(|i| !IMAGE_MIMES.contains(&i.mime.as_str())) {
        return Err(EngineError::BadParams(format!("Formato de imagem não suportado: {} (use PNG, JPEG, GIF ou WebP)", i.mime)));
    }
    let t = e.thread(id)?;
    if t.archived {
        return Err(EngineError::Disabled("Thread arquivada".into()));
    }
    if e.status_of(id) == Status::Idle {
        // uma interrupção que não chegou a fechar turno nenhum não vale para o próximo
        e.core.interrupted.lock().remove(id);
    }
    let cwd = PathBuf::from(&t.cwd);
    if !cwd.is_dir() {
        return Err(EngineError::NotFound(format!("A pasta da thread não existe: {}", t.cwd)));
    }
    // foto da pasta no início do turno para o diff "Último turno"
    let snap_cwd = cwd.clone();
    let turn_tree = blocking(move || git::snapshot_tree(&snap_cwd)).await.ok();
    let title = (t.title == "Nova thread").then(|| {
        let first: String = text.lines().next().unwrap_or("").chars().take(60).collect();
        if first.trim().is_empty() {
            "Imagem".to_string()
        } else {
            first
        }
    });
    let updated = e.update_thread(id, |t| {
        if turn_tree.is_some() {
            t.turn_tree = turn_tree;
        }
        if let Some(title) = title {
            t.title = title;
        }
        t.updated_at = store::now();
        t.unread = false;
    })?;
    e.emit(events::THREAD_UPDATED, json!(e.view(&updated)));

    if is_acp(&t) {
        let sink = e.make_sink();
        e.append_transcript(id, &json!({ "type": "user", "message": { "role": "user", "content": agent::user_content(&text, &images) } }));
        e.set_status(id, Status::Running);
        let ensured = e
            .core
            .acp
            .ensure(
                id,
                acp::AcpOpts {
                    provider: t.provider.clone(),
                    cwd,
                    session_id: t.session_id.clone(),
                    model: Some(t.model.clone()),
                    mode: Some(t.permission_mode.clone()),
                },
                sink.clone(),
            )
            .await;
        if let Err(err) = ensured {
            e.set_status(id, Status::Error);
            return Err(EngineError::Failed(acp_hint(&t.provider, &err.to_string())));
        }
        return e.core.acp.prompt(id, &text, &images, sink).await.map_err(|err| {
            e.set_status(id, Status::Idle);
            EngineError::from(err)
        });
    }

    let bin = claude::find_claude_binary().ok_or("Claude CLI não encontrado. Instale o Claude Code ou defina LAZYAICODE_CLAUDE_BIN.")?;
    e.core
        .agents
        .ensure(
            id,
            SpawnOpts {
                bin,
                claude_dir: e.thread_claude_dir(&t),
                cwd,
                model: t.model.clone(),
                permission_mode: t.permission_mode.clone(),
                resume: t.session_id.clone(),
                effort: t.effort.clone(),
                agent: t.agent.clone(),
                add_dirs: t.add_dirs.clone(),
                skip_permissions: t.skip_permissions,
            },
            e.make_sink(),
        )
        .await?;
    e.set_status(id, Status::Running);
    // antes de escrever: o eco do CLI pode chegar antes de `send_user` voltar
    let blocks = lazyaicode_agent::agent::user_content(&text, &images).len() as u64;
    e.core.queued.lock().entry(id.to_string()).or_default().pending += blocks;
    if let Err(err) = e.core.agents.send_user(id, &text, &images).await {
        e.core.queued.lock().remove(id);
        return Err(err.into());
    }
    Ok(())
}

pub(crate) async fn sync_sessions(e: &Engine, workspace_id: &str) -> Result<usize> {
    let w = e.workspace(workspace_id)?;
    let dir = workspace::claude_dir(&w);
    let core = e.core.clone();
    let d = dir.clone();
    let found = blocking(move || Ok(core.sessions.lock().scan(&d))).await?;
    let dir_s = dir.to_string_lossy().to_string();
    let worktrees = e.core.data_dir.join("worktrees");
    let status = e.core.status.lock().clone();
    let mut s = e.core.store.lock();
    let mut added = 0;
    for info in found {
        let cwd = PathBuf::from(&info.cwd);
        if !workspace::owns_session_cwd(&w, &cwd) || cwd.starts_with(&worktrees) || !cwd.is_dir() {
            continue;
        }
        let existing = s
            .state
            .threads
            .iter_mut()
            .find(|t| t.session_id.as_deref() == Some(info.session_id.as_str()) || t.past_sessions.contains(&info.session_id));
        if let Some(t) = existing {
            // threads de antes dos workspaces (ou de um workspace removido)
            // são adotadas por quem tem a mesma conta e contém a pasta
            if t.workspace_id.is_none() && t.claude_dir.as_deref().is_none_or(|c| c == dir_s) {
                t.workspace_id = Some(w.id.clone());
                t.claude_dir = Some(dir_s.clone());
            }
            let busy = status.get(&t.id).is_some_and(|st| *st != Status::Idle);
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
    let orphans: Vec<(usize, String)> = s
        .state
        .threads
        .iter()
        .enumerate()
        .filter(|(_, t)| t.workspace_id.is_none() && t.claude_dir.as_deref().is_none_or(|c| c == dir_s))
        .map(|(i, t)| (i, t.project_id.clone()))
        .collect();
    for (i, pid) in orphans {
        if s.state.project(&pid).is_some_and(|p| workspace::owns_session_cwd(&w, Path::new(&p.path))) {
            if let Some(t) = s.state.threads.get_mut(i) {
                t.workspace_id = Some(w.id.clone());
                t.claude_dir = Some(dir_s.clone());
            }
        }
    }
    s.save()?;
    Ok(added)
}

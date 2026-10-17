//! Comandos de conveniência para scripts e agentes: git por caminho (sem
//! abrir repositório antes), utilidades de thread (exportar, buscar,
//! esperar, commit/push no checkout), saúde do sistema e terminais.

use std::path::{Path, PathBuf};
use std::time::Duration;

use lazyaicode_agent::{claude, transcript};
use lazyaicode_doc::transcript::{Item, Transcript};
use lazyaicode_doc::Status;
use lazyaicode_git::thread as git;
use lazyaicode_protocol::events;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::registry::{json_of, p, parse, Registry, Spec};
use crate::{blocking, Engine, EngineError, Result};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Id {
    id: String,
}

#[derive(Deserialize)]
struct PathP {
    path: String,
}

fn git_at(path: &str, args: &'static [&'static str]) -> impl FnOnce() -> anyhow::Result<String> + Send + 'static {
    let p = PathBuf::from(path);
    move || git::git(&p, args)
}

/// A conversa de uma thread já montada (histórico do disco).
async fn transcript_of(e: &Engine, id: &str) -> Result<Transcript> {
    let msgs = e.run("thread.transcript", json!({ "id": id })).await?;
    let mut t = Transcript::new();
    for m in msgs.as_array().into_iter().flatten() {
        t.apply(m);
    }
    Ok(t)
}

fn markdown(items: &[Item]) -> String {
    let mut out = String::new();
    for i in items {
        match i {
            Item::User { text, .. } => out.push_str(&format!("### Você\n\n{text}\n\n")),
            Item::Text { text, .. } => out.push_str(&format!("{text}\n\n")),
            Item::Tool { name, input, result, .. } => {
                out.push_str(&format!("> `{name}` {}\n", lazyaicode_doc::transcript::tool_summary(name, input)));
                if let Some(r) = result.as_ref().filter(|r| r.is_error) {
                    out.push_str(&format!("> ⚠ {}\n", r.text.lines().next().unwrap_or("")));
                }
                out.push('\n');
            }
            Item::Error { text, .. } | Item::Notice { text, .. } => out.push_str(&format!("> {text}\n\n")),
            Item::Result { duration_ms, cost_usd, .. } => {
                out.push_str(&format!("_turno: {:.1}s, US$ {cost_usd:.4}_\n\n", *duration_ms as f64 / 1000.0))
            }
            _ => {}
        }
    }
    out.trim_end().to_string()
}

pub fn register(r: &mut Registry) {
    // ---------- git por caminho ----------
    let path_git: [(&str, &str, &'static [&'static str], &str); 12] = [
        ("git.path.status", "Status (git status)", &["status", "--porcelain=v1", "-b"], "Saída de `git status --porcelain -b` na pasta."),
        ("git.path.log", "Recent commits", &["log", "--oneline", "-n", "30", "--decorate"], "Os 30 últimos commits (`git log --oneline`)."),
        ("git.path.diff", "Unstaged diff", &["diff", "--no-color"], "Diff do que não está no staging."),
        ("git.path.diff-staged", "Staged diff", &["diff", "--cached", "--no-color"], "Diff do que está no staging."),
        ("git.path.branches", "Branches", &["branch", "-a", "--format=%(refname:short)"], "Todas as branches (locais e remotas)."),
        ("git.path.current-branch", "Current branch", &["rev-parse", "--abbrev-ref", "HEAD"], "Nome da branch atual."),
        ("git.path.head", "HEAD commit", &["rev-parse", "HEAD"], "SHA do HEAD."),
        ("git.path.remotes", "Remotes", &["remote", "-v"], "Remotos configurados."),
        ("git.path.stash-list", "Stashes", &["stash", "list"], "Entradas do stash."),
        ("git.path.tags", "Tags", &["tag", "--sort=-creatordate"], "Tags, das mais novas."),
        ("git.path.root", "Repository root", &["rev-parse", "--show-toplevel"], "Raiz do repositório que contém a pasta."),
        ("git.path.shortstat", "Change summary", &["diff", "--shortstat", "HEAD"], "Resumo das mudanças desde o HEAD."),
    ];
    for (id, label, args, doc) in path_git {
        r.add(Spec::new(id, label, "git").params(vec![p::path("path")]).read_only().doc(doc).run(move |_, v| async move {
            let PathP { path } = parse(v)?;
            if !Path::new(&path).is_dir() {
                return Err(EngineError::NotFound(format!("Pasta não encontrada: {path}")));
            }
            Ok(json!(blocking(git_at(&path, args)).await?.trim_end()))
        }));
    }
    r.add(Spec::new("git.path.is-repo", "Is a git repository?", "git").params(vec![p::path("path")]).read_only().run(|_, v| async move {
        let PathP { path } = parse(v)?;
        let p2 = PathBuf::from(&path);
        Ok(json!(blocking(move || Ok(git::repo_root(&p2).is_ok())).await?))
    }));
    r.add(
        Spec::new("git.path.commit-all", "Commit everything", "git")
            .params(vec![p::path("path"), p::s("message")])
            .doc("`git add -A` e commit na pasta.")
            .run(|_, v| async move {
                #[derive(Deserialize)]
                struct P {
                    path: String,
                    message: String,
                }
                let p: P = parse(v)?;
                if p.message.trim().is_empty() {
                    return Err(EngineError::BadParams("mensagem vazia".into()));
                }
                let path = PathBuf::from(&p.path);
                Ok(json!(blocking(move || git::commit_all(&path, &p.message)).await?))
            }),
    );

    // ---------- threads ----------
    r.add(Spec::new("thread.rename", "Rename thread", "thread").params(vec![p::s("id"), p::s("title")]).run(|e, v| async move {
        #[derive(Deserialize)]
        struct P {
            id: String,
            title: String,
        }
        let p: P = parse(v)?;
        e.run("thread.update", json!({ "id": p.id, "title": p.title })).await
    }));
    for (id, label, field, value) in [
        ("thread.bookmark", "Add to favorites", "bookmarked", true),
        ("thread.unbookmark", "Remove from favorites", "bookmarked", false),
        ("thread.mark-read", "Mark as read", "unread", false),
        ("thread.mark-unread", "Mark as unread", "unread", true),
    ] {
        r.add(Spec::new(id, label, "thread").params(vec![p::s("id")]).run(move |e, v| async move {
            let Id { id } = parse(v)?;
            e.run("thread.update", json!({ "id": id, field: value })).await
        }));
    }
    r.add(Spec::new("thread.unarchive", "Unarchive thread", "thread").params(vec![p::s("id")]).run(|e, v| async move {
        let Id { id } = parse(v)?;
        e.run("thread.archive", json!({ "id": id, "archived": false })).await
    }));
    r.add(
        Spec::new("thread.status", "Thread status", "thread")
            .params(vec![p::s("id")])
            .read_only()
            .returns("idle | running | awaiting | error")
            .run(|e, v| async move {
                let Id { id } = parse(v)?;
                e.thread(&id)?;
                Ok(json!(e.status_of(&id)))
            }),
    );
    r.add(
        Spec::new("thread.wait", "Wait for the agent", "thread")
            .params(vec![p::s("id"), p::oi("timeoutMs")])
            .read_only()
            .returns("o status final (idle | awaiting | error | timeout)")
            .doc("Espera a thread sair de `running` (fim do turno ou pedido de permissão).")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct P {
                    id: String,
                    timeout_ms: Option<u64>,
                }
                let p: P = parse(v)?;
                e.thread(&p.id)?;
                let mut rx = e.subscribe();
                if e.status_of(&p.id) != Status::Running {
                    return Ok(json!(e.status_of(&p.id)));
                }
                let deadline = tokio::time::Instant::now() + Duration::from_millis(p.timeout_ms.unwrap_or(600_000).clamp(100, 3_600_000));
                loop {
                    match tokio::time::timeout_at(deadline, rx.recv()).await {
                        Err(_) => return Ok(json!("timeout")),
                        Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => return Ok(json!(e.status_of(&p.id))),
                        Ok(_) => {
                            let s = e.status_of(&p.id);
                            if s != Status::Running {
                                return Ok(json!(s));
                            }
                        }
                    }
                }
            }),
    );
    r.add(
        Spec::new("thread.export", "Export conversation", "thread")
            .params(vec![p::s("id"), p::one_of("format", &["markdown", "json"], false)])
            .read_only()
            .doc("A conversa inteira em Markdown (padrão) ou como a lista de itens em JSON.")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    format: Option<String>,
                }
                let p: P = parse(v)?;
                let t = transcript_of(&e, &p.id).await?;
                if p.format.as_deref() == Some("json") {
                    json_of(&t.items)
                } else {
                    Ok(json!(markdown(&t.items)))
                }
            }),
    );
    r.add(Spec::new("thread.last-message", "Last agent message", "thread").params(vec![p::s("id")]).read_only().run(|e, v| async move {
        let Id { id } = parse(v)?;
        let t = transcript_of(&e, &id).await?;
        let last = t.items.iter().rev().find_map(|i| match i {
            Item::Text { text, .. } => Some(text.clone()),
            _ => None,
        });
        Ok(json!(last))
    }));
    r.add(
        Spec::new("thread.search", "Search threads", "thread")
            .params(vec![p::s("query"), p::os("workspaceId"), p::ob("archived")])
            .read_only()
            .doc("Threads cujo título ou pasta contém o texto.")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct P {
                    query: String,
                    workspace_id: Option<String>,
                    archived: Option<bool>,
                }
                let p: P = parse(v)?;
                let q = p.query.to_lowercase();
                let s = e.core.store.lock();
                let found: Vec<Value> = s
                    .state
                    .threads
                    .iter()
                    .filter(|t| p.workspace_id.as_ref().is_none_or(|w| t.workspace_id.as_ref() == Some(w)))
                    .filter(|t| p.archived.is_none_or(|a| t.archived == a))
                    .filter(|t| t.title.to_lowercase().contains(&q) || t.cwd.to_lowercase().contains(&q))
                    .map(|t| json!({ "id": t.id, "title": t.title, "cwd": t.cwd, "archived": t.archived, "updatedAt": t.updated_at }))
                    .collect();
                Ok(Value::Array(found))
            }),
    );
    r.add(
        Spec::new("thread.duplicate", "Duplicate thread", "thread")
            .params(vec![p::s("id"), p::ob("isolated")])
            .doc("Uma thread nova no mesmo projeto, com o mesmo provedor, modelo e modo (conversa do zero).")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    isolated: Option<bool>,
                }
                let p: P = parse(v)?;
                let t = e.thread(&p.id)?;
                e.run(
                    "thread.create",
                    json!({
                        "workspaceId": t.workspace_id,
                        "projectId": t.project_id,
                        "isolated": p.isolated.unwrap_or(t.isolated),
                        "model": t.model,
                        "permissionMode": t.permission_mode,
                        "provider": t.provider,
                        "skipPermissions": t.skip_permissions,
                    }),
                )
                .await
            }),
    );
    r.add(
        Spec::new("thread.commit", "Commit in the thread checkout", "review")
            .params(vec![p::s("id"), p::s("message")])
            .doc("`git add -A` e commit no checkout da thread (antes de um merge).")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    message: String,
                }
                let p: P = parse(v)?;
                let t = e.thread(&p.id)?;
                if p.message.trim().is_empty() {
                    return Err(EngineError::BadParams("mensagem vazia".into()));
                }
                let cwd = PathBuf::from(&t.cwd);
                let out = blocking(move || git::commit_all(&cwd, &p.message)).await?;
                e.emit(events::THREAD_UPDATED, json!(e.view(&t)));
                Ok(json!(out))
            }),
    );
    r.add(
        Spec::new("thread.push", "Push the thread branch", "review")
            .params(vec![p::s("id"), p::os("remote")])
            .doc("Push da branch da thread (com upstream).")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    remote: Option<String>,
                }
                let p: P = parse(v)?;
                let t = e.thread(&p.id)?;
                let cwd = PathBuf::from(&t.cwd);
                let remote = p.remote.unwrap_or_else(|| "origin".into());
                Ok(json!(
                    blocking(move || {
                        let b = git::git(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"])?.trim().to_string();
                        git::git(&cwd, &["push", "-u", &remote, &b])
                    })
                    .await?
                ))
            }),
    );
    r.add(
        Spec::new("thread.discard-file", "Revert a file to the thread base", "review")
            .params(vec![p::s("id"), p::s("path")])
            .destructive()
            .doc("Volta um arquivo do checkout da thread ao que era na base dela (`git checkout <base> -- <arquivo>`; arquivos novos são removidos).")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    path: String,
                }
                let p: P = parse(v)?;
                if p.path.contains("..") || Path::new(&p.path).is_absolute() {
                    return Err(EngineError::BadParams("caminho deve ser relativo ao checkout".into()));
                }
                let t = e.thread(&p.id)?;
                let cwd = PathBuf::from(&t.cwd);
                blocking(move || {
                    let (base, _) = git::resolve_base(&cwd, "upstream", t.base_ref.as_deref(), None)?;
                    let exists = git::git(&cwd, &["cat-file", "-e", &format!("{base}:{}", p.path)]).is_ok();
                    if exists {
                        git::git(&cwd, &["checkout", &base, "--", &p.path])?;
                    } else {
                        let _ = git::git(&cwd, &["rm", "-f", "--cached", "--", &p.path]);
                        let f = cwd.join(&p.path);
                        if f.is_file() {
                            std::fs::remove_file(f)?;
                        }
                    }
                    Ok(Value::Null)
                })
                .await
            }),
    );
    r.add(
        Spec::new("thread.file-diff", "Diff of one file", "review")
            .params(vec![p::s("id"), p::s("path"), p::one_of("base", &["upstream", "turn", "head"], false)])
            .read_only()
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    path: String,
                    base: Option<String>,
                }
                let p: P = parse(v)?;
                let ch = e.run("thread.changes", json!({ "id": p.id, "base": p.base.unwrap_or_else(|| "upstream".into()) })).await?;
                let f = ch["files"].as_array().and_then(|fs| fs.iter().find(|f| f["path"] == p.path.as_str()).cloned());
                Ok(f.map(|f| f["patch"].clone()).unwrap_or(Value::Null))
            }),
    );
    r.add(
        Spec::new("thread.update-from-base", "Update the thread with its base", "review")
            .params(vec![p::s("id")])
            .doc("Traz para a branch da thread o que entrou na base desde que ela nasceu (`git merge <base>` no checkout).")
            .run(|e, v| async move {
                let Id { id } = parse(v)?;
                let t = e.thread(&id)?;
                let base = t.base_ref.clone().ok_or_else(|| EngineError::Disabled("thread sem base (não é isolada)".into()))?;
                let cwd = PathBuf::from(&t.cwd);
                Ok(json!(
                    blocking(move || {
                        let _ = git::git(&cwd, &["fetch", "--quiet"]);
                        git::git(&cwd, &["merge", "--no-edit", &base])
                    })
                    .await?
                ))
            }),
    );
    r.add(Spec::new("thread.count", "Thread counts", "thread").params(vec![p::os("workspaceId")]).read_only().run(|e, v| async move {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P {
            workspace_id: Option<String>,
        }
        let p: P = parse(v)?;
        let s = e.core.store.lock();
        let st = e.core.status.lock();
        let mine: Vec<_> =
            s.state.threads.iter().filter(|t| p.workspace_id.as_ref().is_none_or(|w| t.workspace_id.as_ref() == Some(w))).collect();
        let by = |x: Status| mine.iter().filter(|t| st.get(&t.id).copied().unwrap_or_default() == x).count();
        Ok(json!({
            "total": mine.len(),
            "archived": mine.iter().filter(|t| t.archived).count(),
            "isolated": mine.iter().filter(|t| t.isolated).count(),
            "unread": mine.iter().filter(|t| t.unread).count(),
            "running": by(Status::Running),
            "awaiting": by(Status::Awaiting),
            "error": by(Status::Error),
        }))
    }));

    // ---------- workspaces e projetos ----------
    r.add(Spec::new("workspace.get", "Workspace", "workspace").params(vec![p::s("id")]).read_only().run(|e, v| async move {
        let Id { id } = parse(v)?;
        json_of(e.workspace(&id)?)
    }));
    r.add(Spec::new("workspace.active", "Active workspace", "workspace").read_only().run(|e, _| async move {
        let s = e.core.store.lock();
        Ok(s.state.active_workspace.as_deref().and_then(|id| s.state.workspace(id)).map(|w| json!(w)).unwrap_or(Value::Null))
    }));
    r.add(Spec::new("workspace.rename", "Rename workspace", "workspace").params(vec![p::s("id"), p::s("name")]).run(|e, v| async move {
        #[derive(Deserialize)]
        struct P {
            id: String,
            name: String,
        }
        let p: P = parse(v)?;
        e.run("workspace.update", json!({ "id": p.id, "name": p.name })).await
    }));
    r.add(Spec::new("workspace.set-account", "Change workspace account", "workspace").params(vec![p::s("id"), p::opath("claudeDir")]).run(
        |e, v| async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct P {
                id: String,
                claude_dir: Option<String>,
            }
            let p: P = parse(v)?;
            e.run("workspace.update", json!({ "id": p.id, "claudeDir": p.claude_dir.unwrap_or_default() })).await
        },
    ));
    for (cid, label, hide) in [("workspace.ignore", "Hide a folder", true), ("workspace.unignore", "Show a hidden folder", false)] {
        r.add(Spec::new(cid, label, "workspace").params(vec![p::s("id"), p::path("path")]).run(move |e, v| async move {
            #[derive(Deserialize)]
            struct P {
                id: String,
                path: String,
            }
            let p: P = parse(v)?;
            let w = e.workspace(&p.id)?;
            // a comparação é por caminho absoluto: `docs` vale como `<workspace>/docs`
            let path =
                if Path::new(&p.path).is_absolute() { p.path } else { Path::new(&w.path).join(&p.path).to_string_lossy().to_string() };
            let mut ignored = w.ignored_paths;
            ignored.retain(|x| *x != path);
            if hide {
                ignored.push(path);
            }
            e.run("workspace.update", json!({ "id": p.id, "ignoredPaths": ignored })).await
        }));
    }
    r.add(Spec::new("project.get", "Project", "project").params(vec![p::s("id")]).read_only().run(|e, v| async move {
        let Id { id } = parse(v)?;
        let p = e.core.store.lock().state.project(&id).cloned().ok_or_else(|| EngineError::NotFound("Projeto não encontrado".into()))?;
        json_of(p)
    }));
    r.add(Spec::new("project.threads", "Threads of a project", "project").params(vec![p::s("id")]).read_only().run(|e, v| async move {
        let Id { id } = parse(v)?;
        let ts: Vec<_> = e.core.store.lock().state.threads.iter().filter(|t| t.project_id == id).cloned().collect();
        json_of(ts.iter().map(|t| e.view(t)).collect::<Vec<_>>())
    }));
    r.add(Spec::new("project.open-folder", "Open project folder", "project").params(vec![p::s("id")]).run(|e, v| async move {
        let Id { id } = parse(v)?;
        let path = e
            .core
            .store
            .lock()
            .state
            .project(&id)
            .map(|p| p.path.clone())
            .ok_or_else(|| EngineError::NotFound("Projeto não encontrado".into()))?;
        e.run("system.open-path", json!({ "path": path })).await
    }));

    // ---------- preferências ----------
    r.add(Spec::new("prefs.list", "Preference keys", "settings").read_only().run(|e, _| async move {
        let mut keys: Vec<String> = e.core.store.lock().state.settings.prefs.keys().cloned().collect();
        keys.sort();
        Ok(json!(keys))
    }));
    r.add(
        Spec::new("prefs.reset", "Reset preferences", "settings")
            .params(vec![p::os("prefix")])
            .destructive()
            .doc("Apaga as preferências (todas, ou as que começam com `prefix`).")
            .run(|e, v| async move {
                let prefix = v.get("prefix").and_then(Value::as_str).unwrap_or("").to_string();
                let mut s = e.core.store.lock();
                s.state.settings.prefs.retain(|k, _| !k.starts_with(&prefix));
                s.save()?;
                drop(s);
                e.emit(events::STATE_CHANGED, json!({ "what": "prefs" }));
                Ok(Value::Null)
            }),
    );
    r.add(
        Spec::new("settings.reset", "Reset settings", "settings")
            .destructive()
            .doc("Volta as preferências das threads ao padrão (mantém as da interface).")
            .run(|e, _| async move {
                let mut s = e.core.store.lock();
                let prefs = std::mem::take(&mut s.state.settings.prefs);
                s.state.settings = lazyaicode_doc::Settings { prefs, ..Default::default() };
                s.save()?;
                drop(s);
                e.emit(events::STATE_CHANGED, json!({ "what": "settings" }));
                Ok(Value::Null)
            }),
    );
    r.add(
        Spec::new("settings.export", "Export settings", "settings")
            .read_only()
            .run(|e, _| async move { json_of(e.core.store.lock().state.settings.clone()) }),
    );

    // ---------- sistema ----------
    r.add(
        Spec::new("system.health", "Health check", "system")
            .read_only()
            .returns("{claude, git, node, dataDirWritable, …}")
            .doc("O que o app precisa na máquina: Claude Code, git, Node (para Codex/Gemini via npx) e a pasta de dados.")
            .run(|e, _| async move {
                let data = e.core.data_dir.clone();
                blocking(move || {
                    let version = |bin: &str| {
                        lazyaicode_platform::command(bin)
                            .arg("--version")
                            .output()
                            .ok()
                            .filter(|o| o.status.success())
                            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    };
                    let claude = claude::find_claude_binary();
                    let probe = data.join(".write-test");
                    let writable = std::fs::write(&probe, b"ok").is_ok();
                    let _ = std::fs::remove_file(&probe);
                    Ok(json!({
                        "claude": claude.as_ref().map(|p| p.to_string_lossy().to_string()),
                        "claudeVersion": claude.as_ref().and_then(|p| version(&p.to_string_lossy())),
                        "git": version("git"),
                        "node": version("node"),
                        "npx": lazyaicode_platform::which(if cfg!(windows) { "npx.cmd" } else { "npx" }).is_some(),
                        "codex": lazyaicode_platform::which(if cfg!(windows) { "codex.cmd" } else { "codex" }).is_some(),
                        "gemini": lazyaicode_platform::which(if cfg!(windows) { "gemini.cmd" } else { "gemini" }).is_some(),
                        "wsl": lazyaicode_platform::is_wsl(),
                        "dataDir": data,
                        "dataDirWritable": writable,
                    }))
                })
                .await
            }),
    );
    r.add(Spec::new("system.which", "Find a program", "system").params(vec![p::s("name")]).read_only().run(|_, v| async move {
        #[derive(Deserialize)]
        struct P {
            name: String,
        }
        let p: P = parse(v)?;
        if p.name.contains(['/', '\\']) {
            return Err(EngineError::BadParams("só o nome do programa".into()));
        }
        Ok(json!(lazyaicode_platform::which(&p.name).map(|x| x.to_string_lossy().to_string())))
    }));
    r.add(Spec::new("system.data-dir", "Data folder", "system").read_only().run(|e, _| async move { Ok(json!(e.core.data_dir)) }));
    r.add(Spec::new("system.home", "Home folder", "system").read_only().run(|_, _| async move { Ok(json!(claude::home())) }));
    r.add(
        Spec::new("system.open-url", "Open a web page", "system")
            .params(vec![p::s("url")])
            .doc("Abre um endereço http(s) no navegador padrão.")
            .run(|_, v| async move {
                #[derive(Deserialize)]
                struct P {
                    url: String,
                }
                let p: P = parse(v)?;
                if !(p.url.starts_with("https://") || p.url.starts_with("http://")) || p.url.contains(['\n', '\r', ' ']) {
                    return Err(EngineError::BadParams("só endereços http(s)".into()));
                }
                lazyaicode_platform::open_command(Path::new(&p.url)).spawn().map_err(|e| EngineError::Failed(e.to_string()))?;
                Ok(Value::Null)
            }),
    );
    r.add(
        Spec::new("system.session-file", "Claude session file", "system")
            .params(vec![p::s("id")])
            .read_only()
            .doc("Onde está o `.jsonl` da sessão do Claude Code de uma thread.")
            .run(|e, v| async move {
                let Id { id } = parse(v)?;
                let t = e.thread(&id)?;
                let Some(sid) = t.session_id.clone() else { return Ok(Value::Null) };
                Ok(json!(claude::session_file(&e.thread_claude_dir(&t), &t.cwd, &sid)))
            }),
    );
    r.add(
        Spec::new("system.session-raw", "Raw session messages", "system")
            .params(vec![p::s("id"), p::oi("last")])
            .read_only()
            .doc("As últimas mensagens brutas da sessão (para depuração).")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    last: Option<usize>,
                }
                let p: P = parse(v)?;
                let t = e.thread(&p.id)?;
                let Some(sid) = t.session_id.clone() else { return Ok(json!([])) };
                let f = claude::session_file(&e.thread_claude_dir(&t), &t.cwd, &sid);
                let all = blocking(move || Ok(transcript::load(&f))).await?;
                let n = p.last.unwrap_or(20).min(500);
                Ok(Value::Array(all.into_iter().rev().take(n).rev().collect()))
            }),
    );

    // ---------- terminais ----------
    for (id, label, bytes) in [
        ("terminal.interrupt", "Send Ctrl+C", "\u{3}"),
        ("terminal.eof", "Send Ctrl+D", "\u{4}"),
        ("terminal.clear", "Clear terminal", "\u{c}"),
        ("terminal.tab", "Send Tab", "\t"),
        ("terminal.enter", "Send Enter", "\r"),
    ] {
        r.add(Spec::new(id, label, "terminal").params(vec![p::s("termId")]).run(move |e, v| async move {
            let term = v.get("termId").and_then(Value::as_str).unwrap_or("").to_string();
            e.run("terminal.write", json!({ "termId": term, "data": bytes })).await
        }));
    }
    r.add(
        Spec::new("terminal.run", "Run a command line", "terminal")
            .params(vec![p::s("termId"), p::s("command")])
            .doc("Digita a linha e aperta Enter.")
            .run(|e, v| async move {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct P {
                    term_id: String,
                    command: String,
                }
                let p: P = parse(v)?;
                e.run("terminal.write", json!({ "termId": p.term_id, "data": format!("{}\r", p.command) })).await
            }),
    );

    // ---------- capabilities por tipo ----------
    for (id, label, field) in [
        ("capabilities.skills", "Skills and commands", "commands"),
        ("capabilities.agents", "Agents", "agents"),
        ("capabilities.mcp", "MCP servers", "mcp"),
        ("capabilities.models", "Models", "models"),
    ] {
        r.add(Spec::new(id, label, "capabilities").params(vec![p::s("workspaceId"), p::opath("path")]).read_only().run(
            move |e, v| async move {
                let c = e.run("capabilities.get", v).await?;
                Ok(c[field].clone())
            },
        ));
    }
    r.add(Spec::new("capabilities.refresh", "Forget cached capabilities", "capabilities").run(|e, _| async move {
        e.core.capabilities.lock().clear();
        Ok(Value::Null)
    }));

    // ---------- provedores ----------
    r.add(Spec::new("provider.list", "Providers", "provider").read_only().run(|_, _| async move {
        Ok(json!(lazyaicode_doc::PROVIDERS
            .iter()
            .map(|(id, label, vendor)| json!({ "id": id, "label": label, "vendor": vendor }))
            .collect::<Vec<_>>()))
    }));
    r.add(Spec::new("provider.forget", "Forget provider status", "provider").run(|e, _| async move {
        e.core.provider_status.lock().clear();
        Ok(Value::Null)
    }));

    // ---------- tabelas fixas ----------
    r.add(Spec::new("engine.models", "Claude Code models", "engine").read_only().run(|_, _| async move {
        Ok(json!(lazyaicode_doc::MODELS.iter().map(|(id, label)| json!({ "id": id, "label": label })).collect::<Vec<_>>()))
    }));
    r.add(Spec::new("engine.permission-modes", "Permission modes", "engine").read_only().run(|_, _| async move {
        Ok(json!(lazyaicode_doc::PERMISSION_MODES.iter().map(|(id, label)| json!({ "id": id, "label": label })).collect::<Vec<_>>()))
    }));
    r.add(Spec::new("engine.efforts", "Effort levels", "engine").read_only().run(|_, _| async move {
        Ok(json!(lazyaicode_doc::EFFORTS.iter().map(|(id, label)| json!({ "id": id, "label": label })).collect::<Vec<_>>()))
    }));
    r.add(Spec::new("engine.limits", "Protocol limits", "engine").read_only().run(|_, _| async move {
        Ok(json!({
            "maxRequestLine": lazyaicode_protocol::MAX_REQUEST_LINE,
            "maxReplyBytes": lazyaicode_protocol::MAX_REPLY_BYTES,
            "maxBatchSteps": lazyaicode_protocol::MAX_BATCH_STEPS,
            "maxConnections": lazyaicode_protocol::MAX_CONNECTIONS,
            "maxJsonDepth": lazyaicode_protocol::MAX_JSON_DEPTH,
        }))
    }));
    r.add(Spec::new("engine.journal-clear", "Clear the journal", "engine").run(|e, _| async move {
        *e.core.journal.lock() = Default::default();
        Ok(Value::Null)
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_markdown() {
        let mut t = Transcript::new();
        t.add_local_user("oi", vec![]);
        t.apply(&json!({"type":"assistant","message":{"id":"m","content":[{"type":"text","text":"olá"}]}}));
        let md = markdown(&t.items);
        assert!(md.contains("### Você\n\noi") && md.ends_with("olá"), "{md}");
    }
}

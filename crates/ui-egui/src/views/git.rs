//! O cliente git dentro da thread (ou da tela do repositório): Histórico
//! (grafo + mudanças locais com staging e commit), Árvore (visão geral com
//! zoom), Reflog e Estatísticas. Tudo por comandos `git.*` do motor, sobre o
//! checkout da thread.

use std::collections::HashMap;

use egui::{Align, Color32, Layout, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular as icon;
use lazyaicode_doc::diff::parse_patch;
use lazyaicode_doc::graph::{self, GraphRow};
use lazyaicode_i18n::{t, tf};
use serde_json::{json, Value};

use crate::app::{LazyApp, ToastKind};
use crate::views::widgets;

const PAGE: u64 = 200;
const POLL_SECS: f64 = 2.5;
const WORKING: &str = "\u{0}working";

#[derive(Default)]
pub struct Repo {
    pub id: Option<String>,
    opening: bool,
    pub error: Option<String>,
    pub status: Value,
    fingerprint: String,
    last_poll: f64,
    polling: bool,
    pub commits: Vec<Value>,
    pub refs: Vec<Value>,
    rows: Vec<GraphRow>,
    next_skip: Option<u64>,
    loading_log: bool,
    pub selected: Option<String>,
    commit_diff: Option<Value>,
    diff_file: Option<String>,
    pub working: Value,
    wdiff: [Value; 2],
    /// (staged?, caminho)
    wfile: Option<(bool, String)>,
    pub branches: Vec<Value>,
    pub tags: Vec<Value>,
    pub stashes: Vec<Value>,
    pub remotes: Vec<Value>,
    reflog: Vec<Value>,
    reflog_ref: String,
    stats: Value,
    pub summary: String,
    pub body: String,
    pub amend: bool,
    pub op: Option<String>,
    new_branch: Option<String>,
    new_tag: Option<(String, String)>,
    show_refs: bool,
    tree_view: Option<TreeOverview>,
}

/// Pedidos de senha/credenciais do git (ssh, https).
#[derive(Default)]
pub struct Prompts {
    ssh_key: Option<String>,
    https: Option<Value>,
    secret: String,
    user: String,
}

#[derive(Default)]
pub struct GitUi {
    pub repos: HashMap<String, Repo>,
    pub prompts: Prompts,
}

fn repo<'a>(app: &'a LazyApp, path: &str) -> Option<&'a Repo> {
    app.eph.git.repos.get(path)
}

fn repo_mut<'a>(app: &'a mut LazyApp, path: &str) -> &'a mut Repo {
    app.eph.git.repos.entry(path.to_string()).or_default()
}

pub fn current_repo_id(app: &LazyApp) -> Option<String> {
    app.current_path().and_then(|p| repo(app, &p)).and_then(|r| r.id.clone())
}

fn rid(app: &LazyApp, path: &str) -> Option<String> {
    repo(app, path).and_then(|r| r.id.clone())
}

/// Abre o repositório da pasta no motor (uma vez) e carrega tudo.
fn ensure(app: &mut LazyApp, path: &str) {
    let r = repo_mut(app, path);
    if r.id.is_some() || r.opening || r.error.is_some() {
        return;
    }
    r.opening = true;
    r.show_refs = true;
    let p = path.to_string();
    app.call("git.repo.open", json!({ "path": path }), move |app, res| {
        let r = repo_mut(app, &p);
        r.opening = false;
        match res {
            Ok(v) => {
                r.id = v["repoId"].as_str().map(str::to_string);
                refresh(app, &p, true);
            }
            Err(e) => r.error = Some(e.message),
        }
    });
}

/// Recarrega status, mudanças locais, log, branches e stash.
pub fn refresh(app: &mut LazyApp, path: &str, log: bool) {
    let Some(id) = rid(app, path) else { return };
    let p = path.to_string();
    let q = |m: &str| m.to_string();
    for (method, slot) in [
        (q("git.repo.status"), 0),
        (q("git.working.status"), 1),
        (q("git.branch.list"), 2),
        (q("git.tag.list"), 3),
        (q("git.stash.list"), 4),
        (q("git.remote.list"), 5),
    ] {
        let p2 = p.clone();
        app.call(&method, json!({ "repoId": id }), move |app, res| {
            let Ok(v) = res else { return };
            let r = repo_mut(app, &p2);
            match slot {
                0 => r.status = v,
                1 => r.working = v,
                2 => r.branches = v.as_array().cloned().unwrap_or_default(),
                3 => r.tags = v.as_array().cloned().unwrap_or_default(),
                4 => r.stashes = v.as_array().cloned().unwrap_or_default(),
                _ => r.remotes = v.as_array().cloned().unwrap_or_default(),
            }
        });
    }
    for staged in [false, true] {
        let p2 = p.clone();
        app.call("git.working.diff", json!({ "repoId": id, "staged": staged }), move |app, res| {
            if let Ok(v) = res {
                repo_mut(app, &p2).wdiff[usize::from(staged)] = v;
            }
        });
    }
    if log {
        load_log(app, path, 0);
    }
}

fn load_log(app: &mut LazyApp, path: &str, skip: u64) {
    let Some(id) = rid(app, path) else { return };
    let r = repo_mut(app, path);
    if r.loading_log {
        return;
    }
    r.loading_log = true;
    let p = path.to_string();
    let limit = if skip == 0 { PAGE.max(r.commits.len() as u64) } else { PAGE };
    app.call("git.log.list", json!({ "repoId": id, "query": { "skip": skip, "limit": limit, "branchFilter": null } }), move |app, res| {
        let r = repo_mut(app, &p);
        r.loading_log = false;
        match res {
            Ok(v) => {
                let commits = v["commits"].as_array().cloned().unwrap_or_default();
                if skip == 0 {
                    r.commits = commits;
                    r.refs = v["refs"].as_array().cloned().unwrap_or_default();
                } else {
                    r.commits.extend(commits);
                }
                r.next_skip = v["nextSkip"].as_u64();
                let pairs: Vec<(String, Vec<String>)> = r
                    .commits
                    .iter()
                    .map(|c| {
                        let parents = c["parents"]
                            .as_array()
                            .map(|ps| ps.iter().filter_map(|p| p.as_str().map(str::to_string)).collect())
                            .unwrap_or_default();
                        (c["sha"].as_str().unwrap_or("").to_string(), parents)
                    })
                    .collect();
                r.rows = graph::layout(&pairs);
                r.tree_view = None;
            }
            Err(e) => r.error = Some(e.message),
        }
    });
}

/// Confere a impressão digital do repositório e recarrega se mudou.
fn poll(app: &mut LazyApp, path: &str) {
    let now = app.now;
    let Some(id) = rid(app, path) else { return };
    let r = repo_mut(app, path);
    if r.polling || now - r.last_poll < POLL_SECS {
        return;
    }
    r.polling = true;
    r.last_poll = now;
    let p = path.to_string();
    app.call("git.repo.fingerprint", json!({ "repoId": id }), move |app, res| {
        let r = repo_mut(app, &p);
        r.polling = false;
        let fp = res.map(|v| v.to_string()).unwrap_or_default();
        if !fp.is_empty() && fp != r.fingerprint {
            let first = r.fingerprint.is_empty();
            r.fingerprint = fp;
            if !first {
                refresh(app, &p, true);
            }
        }
    });
}

pub fn refresh_current(app: &mut LazyApp) {
    if let Some(p) = app.current_path() {
        refresh(app, &p, true);
    }
}

pub fn on_turn_finished(app: &mut LazyApp, tid: &str) {
    if let Some(cwd) = app.data.thread(tid).map(|t| t.thread.cwd.clone()) {
        if repo(app, &cwd).is_some_and(|r| r.id.is_some()) {
            refresh(app, &cwd, true);
        }
    }
}

pub fn on_repo_changed(app: &mut LazyApp, p: &Value) {
    let id = p["repoId"].as_str().unwrap_or("");
    let path = app.eph.git.repos.iter().find(|(_, r)| r.id.as_deref() == Some(id)).map(|(p, _)| p.clone());
    if let Some(path) = path {
        refresh(app, &path, true);
    }
}

pub fn on_git_event(app: &mut LazyApp, name: &str, p: &Value) {
    match name {
        "ssh-passphrase-request" => {
            app.eph.git.prompts.ssh_key = Some(p.as_str().unwrap_or("").to_string());
            app.eph.git.prompts.secret.clear();
        }
        "https-credentials-request" => {
            app.eph.git.prompts.https = Some(p.clone());
            app.eph.git.prompts.secret.clear();
            app.eph.git.prompts.user = p["username"].as_str().unwrap_or("").to_string();
        }
        "activity://log" if p["level"] == "error" => {
            let msg = p["message"].as_str().unwrap_or("").to_string();
            if !msg.is_empty() {
                app.toast(msg, ToastKind::Error);
            }
        }
        "git://autostash-conflict" => app.toast(t("gitPanel.autostashConflict"), ToastKind::Error),
        _ => {}
    }
}

/// Uma operação que muda o repositório: marca ocupado, roda e recarrega.
fn op(app: &mut LazyApp, path: &str, label: &str, method: &str, params: Value, ok_msg: Option<String>) {
    let Some(id) = rid(app, path) else { return };
    let mut params = params;
    params["repoId"] = json!(id);
    repo_mut(app, path).op = Some(label.to_string());
    let p = path.to_string();
    app.call(method, params, move |app, res| {
        repo_mut(app, &p).op = None;
        match res {
            Ok(_) => {
                if let Some(m) = ok_msg {
                    app.toast(m, ToastKind::Ok);
                }
            }
            Err(e) => {
                let detail = e.data.as_ref().and_then(|d| d["details"]["output"].as_str()).map(|o| format!("\n{o}")).unwrap_or_default();
                app.toast_err(&format!("{}{detail}", e.message));
            }
        }
        refresh(app, &p, true);
    });
}

fn current_branch(app: &LazyApp, path: &str) -> Option<String> {
    repo(app, path).and_then(|r| r.status["currentBranch"].as_str().map(str::to_string))
}

// ---------- comandos de tela ----------

fn path_of(app: &LazyApp) -> Result<String, String> {
    let p = app.current_path().ok_or_else(|| t("gitPanel.noRepo"))?;
    rid(app, &p).ok_or_else(|| t("gitPanel.noRepo"))?;
    Ok(p)
}

/// Quem registra um comando de tela: `(id, rótulo, grupo, atalho, run)`.
pub type AddUiCmd<'a> = dyn FnMut(&str, &str, &'static str, Option<&str>, crate::commands::RunBox) + 'a;

pub fn register_ui_commands(add: &mut AddUiCmd<'_>) {
    crate::views::git_extra::register(add);
    let mut a = |id: &str, label: &str, sc: Option<&str>, f: crate::commands::RunBox| add(id, label, "git", sc, f);
    a(
        "ui.git.refresh",
        "gitPanel.refresh",
        None,
        Box::new(|app, _| {
            let p = path_of(app)?;
            refresh(app, &p, true);
            Ok(Value::Null)
        }),
    );
    a(
        "ui.git.fetch",
        "gitRemote.fetch",
        Some("Ctrl+Shift+F"),
        Box::new(|app, _| {
            let p = path_of(app)?;
            op(app, &p, "fetch", "git.remote.fetch", json!({}), Some(t("gitRemote.fetched")));
            Ok(Value::Null)
        }),
    );
    a(
        "ui.git.pull",
        "gitRemote.pull",
        Some("Ctrl+Shift+L"),
        Box::new(|app, _| {
            let p = path_of(app)?;
            op(app, &p, "pull", "git.pull.pull", json!({}), Some(t("gitRemote.pulled")));
            Ok(Value::Null)
        }),
    );
    for (id, force) in [("ui.git.push", false), ("ui.git.force-push", true)] {
        a(
            id,
            if force { "gitRemote.forcePush" } else { "gitRemote.push" },
            if force { None } else { Some("Ctrl+Shift+U") },
            Box::new(move |app, _| {
                let p = path_of(app)?;
                let b = current_branch(app, &p).ok_or_else(|| t("gitRemote.detached"))?;
                op(app, &p, "push", "git.remote.push", json!({ "branch": b, "force": force }), Some(t("gitRemote.pushed")));
                Ok(Value::Null)
            }),
        );
    }
    for (id, method, label) in [
        ("ui.git.stage-all", "git.working.stage-all", "gitWorking.stageAll"),
        ("ui.git.unstage-all", "git.working.unstage-all", "gitWorking.unstageAll"),
        ("ui.git.undo", "git.undo.undo", "gitUndo.undo"),
        ("ui.git.redo", "git.undo.redo", "gitUndo.redo"),
        ("ui.git.rebase-continue", "git.rebase.continue", "gitRebase.continue"),
        ("ui.git.rebase-abort", "git.rebase.abort", "gitRebase.abort"),
        ("ui.git.worktree-prune", "git.worktree.prune", "gitWorktrees.prune"),
    ] {
        a(
            id,
            label,
            None,
            Box::new(move |app, _| {
                let p = path_of(app)?;
                op(app, &p, label, method, json!({}), None);
                Ok(Value::Null)
            }),
        );
    }
    a(
        "ui.git.discard-all",
        "gitWorking.discardAll",
        None,
        Box::new(|app, _| {
            let p = path_of(app)?;
            let id = rid(app, &p).unwrap_or_default();
            app.eph.confirm = Some((t("gitWorking.confirmDiscardAll"), "git.working.discard-all".into(), json!({ "repoId": id })));
            app.state.dialog = Some("confirm".into());
            Ok(Value::Null)
        }),
    );
    a(
        "ui.git.commit",
        "gitWorking.commit",
        None,
        Box::new(|app, _| {
            let p = path_of(app)?;
            commit(app, &p);
            Ok(Value::Null)
        }),
    );
    a(
        "ui.git.amend.toggle",
        "gitWorking.amend",
        None,
        Box::new(|app, _| {
            let p = path_of(app)?;
            let r = repo_mut(app, &p);
            r.amend = !r.amend;
            Ok(Value::Null)
        }),
    );
    a(
        "ui.git.ai-message",
        "gitAi.generate",
        Some("Ctrl+Shift+M"),
        Box::new(|app, _| {
            let p = path_of(app)?;
            ai_message(app, &p);
            Ok(Value::Null)
        }),
    );
    a(
        "ui.git.stash",
        "gitStash.save",
        None,
        Box::new(|app, p| {
            let path = path_of(app)?;
            let msg = p.get("message").and_then(Value::as_str).map(str::to_string);
            op(app, &path, "stash", "git.stash.save", json!({ "message": msg, "includeUntracked": true }), Some(t("gitStash.saved")));
            Ok(Value::Null)
        }),
    );
    for (id, method, label) in [
        ("ui.git.stash-pop", "git.stash.pop", "gitStash.pop"),
        ("ui.git.stash-apply", "git.stash.apply", "gitStash.apply"),
        ("ui.git.stash-drop", "git.stash.drop", "gitStash.drop"),
    ] {
        a(
            id,
            label,
            None,
            Box::new(move |app, p| {
                let path = path_of(app)?;
                let index = p.get("index").and_then(Value::as_u64).unwrap_or(0);
                op(app, &path, label, method, json!({ "index": index }), None);
                Ok(Value::Null)
            }),
        );
    }
    a(
        "ui.git.new-branch",
        "gitBranches.newBranch",
        Some("Ctrl+Shift+B"),
        Box::new(|app, p| {
            let path = path_of(app)?;
            match p.get("name").and_then(Value::as_str) {
                Some(name) => op(app, &path, "branch", "git.branch.create", json!({ "name": name, "checkout": true }), None),
                None => repo_mut(app, &path).new_branch = Some(String::new()),
            }
            Ok(Value::Null)
        }),
    );
    a(
        "ui.git.checkout",
        "gitBranches.checkout",
        None,
        Box::new(|app, p| {
            let path = path_of(app)?;
            let name = p.get("name").and_then(Value::as_str).ok_or("falta `name`")?.to_string();
            checkout(app, &path, &name);
            Ok(Value::Null)
        }),
    );
    a(
        "ui.git.merge",
        "gitBranches.merge",
        None,
        Box::new(|app, p| {
            let path = path_of(app)?;
            let name = p.get("name").and_then(Value::as_str).ok_or("falta `name`")?.to_string();
            op(app, &path, "merge", "git.branch.merge", json!({ "name": name }), None);
            Ok(Value::Null)
        }),
    );
    a(
        "ui.git.rebase-onto",
        "gitBranches.rebaseOnto",
        None,
        Box::new(|app, p| {
            let path = path_of(app)?;
            let name = p.get("name").and_then(Value::as_str).ok_or("falta `name`")?.to_string();
            op(app, &path, "rebase", "git.rebase.start", json!({ "ontoBranch": name }), None);
            Ok(Value::Null)
        }),
    );
    a(
        "ui.git.delete-branch",
        "gitBranches.delete",
        None,
        Box::new(|app, p| {
            let path = path_of(app)?;
            let name = p.get("name").and_then(Value::as_str).ok_or("falta `name`")?.to_string();
            let id = rid(app, &path).unwrap_or_default();
            app.eph.confirm = Some((
                tf("gitBranches.confirmDelete", &[("name", &name)]),
                "git.branch.delete-local".into(),
                json!({ "repoId": id, "name": name }),
            ));
            app.state.dialog = Some("confirm".into());
            Ok(Value::Null)
        }),
    );
    a(
        "ui.git.select-working",
        "gitWorking.title",
        None,
        Box::new(|app, _| {
            let path = path_of(app)?;
            repo_mut(app, &path).selected = Some(WORKING.into());
            Ok(Value::Null)
        }),
    );
    a(
        "ui.git.select-commit",
        "Select commit",
        None,
        Box::new(|app, p| {
            let path = path_of(app)?;
            let sha = p.get("sha").and_then(Value::as_str).ok_or("falta `sha`")?.to_string();
            select_commit(app, &path, &sha);
            Ok(Value::Null)
        }),
    );
    a(
        "ui.git.load-more",
        "gitGraph.loadMore",
        None,
        Box::new(|app, _| {
            let path = path_of(app)?;
            if let Some(s) = repo(app, &path).and_then(|r| r.next_skip) {
                load_log(app, &path, s);
            }
            Ok(Value::Null)
        }),
    );
    a(
        "ui.git.toggle-refs",
        "gitPanel.toggleRefs",
        None,
        Box::new(|app, _| {
            let path = path_of(app)?;
            let r = repo_mut(app, &path);
            r.show_refs = !r.show_refs;
            Ok(Value::Null)
        }),
    );
    // ações sobre o commit selecionado (os menus de contexto usam as mesmas)
    let commit_cmds: [(&str, &str, &'static str); 8] = [
        ("ui.git.commit.copy-sha", "gitGraph.copySha", ""),
        ("ui.git.commit.checkout", "gitGraph.checkoutDetached", "git.branch.checkout-detached"),
        ("ui.git.commit.cherry-pick", "gitGraph.cherryPick", "git.commit.cherry-pick"),
        ("ui.git.commit.revert", "gitGraph.revert", "git.commit.revert"),
        ("ui.git.commit.reset-mixed", "gitGraph.resetMixed", "git.reset.to-commit"),
        ("ui.git.commit.reset-hard", "gitGraph.resetHard", "git.reset.to-commit"),
        ("ui.git.commit.branch-here", "gitGraph.branchHere", ""),
        ("ui.git.commit.tag-here", "gitGraph.tagHere", ""),
    ];
    for (id, label, method) in commit_cmds {
        a(
            id,
            label,
            None,
            Box::new(move |app, p| {
                let path = path_of(app)?;
                let sha = p
                    .get("sha")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| repo(app, &path).and_then(|r| r.selected.clone()).filter(|s| s != WORKING))
                    .ok_or_else(|| t("gitGraph.selectCommit"))?;
                commit_action(app, &path, id, method, &sha);
                Ok(Value::Null)
            }),
        );
    }
}

fn commit_action(app: &mut LazyApp, path: &str, id: &str, method: &str, sha: &str) {
    match id {
        "ui.git.commit.copy-sha" => {
            app.eph.copy_request = Some(sha.to_string());
            app.toast(t("app.copied"), ToastKind::Ok);
        }
        "ui.git.commit.branch-here" => repo_mut(app, path).new_branch = Some(String::new()),
        "ui.git.commit.tag-here" => repo_mut(app, path).new_tag = Some((sha.to_string(), String::new())),
        "ui.git.commit.cherry-pick" => op(app, path, "cherry-pick", method, json!({ "sha": sha, "createCommit": true }), None),
        "ui.git.commit.revert" => op(app, path, "revert", method, json!({ "sha": sha }), None),
        "ui.git.commit.reset-mixed" => op(app, path, "reset", method, json!({ "sha": sha, "hard": false }), None),
        "ui.git.commit.reset-hard" => {
            let rid = rid(app, path).unwrap_or_default();
            app.eph.confirm = Some((
                tf("gitGraph.confirmResetHard", &[("sha", &sha.chars().take(8).collect::<String>())]),
                method.into(),
                json!({ "repoId": rid, "sha": sha, "hard": true }),
            ));
            app.state.dialog = Some("confirm".into());
        }
        _ => op(app, path, "checkout", method, json!({ "sha": sha }), None),
    }
}

fn checkout(app: &mut LazyApp, path: &str, name: &str) {
    if let Some(remote) = name
        .split_once('/')
        .filter(|_| repo(app, path).is_some_and(|r| r.branches.iter().any(|b| b["name"] == name && b["isRemote"] == true)))
    {
        // branch remota: cria a local que a acompanha
        let local = remote.1.to_string();
        let sha = repo(app, path)
            .and_then(|r| r.branches.iter().find(|b| b["name"] == name).and_then(|b| b["targetSha"].as_str().map(str::to_string)))
            .unwrap_or_default();
        op(app, path, "checkout", "git.branch.checkout-at", json!({ "name": local, "targetSha": sha, "remoteName": remote.0 }), None);
    } else {
        op(app, path, "checkout", "git.branch.checkout", json!({ "name": name }), None);
    }
}

fn select_commit(app: &mut LazyApp, path: &str, sha: &str) {
    let Some(id) = rid(app, path) else { return };
    let r = repo_mut(app, path);
    r.selected = Some(sha.to_string());
    r.commit_diff = None;
    r.diff_file = None;
    let (p, s) = (path.to_string(), sha.to_string());
    app.call("git.diff.commit", json!({ "repoId": id, "sha": sha }), move |app, res| {
        let r = repo_mut(app, &p);
        if r.selected.as_deref() == Some(s.as_str()) {
            r.commit_diff = Some(res.unwrap_or_else(|e| json!({ "error": e.message })));
            r.diff_file = r.commit_diff.as_ref().and_then(|d| d["files"][0]["path"].as_str().map(str::to_string));
        }
    });
}

fn commit(app: &mut LazyApp, path: &str) {
    let Some(r) = repo(app, path) else { return };
    let summary = r.summary.trim().to_string();
    if summary.is_empty() {
        app.toast_err(&t("gitWorking.needSummary"));
        return;
    }
    let (body, amend) = (r.body.trim().to_string(), r.amend);
    let staged = r.working["staged"].as_array().map_or(0, Vec::len);
    if staged == 0 && !amend {
        app.toast_err(&t("gitWorking.nothingStaged"));
        return;
    }
    let Some(id) = r.id.clone() else { return };
    repo_mut(app, path).op = Some("commit".into());
    let p = path.to_string();
    app.call("git.working.commit", json!({ "repoId": id, "summary": summary, "body": body, "amend": amend }), move |app, res| {
        let r = repo_mut(app, &p);
        r.op = None;
        match res {
            Ok(_) => {
                r.summary.clear();
                r.body.clear();
                r.amend = false;
                app.toast(t("gitWorking.committed"), ToastKind::Ok);
            }
            Err(e) => {
                let out = e.data.as_ref().and_then(|d| d["details"]["output"].as_str()).map(|o| format!("\n{o}")).unwrap_or_default();
                app.toast_err(&format!("{}{out}", e.message));
            }
        }
        refresh(app, &p, true);
    });
}

fn ai_message(app: &mut LazyApp, path: &str) {
    let Some(r) = repo(app, path) else { return };
    let diff: String = r.wdiff[1]["files"]
        .as_array()
        .map(|f| f.iter().filter_map(|x| x["patch"].as_str()).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();
    if diff.trim().is_empty() {
        app.toast_err(&t("gitWorking.nothingStaged"));
        return;
    }
    let diff: String = diff.chars().take(120_000).collect();
    let model = app.data.prefs.get("git.aiModel").and_then(Value::as_str).unwrap_or("haiku").to_string();
    let instructions = app.data.prefs.get("git.aiInstructions").and_then(Value::as_str).unwrap_or("").to_string();
    repo_mut(app, path).op = Some("ai".into());
    let p = path.to_string();
    app.call("ai.commit-message", json!({ "model": model, "diff": diff, "customInstructions": instructions }), move |app, res| {
        let r = repo_mut(app, &p);
        r.op = None;
        match res {
            Ok(v) => {
                r.summary = v["summary"].as_str().unwrap_or("").to_string();
                r.body = v["description"].as_str().unwrap_or("").to_string();
            }
            Err(e) => app.toast_err(&e.message),
        }
    });
}

// ---------- telas ----------

fn needs_repo(app: &mut LazyApp, ui: &mut egui::Ui, path: &str) -> bool {
    let tk = app.tokens;
    ensure(app, path);
    poll(app, path);
    match repo(app, path) {
        Some(r) if r.error.is_some() => {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(r.error.clone().unwrap_or_default()).color(tk.p.red));
            });
            if ui.button(t("gitPanel.retry")).clicked() {
                repo_mut(app, path).error = None;
            }
            false
        }
        Some(r) if r.id.is_some() => true,
        _ => {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| ui.spinner());
            false
        }
    }
}

fn toolbar(app: &mut LazyApp, ui: &mut egui::Ui, path: &str) {
    let tk = app.tokens;
    let Some(r) = repo(app, path) else { return };
    let branch = r.status["currentBranch"].as_str().map(str::to_string);
    let (ahead, behind) = (r.status["ahead"].as_u64().unwrap_or(0), r.status["behind"].as_u64().unwrap_or(0));
    let busy = r.op.clone();
    let in_progress = r.status["inProgressOp"].as_str().map(str::to_string);
    let locals: Vec<String> =
        r.branches.iter().filter(|b| b["isRemote"] != true).filter_map(|b| b["name"].as_str().map(str::to_string)).collect();
    let stashes = r.stashes.len();
    egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 4)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let label = branch.clone().unwrap_or_else(|| t("gitPanel.detached"));
            let br = ui.add(egui::Button::new(RichText::new(format!("{}  {label}  {}", icon::GIT_BRANCH, icon::CARET_DOWN)).strong()));
            egui::Popup::menu(&br).show(|ui| {
                ui.set_min_width(240.0);
                if ui.button(format!("{}  {}", icon::PLUS, t("gitBranches.newBranch"))).clicked() {
                    repo_mut(app, path).new_branch = Some(String::new());
                }
                ui.separator();
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for b in &locals {
                        if ui.selectable_label(Some(b) == branch.as_ref(), b).clicked() && Some(b) != branch.as_ref() {
                            checkout(app, path, b);
                        }
                    }
                });
            });
            ui.separator();
            let tb = |ui: &mut egui::Ui, ic: &str, text: String, tip: String| {
                ui.add_enabled(busy.is_none(), egui::Button::new(format!("{ic} {text}"))).on_hover_text(tip).clicked()
            };
            if tb(ui, icon::ARROWS_DOWN_UP, t("gitRemote.fetch"), "Ctrl+Shift+F".into()) {
                let _ = crate::commands::run_ui(app, "ui.git.fetch", &Value::Null);
            }
            let pull = if behind > 0 { format!("{} ↓{behind}", t("gitRemote.pull")) } else { t("gitRemote.pull") };
            if tb(ui, icon::ARROW_DOWN, pull, "Ctrl+Shift+L".into()) {
                let _ = crate::commands::run_ui(app, "ui.git.pull", &Value::Null);
            }
            let push = if ahead > 0 { format!("{} ↑{ahead}", t("gitRemote.push")) } else { t("gitRemote.push") };
            if tb(ui, icon::ARROW_UP, push, "Ctrl+Shift+U".into()) {
                let _ = crate::commands::run_ui(app, "ui.git.push", &Value::Null);
            }
            ui.separator();
            if tb(ui, icon::ARCHIVE_BOX, t("gitStash.save"), t("gitStash.saveHint")) {
                let _ = crate::commands::run_ui(app, "ui.git.stash", &Value::Null);
            }
            if stashes > 0 && tb(ui, icon::TRAY_ARROW_UP, format!("{} ({stashes})", t("gitStash.pop")), String::new()) {
                let _ = crate::commands::run_ui(app, "ui.git.stash-pop", &json!({ "index": 0 }));
            }
            if tb(ui, icon::ARROW_COUNTER_CLOCKWISE, String::new(), t("gitUndo.undo")) {
                let _ = crate::commands::run_ui(app, "ui.git.undo", &Value::Null);
            }
            if let Some(op) = &busy {
                ui.spinner();
                ui.label(widgets::muted(&tk, op));
            }
            if let Some(ipo) = in_progress {
                widgets::chip(ui, &tk, &format!("{} {ipo}", icon::WARNING), tk.p.amber);
                if ui.small_button(t("gitRebase.continue")).clicked() {
                    let _ = crate::commands::run_ui(app, "ui.git.rebase-continue", &Value::Null);
                }
                if ui.small_button(t("gitRebase.abort")).clicked() {
                    let _ = crate::commands::run_ui(app, "ui.git.rebase-abort", &Value::Null);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let more = widgets::icon_button(ui, &tk, icon::DOTS_THREE, &t("gitPanel.more"));
                egui::Popup::menu(&more).show(|ui| {
                    for (id, ic, label) in [
                        ("ui.git.search", icon::MAGNIFYING_GLASS, "gitGraph.search"),
                        ("ui.git.open-pr", icon::GIT_PULL_REQUEST, "gitIntegrations.newPr"),
                        ("ui.git.remotes", icon::CLOUD, "gitRemote.remotes"),
                        ("ui.git.worktrees", icon::TREE_STRUCTURE, "gitWorktrees.title"),
                        ("ui.git.submodules", icon::CUBE, "gitSubmodules.title"),
                        ("ui.git.lfs-pull", icon::DOWNLOAD_SIMPLE, "gitLfs.pull"),
                        ("ui.git.redo", icon::ARROW_CLOCKWISE, "gitUndo.redo"),
                    ] {
                        if ui.button(format!("{ic}  {}", t(label))).clicked() {
                            if let Err(e) = crate::commands::run_ui(app, id, &Value::Null) {
                                app.toast_err(&e);
                            }
                            ui.close();
                        }
                    }
                });
                if widgets::icon_button(ui, &tk, icon::ARROWS_CLOCKWISE, &t("gitPanel.refresh")).clicked() {
                    refresh(app, path, true);
                }
                let on = repo(app, path).is_some_and(|r| r.show_refs);
                if widgets::icon_button_on(ui, &tk, icon::SIDEBAR, &t("gitPanel.toggleRefs"), on).clicked() {
                    repo_mut(app, path).show_refs = !on;
                }
            });
        });
    });
    widgets::hline(ui, &tk);
    name_dialogs(app, ui, path);
}

fn name_dialogs(app: &mut LazyApp, ui: &mut egui::Ui, path: &str) {
    let tk = app.tokens;
    let start = repo(app, path).and_then(|r| r.selected.clone()).filter(|s| s != WORKING);
    if let Some(mut name) = repo(app, path).and_then(|r| r.new_branch.clone()) {
        let mut close = false;
        egui::Modal::new(egui::Id::new(("new-branch", path))).show(ui.ctx(), |ui| {
            ui.set_width(360.0);
            ui.heading(t("gitBranches.newBranch"));
            if let Some(s) = &start {
                ui.label(widgets::muted(&tk, tf("gitBranches.fromCommit", &[("sha", &s.chars().take(8).collect::<String>())])));
            }
            let r = ui.add(egui::TextEdit::singleline(&mut name).hint_text("feat/minha-branch").desired_width(f32::INFINITY));
            r.request_focus();
            ui.horizontal(|ui| {
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (widgets::primary(ui, &tk, &t("gitBranches.create")).clicked() || enter) && !name.trim().is_empty() {
                    op(
                        app,
                        path,
                        "branch",
                        "git.branch.create",
                        json!({ "name": name.trim(), "startPoint": start, "checkout": true }),
                        None,
                    );
                    close = true;
                }
                if ui.button(t("app.cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close = true;
                }
            });
        });
        repo_mut(app, path).new_branch = if close { None } else { Some(name) };
    }
    if let Some((sha, mut name)) = repo(app, path).and_then(|r| r.new_tag.clone()) {
        let mut close = false;
        egui::Modal::new(egui::Id::new(("new-tag", path))).show(ui.ctx(), |ui| {
            ui.set_width(360.0);
            ui.heading(t("gitGraph.tagHere"));
            let r = ui.add(egui::TextEdit::singleline(&mut name).hint_text("v1.0.0").desired_width(f32::INFINITY));
            r.request_focus();
            ui.horizontal(|ui| {
                if widgets::primary(ui, &tk, &t("gitBranches.create")).clicked() && !name.trim().is_empty() {
                    op(app, path, "tag", "git.tag.create", json!({ "name": name.trim(), "sha": sha }), None);
                    close = true;
                }
                if ui.button(t("app.cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close = true;
                }
            });
        });
        repo_mut(app, path).new_tag = if close { None } else { Some((sha, name)) };
    }
}

pub fn lane_color(tk: &crate::theme::Tokens, i: usize) -> Color32 {
    const PAL: [&str; 10] = ["#f2c14e", "#d7263d", "#9d4edd", "#f77f00", "#2a9d8f", "#3a86ff", "#e76f51", "#8ac926", "#ff006e", "#06d6a0"];
    const PAL_LIGHT: [&str; 10] =
        ["#b7791f", "#c81d34", "#7b2cbf", "#c2410c", "#1f766b", "#1d4ed8", "#b9472c", "#4d7c0f", "#be185d", "#047857"];
    let pal = if tk.mode == crate::theme::Mode::Dark { PAL } else { PAL_LIGHT };
    crate::theme::hex(pal.get(i % pal.len()).copied().unwrap_or("#888888"))
}

pub fn history(app: &mut LazyApp, ui: &mut egui::Ui, path: &str) {
    if !needs_repo(app, ui, path) {
        return;
    }
    let tk = app.tokens;
    toolbar(app, ui, path);
    if repo(app, path).is_some_and(|r| r.show_refs) {
        egui::Panel::left(egui::Id::new(("git-refs", path)))
            .default_size(220.0)
            .resizable(true)
            .frame(egui::Frame::new().fill(tk.p.panel).inner_margin(egui::Margin::same(6)))
            .show(ui, |ui| refs_panel(app, ui, path));
    }
    let selected = repo(app, path).and_then(|r| r.selected.clone());
    egui::Panel::bottom(egui::Id::new(("git-details", path)))
        .default_size(ui.available_height() * 0.5)
        .resizable(true)
        .frame(egui::Frame::new().fill(tk.p.bg))
        .show(ui, |ui| {
            widgets::hline(ui, &tk);
            let op = crate::views::git_extra::in_progress(app, path);
            match (op, selected.as_deref()) {
                (Some(op), Some(WORKING) | None) => {
                    egui::ScrollArea::vertical()
                        .id_salt(("conflicts", path))
                        .auto_shrink([false, false])
                        .show(ui, |ui| crate::views::git_extra::conflicts(app, ui, path, &op));
                }
                (_, Some(WORKING) | None) => working(app, ui, path),
                (_, Some(sha)) => commit_details(app, ui, path, sha),
            }
        });
    egui::CentralPanel::default().frame(egui::Frame::new().fill(tk.p.bg)).show(ui, |ui| commit_list(app, ui, path));
}

fn refs_panel(app: &mut LazyApp, ui: &mut egui::Ui, path: &str) {
    let tk = app.tokens;
    let Some(r) = repo(app, path) else { return };
    let head = r.status["currentBranch"].as_str().map(str::to_string);
    let locals: Vec<Value> = r.branches.iter().filter(|b| b["isRemote"] != true).cloned().collect();
    let remotes: Vec<Value> = r.branches.iter().filter(|b| b["isRemote"] == true).cloned().collect();
    let tags = r.tags.clone();
    let stashes = r.stashes.clone();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::CollapsingHeader::new(RichText::new(format!("{}  {} ({})", icon::GIT_BRANCH, t("gitBranches.local"), locals.len())).strong())
            .default_open(true)
            .show(ui, |ui| {
                for b in &locals {
                    let name = b["name"].as_str().unwrap_or("").to_string();
                    let is_head = Some(&name) == head.as_ref();
                    let ahead = b["ahead"].as_u64().filter(|a| *a > 0).map(|a| format!("  ↑{a}")).unwrap_or_default();
                    let r = ui.selectable_label(
                        is_head,
                        RichText::new(format!("{name}{ahead}")).color(if is_head { tk.p.accent } else { tk.p.text }),
                    );
                    if r.double_clicked() && !is_head {
                        checkout(app, path, &name);
                    }
                    if r.clicked() {
                        if let Some(sha) = b["targetSha"].as_str() {
                            select_commit(app, path, sha);
                        }
                    }
                    r.context_menu(|ui| branch_menu(app, ui, path, &name, is_head, false));
                }
            });
        egui::CollapsingHeader::new(RichText::new(format!("{}  {} ({})", icon::CLOUD, t("gitBranches.remote"), remotes.len())).strong())
            .default_open(false)
            .show(ui, |ui| {
                for b in &remotes {
                    let name = b["name"].as_str().unwrap_or("").to_string();
                    let r = ui.selectable_label(false, &name);
                    if r.double_clicked() {
                        checkout(app, path, &name);
                    }
                    if r.clicked() {
                        if let Some(sha) = b["targetSha"].as_str() {
                            select_commit(app, path, sha);
                        }
                    }
                    r.context_menu(|ui| branch_menu(app, ui, path, &name, false, true));
                }
            });
        egui::CollapsingHeader::new(RichText::new(format!("{}  {} ({})", icon::TAG, t("gitBranches.tags"), tags.len())).strong())
            .default_open(false)
            .show(ui, |ui| {
                for tg in &tags {
                    let name = tg["name"].as_str().unwrap_or("");
                    if ui.selectable_label(false, name).clicked() {
                        if let Some(sha) = tg["targetSha"].as_str().or_else(|| tg["sha"].as_str()) {
                            select_commit(app, path, sha);
                        }
                    }
                }
            });
        egui::CollapsingHeader::new(RichText::new(format!("{}  {} ({})", icon::ARCHIVE_BOX, t("gitStash.title"), stashes.len())).strong())
            .default_open(!stashes.is_empty())
            .show(ui, |ui| {
                for (i, s) in stashes.iter().enumerate() {
                    let msg = s["message"].as_str().unwrap_or("stash").to_string();
                    let r = ui.selectable_label(false, format!("{{{i}}} {msg}"));
                    r.context_menu(|ui| {
                        for (id, label) in [
                            ("ui.git.stash-apply", "gitStash.apply"),
                            ("ui.git.stash-pop", "gitStash.pop"),
                            ("ui.git.stash-drop", "gitStash.drop"),
                        ] {
                            if ui.button(t(label)).clicked() {
                                let _ = crate::commands::run_ui(app, id, &json!({ "index": i }));
                                ui.close();
                            }
                        }
                    });
                }
            });
    });
}

fn branch_menu(app: &mut LazyApp, ui: &mut egui::Ui, path: &str, name: &str, is_head: bool, remote: bool) {
    if !is_head && ui.button(format!("{}  {}", icon::CHECK, t("gitBranches.checkout"))).clicked() {
        checkout(app, path, name);
        ui.close();
    }
    if !is_head && ui.button(format!("{}  {}", icon::GIT_MERGE, tf("gitBranches.mergeInto", &[("name", &name)]))).clicked() {
        let _ = crate::commands::run_ui(app, "ui.git.merge", &json!({ "name": name }));
        ui.close();
    }
    if !is_head && ui.button(format!("{}  {}", icon::GIT_PULL_REQUEST, tf("gitBranches.rebaseOntoName", &[("name", &name)]))).clicked() {
        let _ = crate::commands::run_ui(app, "ui.git.rebase-onto", &json!({ "name": name }));
        ui.close();
    }
    if !is_head && ui.button(format!("{}  {}", icon::GIT_DIFF, t("gitBranches.compareWithCurrent"))).clicked() {
        let _ = crate::commands::run_ui(app, "ui.git.compare", &json!({ "b": name }));
        ui.close();
    }
    if ui.button(format!("{}  {}", icon::COPY, t("gitBranches.copyName"))).clicked() {
        app.eph.copy_request = Some(name.to_string());
        ui.close();
    }
    if !remote && !is_head {
        ui.separator();
        if ui.button(RichText::new(format!("{}  {}", icon::TRASH, t("gitBranches.delete"))).color(app.tokens.p.red)).clicked() {
            let _ = crate::commands::run_ui(app, "ui.git.delete-branch", &json!({ "name": name }));
            ui.close();
        }
    }
}

fn commit_list(app: &mut LazyApp, ui: &mut egui::Ui, path: &str) {
    let tk = app.tokens;
    let Some(r) = repo(app, path) else { return };
    let dirty = r.working["staged"].as_array().map_or(0, Vec::len) + r.working["unstaged"].as_array().map_or(0, Vec::len);
    let commits = r.commits.clone();
    let rows = r.rows.clone();
    let mut refs: HashMap<String, Vec<(String, String, bool)>> = HashMap::new();
    for rf in &r.refs {
        let sha = rf["targetSha"].as_str().unwrap_or("").to_string();
        let kind = rf["kind"].as_str().unwrap_or("").to_string();
        let name = rf["name"].as_str().unwrap_or("").to_string();
        refs.entry(sha).or_default().push((name, kind, rf["isHead"] == true));
    }
    let selected = r.selected.clone();
    let next = r.next_skip;
    let loading = r.loading_log;
    let row_h = tk.row_h;
    let lane_w = 14.0;
    let max_lanes = rows.iter().map(|r| r.width).max().unwrap_or(1).min(graph::MAX_LANES) as f32;
    let graph_w = (max_lanes * lane_w + 8.0).min(260.0);
    let extra = usize::from(dirty > 0) + usize::from(next.is_some());
    let total = commits.len() + extra;
    let now = app.host.now_ms() / 1000;
    let mut click: Option<String> = None;
    let mut menu_action: Option<(&'static str, String)> = None;
    egui::ScrollArea::vertical().id_salt(("commits", path)).auto_shrink([false, false]).show_rows(ui, row_h, total, |ui, range| {
        for i in range {
            let w = ui.available_width();
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, row_h), Sense::click());
            if dirty > 0 && i == 0 {
                let on = selected.as_deref().is_none_or(|s| s == WORKING);
                if on {
                    ui.painter().rect_filled(rect, 0.0, tk.p.active);
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, 0.0, tk.p.hover);
                }
                let c = egui::pos2(rect.left() + 4.0 + lane_w / 2.0, rect.center().y);
                ui.painter().circle_stroke(c, 4.5, Stroke::new(1.5, tk.p.amber));
                ui.painter().text(
                    egui::pos2(rect.left() + graph_w, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    tf("gitWorking.uncommitted", &[("count", &dirty)]),
                    egui::FontId::proportional(tk.text_size),
                    tk.p.amber,
                );
                if resp.clicked() {
                    click = Some(WORKING.into());
                }
                continue;
            }
            let ci = i - usize::from(dirty > 0);
            let Some(c) = commits.get(ci) else {
                // última linha: carregar mais
                if !loading {
                    if let Some(s) = next {
                        load_log(app, path, s);
                    }
                }
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    t("thread.loading"),
                    egui::FontId::proportional(tk.text_size - 1.0),
                    tk.p.faint,
                );
                continue;
            };
            let sha = c["sha"].as_str().unwrap_or("").to_string();
            let on = selected.as_deref() == Some(sha.as_str());
            if on {
                ui.painter().rect_filled(rect, 0.0, tk.p.active);
            } else if resp.hovered() {
                ui.painter().rect_filled(rect, 0.0, tk.p.hover);
            }
            let p = ui.painter_at(rect);
            if let Some(gr) = rows.get(ci) {
                let x = |lane: usize| rect.left() + 4.0 + lane as f32 * lane_w + lane_w / 2.0;
                let (top, mid, bot) = (rect.top(), rect.center().y, rect.bottom());
                for s in &gr.segments {
                    if s.from.max(s.to) >= graph::MAX_LANES {
                        continue;
                    }
                    let a = egui::pos2(x(s.from), if s.from_mid { mid } else { top });
                    let b = egui::pos2(x(s.to), if s.to_mid { mid } else { bot });
                    let col = lane_color(&tk, s.color);
                    if a.x == b.x {
                        p.line_segment([a, b], Stroke::new(1.6, col));
                    } else {
                        let ctrl1 = egui::pos2(a.x, (a.y + b.y) / 2.0);
                        let ctrl2 = egui::pos2(b.x, (a.y + b.y) / 2.0);
                        p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                            [a, ctrl1, ctrl2, b],
                            false,
                            Color32::TRANSPARENT,
                            Stroke::new(1.6, col),
                        ));
                    }
                }
                let center = egui::pos2(x(gr.lane), mid);
                let col = lane_color(&tk, gr.color);
                if c["isMerge"] == true {
                    p.circle(center, 4.0, tk.p.bg, Stroke::new(1.8, col));
                } else {
                    p.circle_filled(center, 4.0, col);
                }
            }
            let mut x = rect.left() + graph_w;
            for (name, kind, is_head) in refs.get(&sha).into_iter().flatten() {
                let (bg, fg) = match kind.as_str() {
                    "Tag" => (tk.p.amber.gamma_multiply(0.2), tk.p.amber),
                    "RemoteBranch" => (tk.p.blue.gamma_multiply(0.15), tk.p.blue),
                    _ => (tk.accent_soft, tk.p.accent),
                };
                let label = if *is_head { format!("{} {name}", icon::CARET_RIGHT) } else { name.clone() };
                let g = p.layout_no_wrap(label, egui::FontId::proportional(tk.text_size - 2.0), fg);
                let rr = Rect::from_min_size(egui::pos2(x, rect.center().y - g.size().y / 2.0 - 2.0), g.size() + Vec2::new(8.0, 4.0));
                p.rect_filled(rr, egui::CornerRadius::same(4), bg);
                p.galley(rr.min + Vec2::new(4.0, 2.0), g, fg);
                x = rr.right() + 4.0;
                if x > rect.right() - 300.0 {
                    break;
                }
            }
            let meta_w = 240.0;
            let mut job = egui::text::LayoutJob::single_section(
                c["summary"].as_str().unwrap_or("").to_string(),
                egui::TextFormat { font_id: egui::FontId::proportional(tk.text_size), color: tk.p.text, ..Default::default() },
            );
            job.wrap = egui::text::TextWrapping::truncate_at_width((rect.right() - meta_w - x).max(30.0));
            let g = p.layout_job(job);
            p.galley(egui::pos2(x, rect.center().y - g.size().y / 2.0), g, tk.p.text);
            let author = c["authorName"].as_str().unwrap_or("");
            let when = widgets::rel_time(c["authorDate"].as_i64().unwrap_or(0) * 1000, now * 1000);
            p.text(
                egui::pos2(rect.right() - 70.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                format!("{author} · {when}"),
                egui::FontId::proportional(tk.text_size - 2.0),
                tk.p.muted,
            );
            p.text(
                egui::pos2(rect.right() - 6.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                c["shortSha"].as_str().unwrap_or(""),
                egui::FontId::monospace(tk.code_size - 2.0),
                tk.p.faint,
            );
            if resp.clicked() {
                click = Some(sha.clone());
            }
            resp.context_menu(|ui| {
                for (id, label) in [
                    ("ui.git.commit.copy-sha", "gitGraph.copySha"),
                    ("ui.git.commit.branch-here", "gitGraph.branchHere"),
                    ("ui.git.commit.tag-here", "gitGraph.tagHere"),
                    ("ui.git.commit.checkout", "gitGraph.checkoutDetached"),
                    ("ui.git.commit.cherry-pick", "gitGraph.cherryPick"),
                    ("ui.git.commit.revert", "gitGraph.revert"),
                    ("ui.git.commit.reset-mixed", "gitGraph.resetMixed"),
                    ("ui.git.commit.reset-hard", "gitGraph.resetHard"),
                    ("ui.git.interactive-rebase", "gitInteractiveRebase.fromHere"),
                ] {
                    if ui.button(t(label)).clicked() {
                        menu_action = Some((id, sha.clone()));
                        ui.close();
                    }
                }
            });
        }
    });
    if let Some(sha) = click {
        if sha == WORKING {
            repo_mut(app, path).selected = Some(WORKING.into());
        } else {
            select_commit(app, path, &sha);
        }
    }
    if let Some((id, sha)) = menu_action {
        let _ = crate::commands::run_ui(app, id, &json!({ "sha": sha }));
    }
}

fn status_letter(s: &Value) -> (&'static str, fn(&crate::theme::Tokens) -> Color32) {
    match s.as_str().unwrap_or("") {
        "Added" | "Untracked" => ("A", |tk| tk.add_fg),
        "Deleted" => ("D", |tk| tk.del_fg),
        "Renamed" => ("R", |tk| tk.p.blue),
        "Conflicted" => ("!", |tk| tk.p.red),
        _ => ("M", |tk| tk.p.amber),
    }
}

fn working(app: &mut LazyApp, ui: &mut egui::Ui, path: &str) {
    let tk = app.tokens;
    let Some(r) = repo(app, path) else { return };
    let staged = r.working["staged"].as_array().cloned().unwrap_or_default();
    let unstaged = r.working["unstaged"].as_array().cloned().unwrap_or_default();
    let wfile = r.wfile.clone();
    let busy = r.op.is_some();
    egui::Panel::left(egui::Id::new(("working-files", path)))
        .default_size(320.0)
        .resizable(true)
        .frame(egui::Frame::new().fill(tk.p.panel).inner_margin(egui::Margin::same(6)))
        .show(ui, |ui| {
            // commit
            let r = repo_mut(app, path);
            ui.add(egui::TextEdit::singleline(&mut r.summary).hint_text(t("gitWorking.summary")).desired_width(f32::INFINITY));
            ui.add(
                egui::TextEdit::multiline(&mut r.body).hint_text(t("gitWorking.description")).desired_rows(2).desired_width(f32::INFINITY),
            );
            let mut amend = r.amend;
            ui.horizontal(|ui| {
                ui.checkbox(&mut amend, t("gitWorking.amend"));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let label = if amend { t("gitWorking.amendCommit") } else { tf("gitWorking.commitN", &[("n", &staged.len())]) };
                    if ui
                        .add_enabled(!busy, egui::Button::new(RichText::new(label).color(tk.on_accent)).fill(tk.p.accent))
                        .on_hover_text("Ctrl+Enter")
                        .clicked()
                    {
                        commit(app, path);
                    }
                    if ui
                        .add_enabled(!busy, egui::Button::new(icon::SPARKLE))
                        .on_hover_text(format!("{} (Ctrl+Shift+M)", t("gitAi.generate")))
                        .clicked()
                    {
                        ai_message(app, path);
                    }
                });
            });
            repo_mut(app, path).amend = amend;
            ui.add_space(4.0);
            widgets::hline(ui, &tk);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for (title, list, is_staged) in [(t("gitWorking.stagedTitle"), &staged, true), (t("gitWorking.unstaged"), &unstaged, false)]
                {
                    ui.horizontal(|ui| {
                        widgets::section(ui, &tk, &format!("{title} ({})", list.len()));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if !list.is_empty() {
                                if is_staged {
                                    if ui.small_button(t("gitWorking.unstageAll")).clicked() {
                                        let _ = crate::commands::run_ui(app, "ui.git.unstage-all", &Value::Null);
                                    }
                                } else {
                                    if ui.small_button(t("gitWorking.stageAll")).clicked() {
                                        let _ = crate::commands::run_ui(app, "ui.git.stage-all", &Value::Null);
                                    }
                                    if ui
                                        .small_button(RichText::new(icon::TRASH).color(tk.p.red))
                                        .on_hover_text(t("gitWorking.discardAll"))
                                        .clicked()
                                    {
                                        let _ = crate::commands::run_ui(app, "ui.git.discard-all", &Value::Null);
                                    }
                                }
                            }
                        });
                    });
                    for f in list {
                        let fpath = f["path"].as_str().unwrap_or("").to_string();
                        let on = wfile.as_ref() == Some(&(is_staged, fpath.clone()));
                        let (letter, color) = status_letter(&f["status"]);
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), tk.row_h - 2.0), Sense::click());
                        if on {
                            ui.painter().rect_filled(rect, egui::CornerRadius::same(tk.radius_sm), tk.p.active);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(rect, egui::CornerRadius::same(tk.radius_sm), tk.p.hover);
                        }
                        let p = ui.painter_at(rect);
                        p.text(
                            rect.left_center() + Vec2::new(8.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            letter,
                            egui::FontId::monospace(tk.code_size),
                            color(&tk),
                        );
                        let mut job = egui::text::LayoutJob::single_section(
                            fpath.clone(),
                            egui::TextFormat {
                                font_id: egui::FontId::proportional(tk.text_size - 1.0),
                                color: tk.p.text,
                                ..Default::default()
                            },
                        );
                        job.wrap = egui::text::TextWrapping::truncate_at_width((rect.width() - 80.0).max(20.0));
                        let g = p.layout_job(job);
                        p.galley(egui::pos2(rect.left() + 24.0, rect.center().y - g.size().y / 2.0), g, tk.p.text);
                        let mut acted = false;
                        if resp.hovered() || on {
                            let mut bx = rect.right() - 12.0;
                            let mut btn = |ic: &str, tip: String, c: Color32| {
                                let br = Rect::from_center_size(egui::pos2(bx, rect.center().y), Vec2::splat(20.0));
                                bx -= 22.0;
                                let r = ui.interact(br, resp.id.with(ic), Sense::click()).on_hover_text(tip);
                                ui.painter().text(
                                    br.center(),
                                    egui::Align2::CENTER_CENTER,
                                    ic,
                                    egui::FontId::proportional(tk.text_size),
                                    if r.hovered() { tk.p.text } else { c },
                                );
                                r.clicked()
                            };
                            if is_staged {
                                if btn(icon::MINUS, t("gitWorking.unstage"), tk.p.muted) {
                                    op(app, path, "unstage", "git.working.unstage-path", json!({ "path": fpath }), None);
                                    acted = true;
                                }
                            } else {
                                if btn(icon::PLUS, t("gitWorking.stage"), tk.p.muted) {
                                    op(app, path, "stage", "git.working.stage-path", json!({ "path": fpath }), None);
                                    acted = true;
                                }
                                if btn(icon::ARROW_COUNTER_CLOCKWISE, t("gitWorking.discard"), tk.p.red) {
                                    let id = rid(app, path).unwrap_or_default();
                                    app.eph.confirm = Some((
                                        tf("gitWorking.confirmDiscard", &[("path", &fpath)]),
                                        "git.working.discard-path".into(),
                                        json!({ "repoId": id, "path": fpath }),
                                    ));
                                    app.state.dialog = Some("confirm".into());
                                    acted = true;
                                }
                            }
                        }
                        if resp.clicked() && !acted {
                            repo_mut(app, path).wfile = Some((is_staged, fpath.clone()));
                        }
                        if resp.double_clicked() {
                            let m = if is_staged { "git.working.unstage-path" } else { "git.working.stage-path" };
                            op(app, path, "stage", m, json!({ "path": fpath }), None);
                        }
                        resp.context_menu(|ui| {
                            if ui.button(format!("{}  {}", icon::USER_LIST, t("gitHistory.blame"))).clicked() {
                                let _ = crate::commands::run_ui(app, "ui.git.blame", &json!({ "path": fpath }));
                                ui.close();
                            }
                            if ui.button(format!("{}  {}", icon::CLOCK_COUNTER_CLOCKWISE, t("gitHistory.fileHistory"))).clicked() {
                                let _ = crate::commands::run_ui(app, "ui.git.file-history", &json!({ "path": fpath }));
                                ui.close();
                            }
                            if ui.button(t("gitWorking.ignore")).clicked() {
                                op(app, path, "ignore", "git.working.ignore", json!({ "pattern": fpath, "local": false }), None);
                                ui.close();
                            }
                            if ui.button(t("sidebar.copyPath")).clicked() {
                                app.eph.copy_request = Some(fpath.clone());
                                ui.close();
                            }
                        });
                    }
                    ui.add_space(6.0);
                }
                if staged.is_empty() && unstaged.is_empty() {
                    ui.label(widgets::faint(&tk, t("gitWorking.clean")));
                }
            });
        });
    egui::CentralPanel::default().frame(egui::Frame::new().fill(tk.p.bg)).show(ui, |ui| {
        let Some((is_staged, fpath)) = wfile else {
            ui.add_space(12.0);
            ui.vertical_centered(|ui| ui.label(widgets::muted(&tk, t("gitWorking.pickFile"))));
            return;
        };
        let diff = repo(app, path).map(|r| r.wdiff[usize::from(is_staged)].clone()).unwrap_or(Value::Null);
        let file = diff["files"].as_array().and_then(|fs| fs.iter().find(|f| f["path"] == fpath.as_str()).cloned());
        match file {
            Some(f) if f["isBinary"] == true => {
                ui.label(widgets::muted(&tk, t("diffFile.binary")));
            }
            Some(f) => {
                let patch = f["patch"].as_str().unwrap_or("").to_string();
                crate::views::git_extra::staging_diff(app, ui, path, &fpath, is_staged, &patch);
            }
            None => {
                ui.label(widgets::faint(&tk, t("gitWorking.noDiff")));
            }
        }
    });
}

fn commit_details(app: &mut LazyApp, ui: &mut egui::Ui, path: &str, sha: &str) {
    let tk = app.tokens;
    let Some(r) = repo(app, path) else { return };
    let c = r.commits.iter().find(|c| c["sha"] == sha).cloned().unwrap_or(Value::Null);
    let diff = r.commit_diff.clone();
    let sel_file = r.diff_file.clone();
    egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(c["summary"].as_str().unwrap_or(sha)).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .small_button(format!("{} {}", icon::COPY, sha.chars().take(10).collect::<String>()))
                    .on_hover_text(t("gitGraph.copySha"))
                    .clicked()
                {
                    app.eph.copy_request = Some(sha.to_string());
                }
            });
        });
        let when = c["authorDate"].as_i64().map(|d| widgets::rel_time(d * 1000, app.host.now_ms())).unwrap_or_default();
        ui.label(widgets::muted(
            &tk,
            format!("{} <{}> · {when}", c["authorName"].as_str().unwrap_or(""), c["authorEmail"].as_str().unwrap_or("")),
        ));
        if let Some(body) = c["body"].as_str().filter(|b| !b.trim().is_empty()) {
            ui.add(egui::Label::new(RichText::new(body.trim()).color(tk.p.text)).wrap().selectable(true));
        }
    });
    widgets::hline(ui, &tk);
    let Some(diff) = diff else {
        ui.spinner();
        return;
    };
    if let Some(e) = diff["error"].as_str() {
        ui.label(RichText::new(e).color(tk.p.red));
        return;
    }
    let files = diff["files"].as_array().cloned().unwrap_or_default();
    egui::Panel::left(egui::Id::new(("commit-files", path)))
        .default_size(260.0)
        .resizable(true)
        .frame(egui::Frame::new().fill(tk.p.panel).inner_margin(egui::Margin::same(4)))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for f in &files {
                    let fp = f["path"].as_str().unwrap_or("").to_string();
                    let (letter, color) = status_letter(&f["status"]);
                    let on = sel_file.as_deref() == Some(fp.as_str());
                    let r =
                        ui.selectable_label(on, RichText::new(format!("{letter}  {fp}")).color(if on { tk.p.text } else { color(&tk) }));
                    r.context_menu(|ui| {
                        if ui.button(t("gitHistory.blame")).clicked() {
                            let _ = crate::commands::run_ui(app, "ui.git.blame", &json!({ "path": fp }));
                            ui.close();
                        }
                        if ui.button(t("gitHistory.fileHistory")).clicked() {
                            let _ = crate::commands::run_ui(app, "ui.git.file-history", &json!({ "path": fp }));
                            ui.close();
                        }
                    });
                    if r.clicked() {
                        repo_mut(app, path).diff_file = Some(fp);
                    }
                }
            });
        });
    egui::CentralPanel::default().frame(egui::Frame::new().fill(tk.p.bg)).show(ui, |ui| {
        if let Some(f) = sel_file.and_then(|p| files.iter().find(|f| f["path"] == p.as_str()).cloned()) {
            if f["isBinary"] == true {
                ui.label(widgets::muted(&tk, t("diffFile.binary")));
            } else {
                crate::views::review::patch_lines(ui, &tk, &parse_patch(f["patch"].as_str().unwrap_or("")), ("cdiff", path, sha));
            }
        }
    });
}

// ---------- Árvore ----------

struct TreeOverview {
    nodes: Vec<(Pos2, usize, String, String)>,
    edges: Vec<(Pos2, Pos2, usize)>,
}

/// Visão geral: só os commits que importam (pontas de ref, merges, pontos
/// de bifurcação, raízes), ligados pelos caminhos de pais entre eles.
fn build_overview(r: &Repo) -> TreeOverview {
    let mut by_sha: HashMap<&str, usize> = HashMap::new();
    for (i, c) in r.commits.iter().enumerate() {
        by_sha.insert(c["sha"].as_str().unwrap_or(""), i);
    }
    let mut children: HashMap<&str, usize> = HashMap::new();
    for c in &r.commits {
        for p in c["parents"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            *children.entry(p).or_default() += 1;
        }
    }
    let ref_names: HashMap<&str, String> = r.refs.iter().fold(HashMap::new(), |mut m, rf| {
        let sha = rf["targetSha"].as_str().unwrap_or("");
        let n = rf["name"].as_str().unwrap_or("").to_string();
        m.entry(sha).and_modify(|e: &mut String| e.push_str(&format!(", {n}"))).or_insert(n);
        m
    });
    let interesting = |c: &Value| -> bool {
        let sha = c["sha"].as_str().unwrap_or("");
        let parents = c["parents"].as_array().map_or(0, Vec::len);
        ref_names.contains_key(sha) || parents != 1 || children.get(sha).copied().unwrap_or(0) != 1
    };
    let keep: Vec<&Value> = r.commits.iter().filter(|c| interesting(c)).collect();
    let keep_set: std::collections::HashSet<&str> = keep.iter().map(|c| c["sha"].as_str().unwrap_or("")).collect();
    // pai "interessante" de cada nó: segue a cadeia de primeiros pais
    let next_kept = |start: &str| -> Option<String> {
        let mut sha = start.to_string();
        for _ in 0..100_000 {
            let i = *by_sha.get(sha.as_str())?;
            let c = r.commits.get(i)?;
            if keep_set.contains(sha.as_str()) {
                return Some(sha);
            }
            sha = c["parents"][0].as_str()?.to_string();
        }
        None
    };
    let pairs: Vec<(String, Vec<String>)> = keep
        .iter()
        .map(|c| {
            let ps =
                c["parents"].as_array().map(|ps| ps.iter().filter_map(Value::as_str).filter_map(next_kept).collect()).unwrap_or_default();
            (c["sha"].as_str().unwrap_or("").to_string(), ps)
        })
        .collect();
    let rows = graph::layout(&pairs);
    let (col_w, row_h) = (190.0, 30.0);
    let mut pos: HashMap<String, Pos2> = HashMap::new();
    let mut nodes = Vec::new();
    for (i, (c, gr)) in keep.iter().zip(&rows).enumerate() {
        let p = Pos2::new(gr.lane as f32 * col_w, i as f32 * row_h);
        let sha = c["sha"].as_str().unwrap_or("").to_string();
        let label = ref_names.get(sha.as_str()).cloned().unwrap_or_else(|| c["shortSha"].as_str().unwrap_or("").to_string());
        pos.insert(sha.clone(), p);
        nodes.push((p, gr.color, label, c["summary"].as_str().unwrap_or("").to_string()));
    }
    let mut edges = Vec::new();
    for ((sha, parents), gr) in pairs.iter().zip(&rows) {
        for p in parents {
            if let (Some(a), Some(b)) = (pos.get(sha), pos.get(p)) {
                edges.push((*a, *b, gr.color));
            }
        }
    }
    TreeOverview { nodes, edges }
}

pub fn tree(app: &mut LazyApp, ui: &mut egui::Ui, path: &str) {
    if !needs_repo(app, ui, path) {
        return;
    }
    let tk = app.tokens;
    if repo(app, path).is_some_and(|r| r.tree_view.is_none() && !r.commits.is_empty()) {
        let ov = repo(app, path).map(build_overview);
        repo_mut(app, path).tree_view = ov;
    }
    let Some(ov) = repo_mut(app, path).tree_view.take() else {
        ui.spinner();
        return;
    };
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(widgets::muted(&tk, tf("gitTree.summary", &[("n", &ov.nodes.len())])));
    });
    let mut rect = egui::Rect::from_min_size(Pos2::new(-40.0, -40.0), Vec2::new(900.0, 700.0));
    let id = egui::Id::new(("tree-scene", path));
    if let Some(r) = ui.data(|d| d.get_temp::<Rect>(id)) {
        rect = r;
    }
    egui::Frame::new().fill(tk.p.bg).show(ui, |ui| {
        egui::Scene::new().zoom_range(0.04..=4.0).show(ui, &mut rect, |ui| {
            let p = ui.painter();
            for (a, b, c) in &ov.edges {
                let col = lane_color(&tk, *c);
                if a.x == b.x {
                    p.line_segment([*a, *b], Stroke::new(2.0, col));
                } else {
                    let mid = (a.y + b.y) / 2.0;
                    p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                        [*a, Pos2::new(a.x, mid), Pos2::new(b.x, mid), *b],
                        false,
                        Color32::TRANSPARENT,
                        Stroke::new(2.0, col),
                    ));
                }
            }
            for (pos, c, label, summary) in &ov.nodes {
                p.circle_filled(*pos, 5.0, lane_color(&tk, *c));
                let r =
                    ui.interact(Rect::from_center_size(*pos, Vec2::splat(12.0)), ui.id().with(label).with(pos.y as i64), Sense::hover());
                r.on_hover_text(summary);
                let mut job = egui::text::LayoutJob::single_section(
                    label.clone(),
                    egui::TextFormat { font_id: egui::FontId::proportional(12.0), color: tk.p.text, ..Default::default() },
                );
                job.wrap = egui::text::TextWrapping::truncate_at_width(160.0);
                let g = p.layout_job(job);
                p.galley(*pos + Vec2::new(10.0, -7.0), g, tk.p.text);
            }
            // o tamanho do que foi desenhado
            let max_y = ov.nodes.iter().map(|n| n.0.y).fold(0.0f32, f32::max);
            ui.allocate_rect(Rect::from_min_max(Pos2::new(-20.0, -20.0), Pos2::new(1200.0, max_y + 40.0)), Sense::hover());
        });
    });
    ui.data_mut(|d| d.insert_temp(id, rect));
    repo_mut(app, path).tree_view = Some(ov);
}

// ---------- Reflog ----------

pub fn reflog(app: &mut LazyApp, ui: &mut egui::Ui, path: &str) {
    if !needs_repo(app, ui, path) {
        return;
    }
    let tk = app.tokens;
    let Some(id) = rid(app, path) else { return };
    let want = repo(app, path)
        .map(|r| if r.reflog_ref.is_empty() { "HEAD".to_string() } else { r.reflog_ref.clone() })
        .unwrap_or_else(|| "HEAD".into());
    if repo(app, path).is_some_and(|r| r.reflog.is_empty() && r.op.as_deref() != Some("reflog")) {
        repo_mut(app, path).op = Some("reflog".into());
        let p = path.to_string();
        app.call("git.reflog.list", json!({ "repoId": id, "refName": want }), move |app, res| {
            let r = repo_mut(app, &p);
            r.op = None;
            match res {
                Ok(v) => r.reflog = v.as_array().cloned().unwrap_or_default(),
                Err(e) => app.toast_err(&e.message),
            }
        });
    }
    let entries = repo(app, path).map(|r| r.reflog.clone()).unwrap_or_default();
    egui::Frame::new().inner_margin(egui::Margin::same(10)).show(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::section(ui, &tk, &format!("{} · {want}", t("mainTabs.reflog")));
            if ui.small_button(icon::ARROWS_CLOCKWISE).clicked() {
                repo_mut(app, path).reflog.clear();
            }
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new(("reflog", path)).striped(true).num_columns(4).show(ui, |ui| {
                for e in &entries {
                    let sha = e["newSha"].as_str().or_else(|| e["sha"].as_str()).unwrap_or("").to_string();
                    ui.label(RichText::new(sha.chars().take(8).collect::<String>()).monospace().color(tk.p.faint));
                    ui.label(RichText::new(e["action"].as_str().or_else(|| e["kind"].as_str()).unwrap_or("")).color(tk.p.blue));
                    ui.label(e["message"].as_str().unwrap_or(""));
                    ui.horizontal(|ui| {
                        if ui.small_button(t("gitReflog.checkout")).clicked() {
                            let _ = crate::commands::run_ui(app, "ui.git.commit.checkout", &json!({ "sha": sha }));
                        }
                        if ui.small_button(t("gitGraph.branchHere")).clicked() {
                            if let Some(s) = sha.get(..) {
                                select_commit(app, path, s);
                            }
                            repo_mut(app, path).new_branch = Some(String::new());
                        }
                    });
                    ui.end_row();
                }
            });
        });
    });
}

// ---------- Estatísticas ----------

pub fn stats(app: &mut LazyApp, ui: &mut egui::Ui, path: &str) {
    if !needs_repo(app, ui, path) {
        return;
    }
    let tk = app.tokens;
    let Some(id) = rid(app, path) else { return };
    if repo(app, path).is_some_and(|r| r.stats.is_null() && r.op.as_deref() != Some("stats")) {
        repo_mut(app, path).op = Some("stats".into());
        let p = path.to_string();
        app.call("git.log.stats", json!({ "repoId": id }), move |app, res| {
            let r = repo_mut(app, &p);
            r.op = None;
            r.stats = res.unwrap_or_else(|e| json!({ "error": e.message }));
        });
    }
    let s = repo(app, path).map(|r| r.stats.clone()).unwrap_or(Value::Null);
    if s.is_null() {
        ui.spinner();
        return;
    }
    if let Some(e) = s["error"].as_str() {
        ui.label(RichText::new(e).color(tk.p.red));
        return;
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::same(14)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (k, label) in [
                    ("totalCommits", "gitStats.commits"),
                    ("contributors", "gitStats.contributors"),
                    ("branches", "gitStats.branches"),
                    ("tags", "gitStats.tags"),
                ] {
                    let v = match &s[k] {
                        Value::Array(a) => a.len().to_string(),
                        Value::Number(n) => n.to_string(),
                        _ => continue,
                    };
                    egui::Frame::new()
                        .fill(tk.p.panel)
                        .stroke(Stroke::new(1.0, tk.p.border))
                        .corner_radius(egui::CornerRadius::same(tk.radius))
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_min_width(140.0);
                            ui.label(RichText::new(v).size(22.0).strong());
                            ui.label(widgets::muted(&tk, t(label)));
                        });
                }
            });
            ui.add_space(12.0);
            // gráfico de atividade (commits por semana/mês, se vier)
            if let Some(series) = s["commitsByMonth"].as_array().or_else(|| s["activity"].as_array()) {
                widgets::section(ui, &tk, &t("gitStats.activity"));
                let max = series.iter().filter_map(|x| x["count"].as_u64().or_else(|| x.as_u64())).max().unwrap_or(1).max(1);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(900.0), 140.0), Sense::hover());
                let n = series.len().max(1) as f32;
                let bw = rect.width() / n;
                for (i, x) in series.iter().enumerate() {
                    let c = x["count"].as_u64().or_else(|| x.as_u64()).unwrap_or(0);
                    let h = rect.height() * c as f32 / max as f32;
                    let b = Rect::from_min_max(
                        Pos2::new(rect.left() + i as f32 * bw + 1.0, rect.bottom() - h),
                        Pos2::new(rect.left() + (i + 1) as f32 * bw - 1.0, rect.bottom()),
                    );
                    ui.painter().rect_filled(b, 2.0, tk.p.accent.gamma_multiply(0.8));
                }
                ui.add_space(12.0);
            }
            if let Some(people) = s["contributors"].as_array().or_else(|| s["authors"].as_array()) {
                widgets::section(ui, &tk, &t("gitStats.contributors"));
                let max = people.iter().filter_map(|p| p["commits"].as_u64()).max().unwrap_or(1).max(1);
                for p in people.iter().take(30) {
                    let n = p["commits"].as_u64().unwrap_or(0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(p["name"].as_str().unwrap_or("?")).color(tk.p.text));
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(300.0, 10.0), Sense::hover());
                        ui.painter().rect_filled(rect, 3.0, tk.p.panel);
                        let w = rect.width() * n as f32 / max as f32;
                        ui.painter().rect_filled(Rect::from_min_size(rect.min, Vec2::new(w, rect.height())), 3.0, tk.p.blue);
                        ui.label(widgets::muted(&tk, n.to_string()));
                    });
                }
            }
        });
    });
}

/// Pedidos de senha do ssh e credenciais https (o git espera a resposta).
pub fn prompts(app: &mut LazyApp, ctx: &egui::Context) {
    let tk = app.tokens;
    if let Some(key) = app.eph.git.prompts.ssh_key.clone() {
        let mut done: Option<Option<String>> = None;
        egui::Modal::new(egui::Id::new("ssh-pass")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading(t("gitSsh.passphraseTitle"));
            ui.label(widgets::muted(&tk, key.clone()));
            let r = ui.add(egui::TextEdit::singleline(&mut app.eph.git.prompts.secret).password(true).desired_width(f32::INFINITY));
            r.request_focus();
            ui.horizontal(|ui| {
                if widgets::primary(ui, &tk, &t("app.ok")).clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    done = Some(Some(app.eph.git.prompts.secret.clone()));
                }
                if ui.button(t("app.cancel")).clicked() {
                    done = Some(None);
                }
            });
        });
        if let Some(pass) = done {
            app.eph.git.prompts.ssh_key = None;
            app.eph.git.prompts.secret.clear();
            app.fire("git.ssh.submit-passphrase", json!({ "passphrase": pass }));
        }
    }
    if let Some(req) = app.eph.git.prompts.https.clone() {
        let mut done: Option<bool> = None;
        egui::Modal::new(egui::Id::new("https-creds")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading(t("gitRemote.credentialsTitle"));
            ui.label(widgets::muted(&tk, req["url"].as_str().unwrap_or("")));
            ui.add(
                egui::TextEdit::singleline(&mut app.eph.git.prompts.user).hint_text(t("gitRemote.username")).desired_width(f32::INFINITY),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.eph.git.prompts.secret)
                    .password(true)
                    .hint_text(t("gitRemote.passwordOrToken"))
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal(|ui| {
                if widgets::primary(ui, &tk, &t("app.ok")).clicked() {
                    done = Some(true);
                }
                if ui.button(t("app.cancel")).clicked() {
                    done = Some(false);
                }
            });
        });
        if let Some(ok) = done {
            let p = &mut app.eph.git.prompts;
            let (u, s) = (std::mem::take(&mut p.user), std::mem::take(&mut p.secret));
            p.https = None;
            if ok {
                app.fire("git.https.submit-credentials", json!({ "username": u, "password": s }));
            } else {
                app.fire("git.https.submit-credentials", json!({}));
            }
        }
    }
}

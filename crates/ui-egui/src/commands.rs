//! Os comandos de tela (`ui.*`): trocar de aba, abrir a paleta, tema,
//! zoom, navegar entre threads… São o lado da casca do "tudo é um comando":
//! atalhos, menus, a paleta e o canal de controle (`ui.run`) chamam todos
//! por aqui. Os comandos do motor que pedem o contexto da tela (a thread
//! aberta, o workspace ativo) recebem esses parâmetros de [`fill_context`].

use std::sync::{Arc, OnceLock};

use lazyaicode_doc::transcript::Decision;
use lazyaicode_doc::Status;
use lazyaicode_i18n::t;
use lazyaicode_protocol::{CommandInfo, ParamInfo, ParamType};
use serde_json::{json, Map, Value};

use crate::app::{LazyApp, ToastKind};
use crate::state::{HistoryView, MainTab, ReviewBase, SettingsTab, TreeMode, MAIN_TABS, SETTINGS_TABS};
use crate::theme::{ContentWidth, Density, THEMES};

/// Resultado de um comando que só responde depois do próximo quadro.
pub const DEFERRED: &str = "\u{0}deferred";

type Run = Arc<dyn Fn(&mut LazyApp, &Value) -> Result<Value, String> + Send + Sync>;
/// Implementação de um comando de tela registrado por outro módulo.
pub type RunBox = Box<dyn Fn(&mut LazyApp, &Value) -> Result<Value, String> + Send + Sync>;

pub struct UiCmd {
    pub id: String,
    pub label: String,
    pub group: &'static str,
    pub shortcut: Option<String>,
    pub params: Vec<ParamInfo>,
    pub doc: &'static str,
    pub run: Run,
}

struct Reg(Vec<UiCmd>);

impl Reg {
    fn add(
        &mut self,
        id: &str,
        label: &str,
        group: &'static str,
        shortcut: Option<&str>,
        run: impl Fn(&mut LazyApp, &Value) -> Result<Value, String> + Send + Sync + 'static,
    ) -> &mut UiCmd {
        self.0.push(UiCmd {
            id: id.into(),
            label: label.into(),
            group,
            shortcut: shortcut.map(str::to_string),
            params: vec![],
            doc: "",
            run: Arc::new(run),
        });
        let n = self.0.len() - 1;
        &mut self.0[n]
    }
}

impl UiCmd {
    fn params(&mut self, p: Vec<ParamInfo>) -> &mut Self {
        self.params = p;
        self
    }

    fn doc(&mut self, d: &'static str) -> &mut Self {
        self.doc = d;
        self
    }
}

fn ok() -> Result<Value, String> {
    Ok(Value::Null)
}

fn sel(app: &LazyApp) -> Result<String, String> {
    app.state.selected.clone().ok_or_else(|| t("palette.needsThread"))
}

fn str_param(p: &Value, k: &str) -> Result<String, String> {
    p.get(k).and_then(Value::as_str).map(str::to_string).ok_or_else(|| format!("falta `{k}`"))
}

/// Comandos do motor que a tela roda com o contexto atual.
fn engine(app: &mut LazyApp, id: &str, extra: Value) -> Result<Value, String> {
    let mut m = match extra {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    let info = app.data.command(id).cloned();
    if let Some(info) = &info {
        fill_context(app, info, &mut m);
    }
    app.fire(id, Value::Object(m));
    ok()
}

fn build() -> Vec<UiCmd> {
    let mut r = Reg(Vec::new());

    // ---------- automação ----------
    r.add("ui.inspect", "Inspect the screen", "ui", None, |app, _| Ok(crate::control::inspect(app)))
        .doc("Estado da tela: aba, thread aberta, painéis e diálogos, workspace ativo, contagens.");
    r.add("ui.screenshot", "Screenshot", "ui", None, crate::control::screenshot)
        .params(vec![ParamInfo::opt("cpu", ParamType::Bool).doc("renderizar com o compositor de referência em CPU")])
        .doc("PNG da janela em base64.");
    r.add("ui.run", "Run a screen command", "ui", None, |app, p| {
        let cmd = str_param(p, "command")?;
        if cmd == "ui.run" {
            return Err("ui.run não chama ui.run".into());
        }
        run_ui(app, &cmd, p.get("params").unwrap_or(&Value::Null))
    })
    .params(vec![ParamInfo::req("command", ParamType::String), ParamInfo::opt("params", ParamType::Json)]);
    r.add("ui.state.set", "Change screen state", "ui", None, |app, p| {
        let mut cur = serde_json::to_value(&app.state).map_err(|e| e.to_string())?;
        let patch = p.get("patch").and_then(Value::as_object).ok_or("`patch` precisa ser um objeto")?;
        if let Some(obj) = cur.as_object_mut() {
            // um campo com nome errado não pode passar calado
            if let Some(k) = patch.keys().find(|k| !obj.contains_key(*k)) {
                return Err(format!("campo desconhecido no estado da tela: {k}"));
            }
            for (k, v) in patch {
                obj.insert(k.clone(), v.clone());
            }
        }
        app.state = serde_json::from_value(cur).map_err(|e| e.to_string())?;
        // o diálogo de confirmação vive fora do estado serializável
        if app.state.dialog.is_none() {
            app.eph.confirm = None;
        }
        ok()
    })
    .params(vec![ParamInfo::req("patch", ParamType::Json)])
    .doc("Mescla campos no estado da tela (os mesmos do ui.inspect).");

    // ---------- janelas e painéis ----------
    r.add("ui.palette.toggle", "palette.title", "view", Some("Ctrl+K"), |app, _| {
        app.state.palette_open = !app.state.palette_open;
        app.state.palette_query.clear();
        app.eph.palette_form = None;
        app.eph.palette_index = 0;
        ok()
    });
    r.add("ui.palette.commands", "palette.allCommands", "view", Some("Ctrl+Shift+P"), |app, _| {
        app.state.palette_open = true;
        app.state.palette_query = ">".into();
        app.eph.palette_form = None;
        ok()
    });
    r.add("ui.settings.open", "settings.title", "view", Some("Ctrl+Comma"), |app, _| {
        app.state.settings_open = true;
        ok()
    });
    r.add("ui.settings.close", "Close settings", "view", None, |app, _| {
        app.state.settings_open = false;
        ok()
    });
    for tab in SETTINGS_TABS {
        r.add(&format!("ui.settings.{}", tab.id()), &format!("settings.tab.{}", tab.id()), "settings", None, move |app, _| {
            app.state.settings_open = true;
            app.state.settings_tab = tab;
            ok()
        });
    }
    r.add("ui.sidebar.toggle", "shortcuts.toggleSidebar", "view", Some("Ctrl+B"), |app, _| {
        app.state.sidebar_open = !app.state.sidebar_open;
        ok()
    });
    r.add("ui.sidebar.mode.project", "sidebar.groupByProject", "view", None, |app, _| {
        app.state.tree_mode = TreeMode::Project;
        ok()
    });
    r.add("ui.sidebar.mode.recent", "sidebar.groupByDate", "view", None, |app, _| {
        app.state.tree_mode = TreeMode::Recent;
        ok()
    });
    r.add("ui.sidebar.mode.toggle", "Toggle grouping", "view", Some("Ctrl+Shift+G"), |app, _| {
        app.state.tree_mode = if app.state.tree_mode == TreeMode::Project { TreeMode::Recent } else { TreeMode::Project };
        ok()
    });
    r.add("ui.sidebar.archived.toggle", "sidebar.showArchived", "view", Some("Ctrl+Shift+A"), |app, _| {
        app.state.show_archived = !app.state.show_archived;
        ok()
    });
    r.add("ui.sidebar.filter", "Filter threads", "view", None, |app, p| {
        app.state.filter = p.get("text").and_then(Value::as_str).unwrap_or("").to_string();
        app.state.sidebar_open = true;
        ok()
    })
    .params(vec![ParamInfo::opt("text", ParamType::String)]);
    r.add("ui.sidebar.collapse-all", "Collapse all groups", "view", None, |app, _| {
        for g in crate::views::sidebar::group_keys(app) {
            app.state.collapsed.insert(g, true);
        }
        ok()
    });
    r.add("ui.sidebar.expand-all", "Expand all groups", "view", None, |app, _| {
        app.state.collapsed.clear();
        for g in crate::views::sidebar::group_keys(app) {
            app.state.collapsed.insert(g, false);
        }
        ok()
    });
    r.add("ui.help.shortcuts", "shortcuts.title", "help", Some("F1"), |app, _| {
        app.state.settings_open = true;
        app.state.settings_tab = SettingsTab::Shortcuts;
        ok()
    });
    r.add("ui.help.about", "About LazyAICode", "help", None, |app, _| {
        app.state.settings_open = true;
        app.state.settings_tab = SettingsTab::About;
        ok()
    });
    r.add("ui.dialog.confirm", "dialogs.confirm", "view", None, |app, _| {
        if crate::views::dialogs::resolve(app, true) {
            ok()
        } else {
            Err(t("dialogs.none"))
        }
    })
    .doc("Confirma o diálogo aberto (apagar thread, descartar mudanças…), como o botão Confirmar.");
    r.add("ui.dialog.cancel", "app.cancel", "view", None, |app, _| {
        let had_result = app.eph.palette_result.take().is_some();
        if crate::views::dialogs::resolve(app, false) || had_result {
            ok()
        } else {
            Err(t("dialogs.none"))
        }
    })
    .doc("Fecha o diálogo aberto sem fazer nada, ou a janela de resultado da paleta.");
    r.add("ui.toasts.clear", "Dismiss notifications", "view", None, |app, _| {
        app.toasts.clear();
        ok()
    });

    // ---------- abas ----------
    for (i, tab) in MAIN_TABS.into_iter().enumerate() {
        let sc = format!("Ctrl+{}", i + 1);
        r.add(&format!("ui.tab.{}", tab.id()), tab.label_key(), "tabs", Some(&sc), move |app, _| set_tab(app, tab));
    }
    r.add("ui.tab.next", "Next tab", "tabs", Some("Ctrl+Tab"), |app, _| cycle_tab(app, 1));
    r.add("ui.tab.prev", "Previous tab", "tabs", Some("Ctrl+Shift+Tab"), |app, _| cycle_tab(app, -1));
    r.add("ui.history.graph", "mainTabs.history", "tabs", None, |app, _| {
        app.state.tab = MainTab::History;
        app.state.history_view = HistoryView::Graph;
        ok()
    });
    r.add("ui.history.review", "thread.agentReview", "tabs", Some("Ctrl+Shift+R"), |app, _| {
        app.state.tab = MainTab::History;
        app.state.history_view = HistoryView::Review;
        if let Some(id) = app.state.selected.clone() {
            let base = app.state.review_base.id();
            crate::views::review::load_changes(app, &id, base);
        }
        ok()
    });

    // ---------- threads ----------
    r.add("ui.thread.new", "sidebar.newThreadShort", "thread", Some("Ctrl+N"), |app, p| new_thread(app, p, false))
        .params(vec![ParamInfo::opt("projectId", ParamType::String)]);
    r.add("ui.thread.new-isolated", "sidebar.newIsolated", "thread", Some("Ctrl+Shift+N"), |app, p| new_thread(app, p, true))
        .params(vec![ParamInfo::opt("projectId", ParamType::String)]);
    r.add("ui.thread.select", "Open thread", "thread", None, |app, p| {
        let id = str_param(p, "id")?;
        if app.data.thread(&id).is_none() {
            return Err(t("palette.threadNotFound"));
        }
        app.select_thread(&id);
        ok()
    })
    .params(vec![ParamInfo::req("id", ParamType::String)]);
    r.add("ui.thread.next", "Next thread", "thread", Some("Alt+ArrowDown"), |app, _| step_thread(app, 1));
    r.add("ui.thread.prev", "Previous thread", "thread", Some("Alt+ArrowUp"), |app, _| step_thread(app, -1));
    for n in 1..=9usize {
        let sc = format!("Alt+{n}");
        r.add(&format!("ui.thread.open-{n}"), &format!("Open thread {n}"), "thread", Some(&sc), move |app, _| {
            let ids = crate::views::sidebar::visible_thread_ids(app);
            let id = ids.get(n - 1).cloned().ok_or_else(|| t("palette.threadNotFound"))?;
            app.select_thread(&id);
            ok()
        });
    }
    r.add("ui.thread.close", "Close thread", "thread", None, |app, _| {
        app.state.selected = None;
        ok()
    });
    r.add("ui.thread.rename", "thread.renameHint", "thread", Some("F2"), |app, p| {
        let id = sel(app)?;
        match p.get("title").and_then(Value::as_str) {
            Some(title) => engine(app, "thread.update", json!({ "id": id, "title": title })),
            None => {
                app.eph.editing_title = app.selected().map(|t| t.thread.title.clone());
                ok()
            }
        }
    })
    .params(vec![ParamInfo::opt("title", ParamType::String)]);
    r.add("ui.thread.toggle-bookmark", "shortcuts.toggleBookmark", "thread", Some("Ctrl+D"), |app, _| {
        let id = sel(app)?;
        let on = app.selected().is_some_and(|t| t.thread.bookmarked);
        let v = !on;
        app.call_ok("thread.update", json!({ "id": id, "bookmarked": v }), |app, t| {
            if let Ok(t) = serde_json::from_value(t) {
                app.data.upsert(t);
            }
        });
        ok()
    });
    r.add("ui.thread.mark-unread", "Mark as unread", "thread", None, |app, _| {
        let id = sel(app)?;
        engine(app, "thread.update", json!({ "id": id, "unread": true }))
    });
    r.add("ui.thread.archive", "thread.archiveThread", "thread", Some("Ctrl+Shift+E"), |app, _| {
        let id = sel(app)?;
        let archived = !app.selected().is_some_and(|t| t.thread.archived);
        app.call_ok("thread.archive", json!({ "id": id, "archived": archived }), move |app, v| {
            if let Ok(t) = serde_json::from_value(v) {
                app.data.upsert(t);
            }
            app.toast(t(if archived { "thread.threadArchived" } else { "thread.threadRestored" }), ToastKind::Ok);
        });
        ok()
    });
    r.add("ui.thread.delete", "Delete thread", "thread", Some("Ctrl+Shift+Delete"), |app, _| {
        let id = sel(app)?;
        let title = app.selected().map(|t| t.thread.title.clone()).unwrap_or_default();
        app.eph.confirm =
            Some((lazyaicode_i18n::tf("thread.confirmDelete", &[("title", &title)]), "thread.delete".into(), json!({ "id": id })));
        app.state.dialog = Some("confirm".into());
        ok()
    });
    r.add("ui.thread.interrupt", "Stop the agent", "thread", Some("Escape"), |app, _| {
        let id = sel(app)?;
        if app.selected().is_some_and(|t| matches!(t.status, Status::Running | Status::Awaiting)) {
            app.fire("thread.interrupt", json!({ "id": id }));
        }
        ok()
    });
    r.add("ui.thread.send", "Send message", "thread", None, |app, p| {
        let id = sel(app)?;
        let text = match p.get("text").and_then(Value::as_str) {
            Some(t) => t.to_string(),
            None => app.data.drafts.remove(&id).unwrap_or_default(),
        };
        if text.trim().is_empty() && app.data.attachments.get(&id).is_none_or(Vec::is_empty) {
            return Err(t("uiErrors.emptyMessage"));
        }
        app.send_message(&id, text.trim().to_string());
        ok()
    })
    .params(vec![ParamInfo::opt("text", ParamType::String)]);
    r.add("ui.composer.focus", "Focus the message box", "thread", Some("Ctrl+L"), |app, _| {
        app.state.tab = MainTab::Chat;
        app.eph.focus_composer = true;
        ok()
    });
    r.add("ui.composer.clear", "Clear the message box", "thread", None, |app, _| {
        let id = sel(app)?;
        app.data.drafts.remove(&id);
        app.data.attachments.remove(&id);
        ok()
    });
    r.add("ui.composer.attach", "Attach image", "thread", Some("Ctrl+Shift+I"), |app, _| {
        let id = sel(app)?;
        app.host.pick_images("ui.composer.attach-paths", json!({ "id": id }));
        ok()
    });
    r.add("ui.composer.attach-paths", "Attach image files", "thread", None, |app, p| {
        let id = p.get("id").and_then(Value::as_str).map(str::to_string).map_or_else(|| sel(app), Ok)?;
        for path in p.get("paths").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
            crate::views::composer::attach_path(app, &id, path);
        }
        ok()
    })
    .params(vec![ParamInfo::opt("id", ParamType::String), ParamInfo::req("paths", ParamType::Strings)]);
    r.add("ui.composer.attach-data", "Attach image data", "thread", None, |app, p| {
        let id = p.get("id").and_then(Value::as_str).map(str::to_string).map_or_else(|| sel(app), Ok)?;
        let mime = str_param(p, "mime")?;
        if !["image/png", "image/jpeg", "image/gif", "image/webp"].contains(&mime.as_str()) {
            return Err(t("thread.onlyImages"));
        }
        let data = str_param(p, "data")?;
        if data.len() > 7 * 1024 * 1024 {
            return Err(lazyaicode_i18n::tf("images.tooLarge", &[("name", &p.get("name").and_then(Value::as_str).unwrap_or("?"))]));
        }
        let list = app.data.attachments.entry(id).or_default();
        if list.len() < crate::views::composer::MAX_IMAGES {
            list.push(crate::data::Attachment { mime, data, name: p.get("name").and_then(Value::as_str).unwrap_or("imagem").into() });
        }
        ok()
    })
    .params(vec![
        ParamInfo::opt("id", ParamType::String),
        ParamInfo::req("mime", ParamType::String),
        ParamInfo::req("data", ParamType::String).doc("base64"),
        ParamInfo::opt("name", ParamType::String),
    ]);
    r.add("ui.composer.paste-image", "Paste image", "thread", Some("Ctrl+Shift+V"), |app, _| {
        let id = sel(app)?;
        crate::views::composer::paste_image(app, &id);
        ok()
    });
    r.add("ui.thread.scroll-bottom", "Scroll to the end", "thread", Some("Ctrl+End"), |app, _| {
        app.eph.scroll_to_bottom = true;
        ok()
    });
    r.add("ui.thread.expand-tools", "Expand tool calls", "thread", None, |app, _| set_tools_open(app, true));
    r.add("ui.thread.collapse-tools", "Collapse tool calls", "thread", None, |app, _| set_tools_open(app, false));
    r.add("ui.thread.expand-thinking", "Show reasoning", "thread", None, |app, _| {
        let keys: Vec<String> = current_items(app)
            .filter(|i| matches!(i, lazyaicode_doc::transcript::Item::Thinking { .. }))
            .map(|i| i.key().to_string())
            .collect();
        for k in keys {
            app.eph.thinking_open.insert(k, true);
        }
        ok()
    });
    for (id, decision, sc) in
        [("allow", Decision::Allow, "Ctrl+Y"), ("always", Decision::Always, "Ctrl+Shift+Y"), ("deny", Decision::Deny, "Ctrl+Shift+X")]
    {
        r.add(&format!("ui.permission.{id}"), &format!("permission.{id}"), "thread", Some(sc), move |app, _| {
            let tid = sel(app)?;
            let rid = app
                .data
                .transcripts
                .get(&tid)
                .and_then(|tr| tr.pending_permission())
                .and_then(|i| match i {
                    lazyaicode_doc::transcript::Item::Permission { request_id, .. } => Some(request_id.clone()),
                    _ => None,
                })
                .ok_or_else(|| t("palette.noPermission"))?;
            app.respond_permission(&tid, &rid, decision);
            ok()
        });
    }
    for (mode, label) in [("apply", "thread.applyTitle"), ("merge", "thread.mergeTitle")] {
        r.add(&format!("ui.thread.land-{mode}"), label, "review", None, move |app, _| {
            let id = sel(app)?;
            app.call_ok("thread.land", json!({ "id": id, "mode": mode }), |app, v| {
                app.toast(v.as_str().unwrap_or("ok").to_string(), ToastKind::Ok);
            });
            ok()
        });
    }
    r.add("ui.thread.land-pr", "thread.prTitle", "review", None, |app, _| {
        let id = sel(app)?;
        app.send_message(&id, t("thread.prPrompt"));
        ok()
    });
    r.add("ui.thread.copy-path", "Copy checkout path", "thread", None, |app, _| {
        let p = app.selected().map(|t| t.thread.cwd.clone()).ok_or_else(|| t("palette.needsThread"))?;
        copy(app, p)
    });
    r.add("ui.thread.copy-branch", "Copy branch name", "thread", None, |app, _| {
        let id = sel(app)?;
        let b = app
            .data
            .branch
            .get(&id)
            .and_then(|b| b["branch"].as_str().map(str::to_string))
            .or_else(|| app.selected().and_then(|t| t.thread.branch.clone()));
        copy(app, b.ok_or_else(|| t("uiErrors.noBranch"))?)
    });
    r.add("ui.thread.copy-session", "Copy session id", "thread", None, |app, _| {
        let s = app.selected().and_then(|t| t.thread.session_id.clone()).ok_or_else(|| t("uiErrors.noSession"))?;
        copy(app, s)
    });
    r.add("ui.thread.copy-transcript", "Copy conversation as Markdown", "thread", None, |app, _| {
        let id = sel(app)?;
        let md = crate::views::transcript::to_markdown(app.data.transcripts.get(&id).map(|t| t.items.as_slice()).unwrap_or(&[]));
        copy(app, md)
    });
    r.add("ui.thread.open-folder", "Open checkout folder", "thread", None, |app, _| {
        let p = app.selected().map(|t| t.thread.cwd.clone()).ok_or_else(|| t("palette.needsThread"))?;
        engine(app, "system.open-path", json!({ "path": p }))
    });
    r.add("ui.thread.reload", "Reload conversation", "thread", None, |app, _| {
        let id = sel(app)?;
        app.data.transcripts.remove(&id);
        app.load_transcript(&id);
        app.refresh_thread_meta(&id);
        ok()
    });
    for (m, label) in lazyaicode_doc::MODELS {
        let mm = m.to_string();
        r.add(&format!("ui.thread.model.{}", m.replace(['[', ']'], "-").trim_end_matches('-')), label, "model", None, move |app, _| {
            let id = sel(app)?;
            set_thread(app, "thread.set-model", json!({ "id": id, "model": mm }))
        });
    }
    for (m, label) in lazyaicode_doc::PERMISSION_MODES {
        let mm = m.to_string();
        r.add(&format!("ui.thread.mode.{}", m.to_lowercase()), label, "model", None, move |app, _| {
            let id = sel(app)?;
            set_thread(app, "thread.set-permission-mode", json!({ "id": id, "mode": mm }))
        });
    }
    r.add("ui.thread.mode.cycle", "Cycle permission mode", "model", Some("Shift+Tab"), |app, _| {
        let id = sel(app)?;
        let cur = app.selected().map(|t| t.thread.permission_mode.clone()).unwrap_or_default();
        let modes: Vec<&str> = lazyaicode_doc::PERMISSION_MODES.iter().map(|m| m.0).collect();
        let i = modes.iter().position(|m| *m == cur).map_or(0, |i| (i + 1) % modes.len());
        let next = modes.get(i).copied().unwrap_or("default");
        set_thread(app, "thread.set-permission-mode", json!({ "id": id, "mode": next }))
    });
    for (e, label) in lazyaicode_doc::EFFORTS {
        let ee = e.to_string();
        let id = if e.is_empty() { "default" } else { e };
        r.add(&format!("ui.thread.effort.{id}"), label, "model", None, move |app, _| {
            let id = sel(app)?;
            set_thread(app, "thread.set-options", json!({ "id": id, "effort": ee }))
        });
    }
    for (p, label, _) in lazyaicode_doc::PROVIDERS {
        r.add(&format!("ui.thread.provider.{p}"), label, "model", None, move |app, _| {
            let id = sel(app)?;
            set_thread(app, "thread.set-provider", json!({ "id": id, "provider": p }))
        });
    }
    r.add("ui.thread.skip-permissions.toggle", "Toggle skip permissions", "model", None, |app, _| {
        let id = sel(app)?;
        let v = !app.selected().is_some_and(|t| t.thread.skip_permissions);
        set_thread(app, "thread.set-options", json!({ "id": id, "skipPermissions": v }))
    });

    // ---------- revisão ----------
    for base in [ReviewBase::Upstream, ReviewBase::Turn, ReviewBase::Head] {
        r.add(&format!("ui.review.base.{}", base.id()), &format!("agentReview.base.{}", base.id()), "review", None, move |app, _| {
            app.state.review_base = base;
            if let Some(id) = app.state.selected.clone() {
                crate::views::review::load_changes(app, &id, base.id());
            }
            ok()
        });
    }
    r.add("ui.review.next-file", "Next file", "review", Some("Alt+ArrowRight"), |app, _| crate::views::review::step_file(app, 1));
    r.add("ui.review.prev-file", "Previous file", "review", Some("Alt+ArrowLeft"), |app, _| crate::views::review::step_file(app, -1));
    r.add("ui.review.toggle-seen", "Mark file as reviewed", "review", Some("Ctrl+Shift+S"), |app, _| {
        crate::views::review::toggle_seen(app, None)
    });
    r.add("ui.review.mark-all-seen", "Mark all as reviewed", "review", None, |app, _| crate::views::review::mark_all(app, true));
    r.add("ui.review.mark-none-seen", "Mark none as reviewed", "review", None, |app, _| crate::views::review::mark_all(app, false));
    r.add("ui.review.refresh", "Refresh changes", "review", None, |app, _| {
        let id = sel(app)?;
        let base = app.state.review_base.id();
        crate::views::review::load_changes(app, &id, base);
        ok()
    });

    // ---------- workspaces ----------
    r.add("ui.workspace.new", "newWorkspace.title", "workspace", Some("Ctrl+Shift+O"), |app, _| {
        app.state.new_workspace_open = true;
        ok()
    });
    r.add("ui.workspace.pick", "workspacePicker.title", "workspace", None, |app, _| {
        app.eph.picker_checked = app.state.open_workspaces.clone();
        app.state.picking_workspaces = true;
        ok()
    });
    r.add("ui.workspace.open", "Open workspace", "workspace", None, |app, p| {
        let id = str_param(p, "id")?;
        if app.data.workspace(&id).is_none() {
            return Err(t("uiErrors.workspaceNotFound"));
        }
        app.open_workspace(&id, None);
        ok()
    })
    .params(vec![ParamInfo::req("id", ParamType::String)])
    .doc("Abre o workspace: na janela onde ele já está, ou numa janela nova (no desktop); na web, numa aba.");
    r.add("ui.activity.toggle", "activity.title", "workspace", Some("Ctrl+Shift+D"), |app, _| {
        app.state.activity_open = !app.state.activity_open;
        if app.state.activity_open {
            app.refresh_activity();
        }
        ok()
    })
    .doc("Mostra / esconde as sessões de todos os workspaces que rodam, esperam você ou terminaram sem ser lidas.");
    r.add("ui.activity.next", "activity.next", "workspace", Some("Ctrl+Shift+T"), |app, _| crate::views::activity::open_next(app))
        .doc("Abre a próxima sessão que espera você ou terminou sem ser lida, em qualquer workspace (na janela dele).");
    r.add("ui.workspace.close", "Close workspace tab", "workspace", Some("Ctrl+W"), |app, _| {
        let id = app.data.active_workspace.clone().ok_or_else(|| t("uiErrors.noWorkspace"))?;
        app.close_workspace(&id);
        ok()
    });
    r.add("ui.workspace.next", "Next workspace", "workspace", Some("Ctrl+PageDown"), |app, _| step_workspace(app, 1));
    r.add("ui.workspace.prev", "Previous workspace", "workspace", Some("Ctrl+PageUp"), |app, _| step_workspace(app, -1));
    for n in 1..=9usize {
        let sc = format!("Ctrl+Alt+{n}");
        r.add(&format!("ui.workspace.switch-{n}"), &format!("Workspace {n}"), "workspace", Some(&sc), move |app, _| {
            let id = crate::windows::open_everywhere(app).get(n - 1).cloned().ok_or_else(|| t("uiErrors.noWorkspaceAt"))?;
            app.open_workspace(&id, None);
            ok()
        });
    }
    r.add("ui.workspace.refresh", "Refresh", "workspace", Some("F5"), |app, _| {
        app.refresh_all();
        if let Some(id) = app.state.selected.clone() {
            app.refresh_thread_meta(&id);
        }
        crate::views::git::refresh_current(app);
        ok()
    });
    r.add("ui.workspace.edit", "Edit workspace", "workspace", None, |app, _| {
        app.state.settings_workspace = app.data.active_workspace.clone();
        app.state.settings_tab = SettingsTab::Workspaces;
        app.state.settings_open = true;
        ok()
    });
    r.add("ui.workspace.set-path", "Set new workspace folder", "workspace", None, |app, p| {
        app.eph.new_workspace_path = str_param(p, "path")?;
        if app.eph.new_workspace_name.trim().is_empty() {
            app.eph.new_workspace_name =
                std::path::Path::new(&app.eph.new_workspace_path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        }
        app.state.new_workspace_open = true;
        ok()
    })
    .params(vec![ParamInfo::req("path", ParamType::Path)]);
    r.add("ui.repo.open", "Open repository screen", "workspace", None, |app, p| {
        let id = str_param(p, "projectId")?;
        app.state.repo_project = Some(id);
        app.state.selected = None;
        if app.state.tab == MainTab::Chat {
            app.state.tab = MainTab::History;
        }
        ok()
    })
    .params(vec![ParamInfo::req("projectId", ParamType::String)]);

    // ---------- aparência ----------
    let mut themes: Vec<(&str, &str)> = vec![("system", "appearance.themeSystem")];
    themes.extend(THEMES.iter().map(|t| (t.id, t.label)));
    for (id, label) in themes {
        r.add(&format!("ui.theme.{id}"), label, "appearance", None, move |app, _| {
            app.state.appearance.theme = id.into();
            ok()
        });
    }
    r.add("ui.theme.next", "Next theme", "appearance", None, |app, _| {
        let ids: Vec<&str> = std::iter::once("system").chain(THEMES.iter().map(|t| t.id)).collect();
        let i = ids.iter().position(|x| *x == app.state.appearance.theme).map_or(0, |i| (i + 1) % ids.len());
        app.state.appearance.theme = ids.get(i).copied().unwrap_or("system").into();
        ok()
    });
    for (d, label) in [
        (Density::Compact, "appearance.densityCompact"),
        (Density::Normal, "appearance.densityNormal"),
        (Density::Comfortable, "appearance.densityComfortable"),
    ] {
        r.add(&format!("ui.density.{}", format!("{d:?}").to_lowercase()), label, "appearance", None, move |app, _| {
            app.state.appearance.density = d;
            ok()
        });
    }
    for (w, label) in [
        (ContentWidth::Narrow, "appearance.widthNarrow"),
        (ContentWidth::Normal, "appearance.widthNormal"),
        (ContentWidth::Wide, "appearance.widthWide"),
        (ContentWidth::Full, "appearance.widthFull"),
    ] {
        r.add(&format!("ui.width.{}", format!("{w:?}").to_lowercase()), label, "appearance", None, move |app, _| {
            app.state.appearance.content_width = w;
            ok()
        });
    }
    r.add("ui.zoom.in", "Zoom in", "appearance", Some("Ctrl+Plus"), |app, _| zoom(app, 10));
    r.add("ui.zoom.in-alt", "Zoom in", "appearance", Some("Ctrl+Equals"), |app, _| zoom(app, 10));
    r.add("ui.zoom.out", "Zoom out", "appearance", Some("Ctrl+Minus"), |app, _| zoom(app, -10));
    r.add("ui.zoom.reset", "Reset zoom", "appearance", Some("Ctrl+0"), |app, _| {
        app.state.appearance.scale = 100;
        ok()
    });
    for (field, name) in [("text", "Text"), ("code", "Code"), ("terminal", "Terminal")] {
        for (dir, delta) in [("bigger", 1.0f32), ("smaller", -1.0)] {
            r.add(&format!("ui.font.{field}.{dir}"), &format!("{name} font {dir}"), "appearance", None, move |app, _| {
                let a = &mut app.state.appearance;
                let f = match field {
                    "text" => &mut a.text_size,
                    "code" => &mut a.code_size,
                    _ => &mut a.terminal_size,
                };
                *f = (*f + delta).clamp(9.0, 24.0);
                ok()
            });
        }
    }
    r.add("ui.appearance.reset", "Reset appearance", "appearance", None, |app, _| {
        app.state.appearance = Default::default();
        ok()
    });
    for (id, label, _) in lazyaicode_i18n::LOCALES {
        r.add(&format!("ui.language.{id}"), label, "appearance", None, move |app, _| {
            lazyaicode_i18n::set_locale(id);
            if let Some(s) = &mut app.data.settings {
                s.language = id.into();
            }
            app.fire("settings.language", json!({ "value": id }));
            ok()
        });
    }

    // ---------- terminal ----------
    r.add("ui.terminal.restart", "Restart terminal", "terminal", None, |app, _| crate::terminal::restart_current(app));
    r.add("ui.terminal.clear", "Clear terminal", "terminal", Some("Ctrl+Shift+K"), |app, _| crate::terminal::write_current(app, "\x0c"));
    r.add("ui.terminal.interrupt", "Send Ctrl+C", "terminal", None, |app, _| crate::terminal::write_current(app, "\x03"));
    r.add("ui.terminal.copy-screen", "Copy terminal screen", "terminal", None, |app, _| {
        let text = crate::terminal::current(app).map(|t| t.term.text()).ok_or_else(|| t("uiErrors.terminalClosed"))?;
        copy(app, text)
    });
    r.add("ui.terminal.write", "Type into terminal", "terminal", None, |app, p| {
        crate::terminal::write_current(app, &str_param(p, "text")?)
    })
    .params(vec![ParamInfo::req("text", ParamType::String)]);
    r.add("ui.terminal.run", "Run in terminal", "terminal", None, |app, p| {
        app.state.tab = MainTab::Terminal;
        crate::terminal::write_current(app, &format!("{}\r", str_param(p, "command")?))
    })
    .params(vec![ParamInfo::req("command", ParamType::String)]);

    // ---------- janela ----------
    r.add("ui.window.fullscreen", "Toggle full screen", "window", Some("F11"), |app, _| {
        crate::views::titlebar::viewport(app, "fullscreen");
        ok()
    });
    r.add("ui.window.minimize", "windowControls.minimize", "window", None, |app, _| {
        crate::views::titlebar::viewport(app, "minimize");
        ok()
    });
    r.add("ui.window.maximize", "windowControls.maximize", "window", None, |app, _| {
        crate::views::titlebar::viewport(app, "maximize");
        ok()
    });
    r.add("ui.app.check-update", "update.check", "window", None, |app, _| {
        app.check_update(true);
        ok()
    })
    .doc("Consulta o último release e avisa se há versão nova (ou que esta já é a mais nova).");
    r.add("ui.app.quit", "Quit", "window", Some("Ctrl+Q"), |app, _| {
        // sair fecha todas as janelas (fechar só uma é o X dela)
        app.eph.quitting = true;
        crate::views::titlebar::viewport(app, "close");
        ok()
    });

    more(&mut r);
    crate::views::git::register_ui_commands(&mut |id: &str,
                                                  label: &str,
                                                  group: &'static str,
                                                  shortcut: Option<&str>,
                                                  params: Vec<ParamInfo>,
                                                  run: RunBox| {
        r.add(id, label, group, shortcut, run).params(params);
    });
    r.0
}

/// Mais ações de tela: navegação fina, composer, revisão, janela e
/// depuração.
fn more(r: &mut Reg) {
    r.add("ui.sidebar.clear-filter", "sidebar.clear", "view", None, |app, _| {
        app.state.filter.clear();
        ok()
    });
    r.add("ui.sidebar.show-all", "Show every thread", "view", None, |app, _| {
        for g in crate::views::sidebar::group_keys(app) {
            app.state.show_all.insert(g, true);
        }
        ok()
    });
    r.add("ui.sidebar.width-reset", "Reset sidebar width", "view", None, |app, _| {
        app.state.sidebar_width = 264.0;
        ok()
    });
    r.add("ui.thread.first", "First thread", "thread", Some("Alt+Home"), |app, _| {
        let id = crate::views::sidebar::visible_thread_ids(app).first().cloned().ok_or_else(|| t("palette.threadNotFound"))?;
        app.select_thread(&id);
        ok()
    });
    r.add("ui.thread.last", "Last thread", "thread", Some("Alt+End"), |app, _| {
        let id = crate::views::sidebar::visible_thread_ids(app).last().cloned().ok_or_else(|| t("palette.threadNotFound"))?;
        app.select_thread(&id);
        ok()
    });
    r.add("ui.thread.copy-last-message", "Copy last agent message", "thread", Some("Ctrl+Shift+C"), |app, _| {
        let text = current_items(app)
            .filter_map(|i| match i {
                lazyaicode_doc::transcript::Item::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .last()
            .ok_or_else(|| t("uiErrors.noReply"))?;
        copy(app, text)
    });
    r.add("ui.thread.copy-id", "Copy thread id", "thread", None, |app, _| {
        let id = sel(app)?;
        copy(app, id)
    });
    r.add("ui.thread.duplicate", "Duplicate thread", "thread", None, |app, _| {
        let id = sel(app)?;
        app.call_ok("thread.duplicate", json!({ "id": id }), |app, v| {
            if let Ok(t) = serde_json::from_value::<lazyaicode_doc::ThreadView>(v) {
                let id = t.thread.id.clone();
                app.data.upsert(t);
                app.select_thread(&id);
            }
        });
        ok()
    });
    r.add("ui.thread.commit", "Commit the thread checkout", "review", None, |app, p| {
        let id = sel(app)?;
        let msg = p
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| app.selected().map(|t| t.thread.title.clone()).unwrap_or_default());
        app.call_ok("thread.commit", json!({ "id": id, "message": msg }), |app, v| {
            app.toast(format!("✓ {}", v.as_str().unwrap_or("")), ToastKind::Ok)
        });
        ok()
    })
    .params(vec![ParamInfo::opt("message", ParamType::String)]);
    r.add("ui.thread.push", "Push the thread branch", "review", None, |app, _| {
        let id = sel(app)?;
        app.call_ok("thread.push", json!({ "id": id }), |app, _| app.toast(t("gitRemote.pushed"), ToastKind::Ok));
        ok()
    });
    r.add("ui.thread.update-from-base", "Update thread from its base", "review", None, |app, _| {
        let id = sel(app)?;
        app.call_ok("thread.update-from-base", json!({ "id": id }), |app, _| app.toast("✓", ToastKind::Ok));
        ok()
    });
    r.add("ui.thread.collapse-thinking", "Hide reasoning", "thread", None, |app, _| {
        app.eph.thinking_open.clear();
        ok()
    });
    r.add("ui.thread.scroll-top", "Scroll to the start", "thread", Some("Ctrl+Home"), |app, _| {
        app.eph.scroll_top = true;
        ok()
    });
    r.add("ui.thread.terminal", "Open the thread terminal", "thread", Some("Ctrl+Backquote"), |app, _| {
        app.state.tab = MainTab::Terminal;
        ok()
    });
    r.add("ui.composer.insert", "Insert text in the message box", "thread", None, |app, p| {
        let id = sel(app)?;
        let text = str_param(p, "text")?;
        app.data.drafts.entry(id).or_default().push_str(&text);
        app.eph.focus_composer = true;
        ok()
    })
    .params(vec![ParamInfo::req("text", ParamType::String)]);
    r.add("ui.composer.slash", "Start a slash command", "thread", None, |app, p| {
        let id = sel(app)?;
        let name = p.get("name").and_then(Value::as_str).unwrap_or("");
        app.data.drafts.insert(id, format!("/{name}"));
        app.eph.slash_dismissed = !name.is_empty();
        app.eph.focus_composer = true;
        app.state.tab = MainTab::Chat;
        ok()
    })
    .params(vec![ParamInfo::opt("name", ParamType::String)]);
    r.add("ui.composer.recall", "Recall the last message", "thread", None, |app, _| {
        let id = sel(app)?;
        let last = current_items(app)
            .filter_map(|i| match i {
                lazyaicode_doc::transcript::Item::User { text, .. } => Some(text.clone()),
                _ => None,
            })
            .last()
            .ok_or_else(|| t("uiErrors.noMessage"))?;
        app.data.drafts.insert(id, last);
        app.eph.focus_composer = true;
        ok()
    });
    r.add("ui.review.first-file", "First changed file", "review", None, |app, _| crate::views::review::step_file(app, -10_000));
    r.add("ui.review.last-file", "Last changed file", "review", None, |app, _| crate::views::review::step_file(app, 10_000));
    r.add("ui.review.open-file", "Open a file in the review", "review", None, |app, p| {
        app.state.tab = MainTab::History;
        app.state.history_view = HistoryView::Review;
        app.state.review_file = Some(str_param(p, "path")?);
        ok()
    })
    .params(vec![ParamInfo::req("path", ParamType::String)]);
    r.add("ui.review.discard-file", "Revert file to the thread base", "review", None, |app, p| {
        let id = sel(app)?;
        let path = p
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| app.state.review_file.clone())
            .ok_or_else(|| t("uiErrors.noFile"))?;
        app.eph.confirm = Some((
            lazyaicode_i18n::tf("gitWorking.confirmDiscard", &[("path", &path)]),
            "thread.discard-file".into(),
            json!({ "id": id, "path": path }),
        ));
        app.state.dialog = Some("confirm".into());
        ok()
    })
    .params(vec![ParamInfo::opt("path", ParamType::String)]);
    r.add("ui.review.copy-patch", "Copy the file diff", "review", None, |app, _| {
        let id = sel(app)?;
        let path = app.state.review_file.clone().ok_or_else(|| t("uiErrors.noFile"))?;
        let patch = app
            .data
            .changes
            .get(&id)
            .and_then(|c| {
                c["files"]
                    .as_array()
                    .and_then(|f| f.iter().find(|x| x["path"] == path.as_str()).and_then(|x| x["patch"].as_str().map(str::to_string)))
            })
            .ok_or_else(|| t("uiErrors.noDiff"))?;
        copy(app, patch)
    });
    r.add("ui.repo.open-current", "Open the repository screen", "workspace", None, |app, _| {
        let pid = app.current_project().map(|p| p.id.clone()).ok_or_else(|| t("uiErrors.noProject"))?;
        run_ui(app, "ui.repo.open", &json!({ "projectId": pid }))
    });
    r.add("ui.repo.close", "repoScreen.close", "workspace", None, |app, _| {
        app.state.repo_project = None;
        ok()
    });
    r.add("ui.palette.open", "Open the palette with text", "view", None, |app, p| {
        app.state.palette_open = true;
        app.state.palette_query = p.get("query").and_then(Value::as_str).unwrap_or("").to_string();
        app.eph.palette_index = 0;
        ok()
    })
    .params(vec![ParamInfo::opt("query", ParamType::String)]);
    r.add("ui.palette.threads", "Go to thread", "view", Some("Ctrl+P"), |app, _| {
        app.state.palette_open = true;
        app.state.palette_query.clear();
        ok()
    });
    r.add("ui.settings.toggle", "Toggle settings", "view", None, |app, _| {
        app.state.settings_open = !app.state.settings_open;
        ok()
    });
    r.add("ui.notifications.test", "Test a notification", "help", None, |app, _| {
        app.host.notify("LazyAICode", &t("app.done"));
        app.toast(t("app.done"), ToastKind::Info);
        ok()
    });
    r.add("ui.density.toggle", "Toggle compact density", "appearance", None, |app, _| {
        let a = &mut app.state.appearance;
        a.density = if a.density == Density::Compact { Density::Normal } else { Density::Compact };
        ok()
    });
    r.add("ui.language.next", "Next language", "appearance", None, |app, _| {
        let ids: Vec<&str> = lazyaicode_i18n::LOCALES.iter().map(|l| l.0).collect();
        let cur = ids.iter().position(|x| *x == lazyaicode_i18n::locale()).unwrap_or(0);
        let next = ids.get((cur + 1) % ids.len()).copied().unwrap_or("en");
        run_ui(app, &format!("ui.language.{next}"), &Value::Null)
    });
    r.add("ui.zoom.set", "Set zoom", "appearance", None, |app, p| {
        let v = p.get("percent").and_then(Value::as_u64).ok_or("falta `percent`")?;
        app.state.appearance.scale = (v as u32).clamp(50, 250);
        ok()
    })
    .params(vec![ParamInfo::req("percent", ParamType::Int)]);
    r.add("ui.window.always-on-top", "Keep window on top", "window", None, |app, _| {
        crate::views::titlebar::viewport(app, "on-top");
        ok()
    });
    r.add("ui.debug.reload-commands", "Reload the command list", "help", None, |app, _| {
        app.call_ok("engine.commands", Value::Null, |app, v| {
            let mut cmds: Vec<CommandInfo> = serde_json::from_value(v).unwrap_or_default();
            cmds.extend(ui_command_infos());
            app.data.commands = cmds;
        });
        ok()
    });
    r.add("ui.debug.reload-settings", "Reload settings", "help", None, |app, _| {
        app.refresh_settings();
        ok()
    });
    r.add("ui.debug.clear-caches", "Clear screen caches", "help", None, |app, _| {
        app.data.caps.clear();
        app.data.changes.clear();
        app.data.diff_stats.clear();
        app.data.transcripts.clear();
        app.eph.git.repos.clear();
        app.terms.clear();
        if let Some(id) = app.state.selected.clone() {
            app.load_transcript(&id);
        }
        ok()
    });
    r.add("ui.debug.health", "Check the machine", "help", None, |app, _| {
        app.call_ok("system.health", Value::Null, |app, v| {
            app.eph.palette_result = Some(("system.health".into(), serde_json::to_string_pretty(&v).unwrap_or_default()));
        });
        ok()
    });
    r.add("ui.debug.journal", "Show the session journal", "help", None, |app, _| {
        app.call_ok("engine.journal", Value::Null, |app, v| {
            app.eph.palette_result = Some(("engine.journal".into(), serde_json::to_string_pretty(&v).unwrap_or_default()));
        });
        ok()
    });
    r.add("ui.debug.state", "Show the screen state", "help", None, |app, _| {
        let v = crate::control::inspect(app);
        app.eph.palette_result = Some(("ui.inspect".into(), serde_json::to_string_pretty(&v).unwrap_or_default()));
        Ok(Value::Null)
    });
}

fn registry() -> &'static [UiCmd] {
    static R: OnceLock<Vec<UiCmd>> = OnceLock::new();
    R.get_or_init(build)
}

pub fn ui_commands() -> &'static [UiCmd] {
    registry()
}

/// Os comandos de tela no mesmo formato dos do motor.
pub fn ui_command_infos() -> Vec<CommandInfo> {
    registry()
        .iter()
        .map(|c| CommandInfo {
            id: c.id.clone(),
            label: c.label.clone(),
            group: c.group.into(),
            menu: vec![],
            shortcut: c.shortcut.clone(),
            params: c.params.clone(),
            doc: c.doc.into(),
            returns: String::new(),
            read_only: matches!(c.id.as_str(), "ui.inspect" | "ui.screenshot"),
            destructive: c.id == "ui.thread.delete",
            scope: "ui".into(),
        })
        .collect()
}

/// Executa um comando de tela (ou, com id do motor, o comando do motor com
/// o contexto da tela).
pub fn run_ui(app: &mut LazyApp, id: &str, params: &Value) -> Result<Value, String> {
    if let Some(c) = registry().iter().find(|c| c.id == id) {
        lazyaicode_protocol::check_params(&c.params, params)?;
        let run = c.run.clone();
        return run(app, params);
    }
    if app.data.command(id).is_some() {
        return engine(app, id, params.clone());
    }
    Err(format!("comando desconhecido: {id}"))
}

/// Parâmetros que a tela sabe: a thread aberta (`id`/`threadId`), o
/// workspace ativo, o projeto e o repositório git da tela.
pub fn fill_context(app: &LazyApp, info: &CommandInfo, m: &mut Map<String, Value>) {
    let thread = app.state.selected.clone();
    let project = app.current_project().map(|p| p.id.clone());
    let repo = crate::views::git::current_repo_id(app);
    let path = app.current_path();
    for p in &info.params {
        if m.contains_key(&p.name) {
            continue;
        }
        let v = match p.name.as_str() {
            "id" if info.id.starts_with("thread.") => thread.clone().map(Value::String),
            "threadId" => thread.clone().map(Value::String),
            "workspaceId" => app.data.active_workspace.clone().map(Value::String),
            "projectId" => project.clone().map(Value::String),
            "repoId" => repo.clone().map(Value::String),
            "cwd" | "path" if info.id.starts_with("terminal.") || info.id == "system.open-path" => path.clone().map(Value::String),
            _ => None,
        };
        if let Some(v) = v {
            m.insert(p.name.clone(), v);
        }
    }
}

/// Falta algum parâmetro obrigatório depois do contexto?
pub fn missing_params(info: &CommandInfo, m: &Map<String, Value>) -> Vec<String> {
    info.params.iter().filter(|p| p.required && !m.contains_key(&p.name)).map(|p| p.name.clone()).collect()
}

fn set_tab(app: &mut LazyApp, tab: MainTab) -> Result<Value, String> {
    let git = app.current_project().is_some_and(|p| p.is_git);
    if tab.is_git() && !git {
        return Err(t("mainTabs.notGit"));
    }
    app.state.tab = tab;
    if tab == MainTab::Chat {
        app.eph.focus_composer = true;
    }
    ok()
}

fn cycle_tab(app: &mut LazyApp, d: i32) -> Result<Value, String> {
    let git = app.current_project().is_some_and(|p| p.is_git);
    let tabs: Vec<MainTab> = MAIN_TABS.into_iter().filter(|t| git || !t.is_git()).collect();
    let i = tabs.iter().position(|t| *t == app.state.tab).unwrap_or(0) as i32;
    let n = tabs.len() as i32;
    if let Some(t) = tabs.get(((i + d).rem_euclid(n.max(1))) as usize) {
        app.state.tab = *t;
    }
    ok()
}

fn step_thread(app: &mut LazyApp, d: i32) -> Result<Value, String> {
    let ids = crate::views::sidebar::visible_thread_ids(app);
    if ids.is_empty() {
        return ok();
    }
    let cur = app.state.selected.as_ref().and_then(|s| ids.iter().position(|x| x == s));
    let next = match cur {
        Some(i) => (i as i32 + d).clamp(0, ids.len() as i32 - 1) as usize,
        None => 0,
    };
    if let Some(id) = ids.get(next).cloned() {
        app.select_thread(&id);
    }
    ok()
}

fn step_workspace(app: &mut LazyApp, d: i32) -> Result<Value, String> {
    let ids = crate::windows::open_everywhere(app);
    let Some(cur) = app.data.active_workspace.as_ref().and_then(|a| ids.iter().position(|x| x == a)) else { return ok() };
    let n = ids.len() as i32;
    if let Some(id) = ids.get(((cur as i32 + d).rem_euclid(n.max(1))) as usize).cloned() {
        app.open_workspace(&id, None);
    }
    ok()
}

fn zoom(app: &mut LazyApp, d: i32) -> Result<Value, String> {
    let s = app.state.appearance.scale as i32 + d;
    app.state.appearance.scale = s.clamp(50, 250) as u32;
    ok()
}

fn copy(app: &mut LazyApp, text: String) -> Result<Value, String> {
    app.eph.copy_request = Some(text);
    app.toast(t("app.copied"), ToastKind::Ok);
    ok()
}

fn current_items(app: &LazyApp) -> impl Iterator<Item = &lazyaicode_doc::transcript::Item> {
    app.state.selected.as_ref().and_then(|id| app.data.transcripts.get(id)).into_iter().flat_map(|t| t.items.iter())
}

fn set_tools_open(app: &mut LazyApp, open: bool) -> Result<Value, String> {
    let keys: Vec<String> =
        current_items(app).filter(|i| matches!(i, lazyaicode_doc::transcript::Item::Tool { .. })).map(|i| i.key().to_string()).collect();
    for k in keys {
        app.eph.tool_open.insert(k, open);
    }
    ok()
}

fn set_thread(app: &mut LazyApp, cmd: &str, p: Value) -> Result<Value, String> {
    app.call_ok(cmd, p, |app, v| {
        if let Ok(t) = serde_json::from_value(v) {
            app.data.upsert(t);
        }
    });
    ok()
}

/// Nova thread no projeto dado, no da thread aberta ou no primeiro do workspace.
pub fn new_thread(app: &mut LazyApp, p: &Value, isolated: bool) -> Result<Value, String> {
    let ws = app.data.active_workspace.clone().ok_or_else(|| t("uiErrors.noWorkspace"))?;
    let project = p
        .get("projectId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| app.current_project().map(|p| p.id.clone()))
        .or_else(|| app.data.projects.first().map(|p| p.id.clone()))
        .ok_or_else(|| t("sidebar.noProjects"))?;
    let settings = app.data.settings.clone().unwrap_or_default();
    let params = json!({
        "workspaceId": ws,
        "projectId": project,
        "isolated": isolated,
        "provider": settings.default_provider,
    });
    app.call_ok("thread.create", params, |app, v| {
        if let Ok(t) = serde_json::from_value::<lazyaicode_doc::ThreadView>(v) {
            let id = t.thread.id.clone();
            app.data.upsert(t);
            app.data.transcripts.insert(id.clone(), Default::default());
            app.state.tab = MainTab::Chat;
            app.select_thread(&id);
        }
    });
    ok()
}

/// `Ctrl+Shift+N` → atalho do egui. `Ctrl` é ⌘ no macOS.
pub fn parse_shortcut(s: &str) -> Option<egui::KeyboardShortcut> {
    let mut mods = egui::Modifiers::NONE;
    let mut key = None;
    for part in s.split('+') {
        match part.trim() {
            "Ctrl" | "Cmd" => mods |= egui::Modifiers::COMMAND,
            "Shift" => mods |= egui::Modifiers::SHIFT,
            "Alt" => mods |= egui::Modifiers::ALT,
            "Comma" => key = Some(egui::Key::Comma),
            other => key = egui::Key::from_name(other),
        }
    }
    key.map(|k| egui::KeyboardShortcut::new(mods, k))
}

/// O atalho efetivo de um comando (o do usuário, ou o padrão).
pub fn shortcut_of(app: &LazyApp, c: &UiCmd) -> Option<String> {
    app.state.shortcuts.get(&c.id).cloned().or_else(|| c.shortcut.clone()).filter(|s| !s.is_empty())
}

/// Atalhos de teclado → comandos. Com um campo de texto em foco, só os que
/// têm Ctrl/Alt ou tecla de função (Esc e Shift+Tab ficam com o campo).
pub fn handle_shortcuts(app: &mut LazyApp, ctx: &egui::Context) {
    let typing = ctx.egui_wants_keyboard_input();
    let mut fire: Vec<String> = Vec::new();
    for c in registry() {
        let Some(sc) = shortcut_of(app, c).and_then(|s| parse_shortcut(&s)) else { continue };
        let fkey = matches!(sc.logical_key, egui::Key::F1 | egui::Key::F2 | egui::Key::F5 | egui::Key::F11);
        let modded = sc.modifiers.command || sc.modifiers.alt || sc.modifiers.ctrl;
        if typing && !(modded || fkey) {
            continue;
        }
        // na aba Terminal as teclas vão para o shell (menos as de troca de aba e paleta)
        if app.state.tab == MainTab::Terminal
            && typing
            && !(c.id.starts_with("ui.tab.")
                || c.id == "ui.palette.toggle"
                || c.id.starts_with("ui.workspace.")
                || c.id == "ui.sidebar.toggle")
        {
            continue;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc)) {
            fire.push(c.id.clone());
        }
    }
    for id in fire {
        if let Err(e) = run_ui(app, &id, &Value::Null) {
            app.toast_err(&e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_commands_are_unique_well_formed_and_their_shortcuts_parse() {
        let mut ids = std::collections::HashSet::new();
        let mut shortcuts = std::collections::HashMap::new();
        for c in registry() {
            assert!(lazyaicode_protocol::valid_command_id(&c.id), "{}", c.id);
            assert!(c.id.starts_with("ui."), "{}", c.id);
            assert!(ids.insert(c.id.clone()), "repetido: {}", c.id);
            if let Some(s) = &c.shortcut {
                assert!(parse_shortcut(s).is_some(), "{}: atalho {s}", c.id);
                if let Some(other) = shortcuts.insert(s.clone(), c.id.clone()) {
                    panic!("{s} está em {other} e {}", c.id);
                }
            }
        }
        assert!(registry().len() > 150, "{}", registry().len());
    }
}

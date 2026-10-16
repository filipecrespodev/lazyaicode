//! O app: estado de tela, cópia dos dados do motor e o laço de cada quadro
//! (receber respostas e eventos → desenhar → mandar comandos).

use std::collections::HashMap;

use lazyaicode_doc::transcript::{Decision, Transcript};
use lazyaicode_doc::{Status, ThreadView};
use lazyaicode_i18n::{t, tf};
use lazyaicode_protocol::{events, CommandInfo, ErrorBody};
use serde_json::{json, Value};

use crate::client::{Client, Incoming, Ticket};
use crate::data::Data;
use crate::host::{Host, Inbox, UiMsg};
use crate::state::{MainTab, UiState};
use crate::terminal::TermView;
use crate::theme::{self, Tokens};

pub type Callback = Box<dyn FnOnce(&mut LazyApp, Result<Value, ErrorBody>)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Ok,
    Error,
}

pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
    pub until: f64,
}

/// Estado passageiro das telas (o que não interessa ao `ui.inspect`).
#[derive(Default)]
pub struct Ephemeral {
    pub editing_title: Option<String>,
    pub thinking_open: HashMap<String, bool>,
    pub tool_open: HashMap<String, bool>,
    pub slash_index: usize,
    pub slash_dismissed: bool,
    pub palette_index: usize,
    /// comando escolhido na paleta que ainda precisa de parâmetros
    pub palette_form: Option<(CommandInfo, serde_json::Map<String, Value>)>,
    pub new_workspace_path: String,
    pub new_workspace_name: String,
    pub new_workspace_account: Option<String>,
    pub picker_checked: Vec<String>,
    pub preview_image: Option<String>,
    pub scroll_to_bottom: bool,
    pub focus_composer: bool,
    pub confirm: Option<(String, String, Value)>,
    pub land_open: bool,
    pub git: crate::views::git::GitUi,
    pub settings_ws_name: String,
    pub ignored_draft: Vec<String>,
    /// resultado de um comando de leitura rodado pela paleta (título, JSON)
    pub palette_result: Option<(String, String)>,
    /// rolar a conversa para o começo no próximo quadro
    pub scroll_top: bool,
    /// texto para a área de transferência (precisa do `ctx`)
    pub copy_request: Option<String>,
    /// comandos de janela pendentes (`minimize`, `maximize`, `close`, `fullscreen`)
    pub viewport: Vec<&'static str>,
}

pub struct LazyApp {
    pub(crate) client: Box<dyn Client>,
    pub(crate) host: Box<dyn Host>,
    pub(crate) inbox: Inbox,
    pending: HashMap<Ticket, Callback>,
    /// O estado de tela (o que o canal de controle vê e muda).
    pub state: UiState,
    pub(crate) data: Data,
    pub(crate) eph: Ephemeral,
    pub(crate) tokens: Tokens,
    applied: Option<(String, bool, u32, u32, u32, u32)>,
    pub(crate) toasts: Vec<Toast>,
    pub(crate) md: egui_commonmark::CommonMarkCache,
    pub(crate) terms: HashMap<String, TermView>,
    pub(crate) system_dark: bool,
    pub(crate) focused: bool,
    pub(crate) now: f64,
    /// pedidos de captura de tela esperando o quadro (`ui.screenshot`)
    pub(crate) screenshot_waiters: Vec<std::sync::mpsc::Sender<Result<Value, ErrorBody>>>,
    /// a próxima captura usa o renderizador em CPU
    pub(crate) screenshot_cpu: bool,
    pub(crate) last_persisted: Option<String>,
}

impl LazyApp {
    pub fn new(client: Box<dyn Client>, host: Box<dyn Host>, inbox: Inbox) -> Self {
        let state = UiState::default();
        let tokens = Tokens::new(theme::resolve(&state.appearance.theme, true), &state.appearance);
        let mut app = Self {
            client,
            host,
            inbox,
            pending: HashMap::new(),
            state,
            data: Data::default(),
            eph: Ephemeral::default(),
            tokens,
            applied: None,
            toasts: vec![],
            md: egui_commonmark::CommonMarkCache::default(),
            terms: HashMap::new(),
            system_dark: true,
            focused: true,
            now: 0.0,
            screenshot_waiters: vec![],
            screenshot_cpu: false,
            last_persisted: None,
        };
        app.boot();
        app
    }

    pub fn inbox(&self) -> Inbox {
        self.inbox.clone()
    }

    // ---------- chamadas ao motor ----------

    /// Manda um método; `then` roda com o resultado num quadro seguinte.
    pub fn call(&mut self, method: &str, params: Value, then: impl FnOnce(&mut LazyApp, Result<Value, ErrorBody>) + 'static) {
        let ticket = self.client.call(method, params);
        self.pending.insert(ticket, Box::new(then));
    }

    /// Como [`call`], com erro virando aviso e o resultado ignorado.
    pub fn fire(&mut self, method: &str, params: Value) {
        self.call(method, params, |app, r| {
            if let Err(e) = r {
                app.toast_err(&e.message);
            }
        });
    }

    /// Chama e, dando certo, roda `ok`; dando errado, mostra o erro.
    pub fn call_ok(&mut self, method: &str, params: Value, ok: impl FnOnce(&mut LazyApp, Value) + 'static) {
        self.call(method, params, move |app, r| match r {
            Ok(v) => ok(app, v),
            Err(e) => app.toast_err(&e.message),
        });
    }

    pub fn toast(&mut self, text: impl Into<String>, kind: ToastKind) {
        let secs = if kind == ToastKind::Error { 8.0 } else { 4.0 };
        self.toasts.push(Toast { text: text.into(), kind, until: self.now + secs });
        if self.toasts.len() > 5 {
            self.toasts.remove(0);
        }
    }

    pub fn toast_err(&mut self, text: &str) {
        self.toast(text.to_string(), ToastKind::Error);
    }

    // ---------- abertura ----------

    fn boot(&mut self) {
        self.call_ok("engine.commands", Value::Null, |app, v| {
            let mut cmds: Vec<CommandInfo> = serde_json::from_value(v).unwrap_or_default();
            cmds.extend(crate::commands::ui_command_infos());
            app.data.commands = cmds;
        });
        self.call_ok("engine.version", Value::Null, |app, v| app.data.version = Some(v));
        self.refresh_settings();
        self.call("prefs.get", Value::Null, |app, r| {
            if let Ok(Value::Object(prefs)) = r {
                app.restore_prefs(&prefs);
                app.data.prefs = prefs;
            }
            app.refresh_workspaces(true);
        });
        self.call_ok("system.user-profile", Value::Null, |app, v| app.data.profile = Some(v));
        self.call_ok("account.list", Value::Null, |app, v| app.data.accounts = v.as_array().cloned().unwrap_or_default());
    }

    pub(crate) fn refresh_settings(&mut self) {
        self.call_ok("settings.get", Value::Null, |app, v| {
            if let Ok(s) = serde_json::from_value::<lazyaicode_doc::Settings>(v["settings"].clone()) {
                lazyaicode_i18n::set_locale(&s.language);
                app.data.settings = Some(s);
            }
            app.data.claude_binary = v["claudeBinary"].as_str().map(str::to_string);
            app.data.default_claude_dir = v["defaultClaudeDir"].as_str().unwrap_or("").to_string();
            app.data.suggested_claude_dir = v["suggestedClaudeDir"].as_str().unwrap_or("").to_string();
            app.data.data_dir = v["dataDir"].as_str().unwrap_or("").to_string();
        });
    }

    fn restore_prefs(&mut self, p: &serde_json::Map<String, Value>) {
        let get = |k: &str| p.get(k).cloned().unwrap_or(Value::Null);
        if let Ok(a) = serde_json::from_value(get("ui.appearance")) {
            self.state.appearance = a;
        }
        if let Ok(v) = serde_json::from_value::<Vec<String>>(get("ui.openWorkspaces")) {
            self.state.open_workspaces = v;
        }
        if let Some(b) = get("ui.sidebarOpen").as_bool() {
            self.state.sidebar_open = b;
        }
        if let Some(w) = get("ui.sidebarWidth").as_f64() {
            self.state.sidebar_width = (w as f32).clamp(180.0, 600.0);
        }
        if let Ok(m) = serde_json::from_value(get("ui.treeMode")) {
            self.state.tree_mode = m;
        }
        if let Ok(s) = serde_json::from_value(get("ui.shortcuts")) {
            self.state.shortcuts = s;
        }
        if let Ok(c) = serde_json::from_value(get("ui.collapsed")) {
            self.state.collapsed = c;
        }
    }

    /// Grava as preferências de tela que mudaram (no máximo uma vez por quadro).
    fn persist_prefs(&mut self) {
        let snapshot = json!({
            "ui.appearance": self.state.appearance,
            "ui.openWorkspaces": self.state.open_workspaces,
            "ui.sidebarOpen": self.state.sidebar_open,
            "ui.sidebarWidth": self.state.sidebar_width.round(),
            "ui.treeMode": self.state.tree_mode,
            "ui.shortcuts": self.state.shortcuts,
            "ui.collapsed": self.state.collapsed,
        });
        let s = snapshot.to_string();
        if !self.data.booted || self.last_persisted.as_deref() == Some(s.as_str()) {
            return;
        }
        let first = self.last_persisted.is_none();
        self.last_persisted = Some(s);
        if first {
            return;
        }
        if let Value::Object(m) = snapshot {
            for (k, v) in m {
                if self.data.prefs.get(&k) != Some(&v) {
                    self.data.prefs.insert(k.clone(), v.clone());
                    self.fire("prefs.set", json!({ "key": k, "value": v }));
                }
            }
        }
    }

    pub(crate) fn refresh_workspaces(&mut self, first: bool) {
        self.call_ok("workspace.list", Value::Null, move |app, v| {
            app.data.workspaces = serde_json::from_value(v["workspaces"].clone()).unwrap_or_default();
            let active = v["active"].as_str().map(str::to_string);
            let known: Vec<String> = app.data.workspaces.iter().map(|w| w.id.clone()).collect();
            app.state.open_workspaces.retain(|id| known.contains(id));
            if first {
                app.data.booted = true;
                for w in app.data.workspaces.clone() {
                    app.refresh_auth(w.claude_dir.clone());
                }
                match app.data.workspaces.len() {
                    0 => app.state.new_workspace_open = true,
                    1 => {
                        let id = app.data.workspaces[0].id.clone();
                        app.switch_workspace(&id);
                    }
                    _ => {
                        // com mais de um, a pessoa escolhe quais abrir (já marcados os da última vez)
                        app.eph.picker_checked = if app.state.open_workspaces.is_empty() {
                            active.clone().into_iter().collect()
                        } else {
                            app.state.open_workspaces.clone()
                        };
                        app.state.picking_workspaces = true;
                    }
                }
            }
            if app.data.active_workspace.is_none() && !first {
                if let Some(a) = active {
                    app.switch_workspace(&a);
                }
            }
        });
    }

    pub(crate) fn refresh_auth(&mut self, dir: Option<String>) {
        let key = dir.clone().unwrap_or_default();
        self.call("account.status", json!({ "claudeDir": dir }), move |app, r| {
            let v = r.unwrap_or_else(|e| json!({ "loggedIn": false, "error": e.message }));
            app.data.auth.insert(key, v);
        });
    }

    pub fn switch_workspace(&mut self, id: &str) {
        if !self.state.open_workspaces.iter().any(|w| w == id) {
            self.state.open_workspaces.push(id.to_string());
        }
        self.state.picking_workspaces = false;
        self.data.active_workspace = Some(id.to_string());
        self.state.selected = None;
        self.state.repo_project = None;
        self.data.projects.clear();
        self.data.threads.clear();
        self.fire("workspace.activate", json!({ "id": id }));
        self.refresh_all();
    }

    pub fn close_workspace(&mut self, id: &str) {
        let Some(i) = self.state.open_workspaces.iter().position(|w| w == id) else { return };
        self.state.open_workspaces.remove(i);
        if self.data.active_workspace.as_deref() != Some(id) {
            return;
        }
        match self.state.open_workspaces.get(i.min(self.state.open_workspaces.len().saturating_sub(1))).cloned() {
            Some(next) => self.switch_workspace(&next),
            None => {
                self.data.active_workspace = None;
                self.state.selected = None;
                self.data.threads.clear();
                self.data.projects.clear();
                self.eph.picker_checked = vec![];
                self.state.picking_workspaces = true;
            }
        }
    }

    /// Recarrega projetos e threads do workspace e importa sessões novas.
    pub fn refresh_all(&mut self) {
        let Some(ws) = self.data.active_workspace.clone() else { return };
        self.call_ok("project.list", json!({ "workspaceId": ws }), |app, v| {
            app.data.projects = serde_json::from_value(v).unwrap_or_default();
        });
        self.refresh_threads();
        let ws2 = ws.clone();
        self.call("thread.sync-sessions", json!({ "workspaceId": ws }), move |app, r| {
            if r.ok().and_then(|v| v.as_u64()).unwrap_or(0) > 0 && app.data.active_workspace.as_deref() == Some(ws2.as_str()) {
                app.refresh_threads();
            }
        });
    }

    pub fn refresh_threads(&mut self) {
        let Some(ws) = self.data.active_workspace.clone() else { return };
        self.call_ok("thread.list", json!({ "workspaceId": ws }), |app, v| {
            app.data.threads = serde_json::from_value(v).unwrap_or_default();
            if let Some(sel) = app.state.selected.clone() {
                if app.data.thread(&sel).is_none() {
                    app.state.selected = None;
                }
            }
        });
    }

    // ---------- threads ----------

    pub fn selected(&self) -> Option<&ThreadView> {
        self.state.selected.as_deref().and_then(|id| self.data.thread(id))
    }

    pub fn select_thread(&mut self, id: &str) {
        self.state.selected = Some(id.to_string());
        self.state.repo_project = None;
        self.eph.scroll_to_bottom = true;
        self.eph.focus_composer = true;
        self.eph.slash_dismissed = false;
        self.state.review_file = None;
        if !self.data.transcripts.contains_key(id) {
            self.load_transcript(id);
        }
        self.refresh_thread_meta(id);
        if self.data.thread(id).is_some_and(|t| t.thread.unread) {
            if let Some(t) = self.data.thread_mut(id) {
                t.thread.unread = false;
            }
            self.fire("thread.update", json!({ "id": id, "unread": false }));
        }
        let project = self.data.thread(id).map(|t| t.thread.project_id.clone());
        let is_git = project.as_deref().and_then(|p| self.data.project(p)).is_some_and(|p| p.is_git);
        if !is_git && self.state.tab.is_git() {
            self.state.tab = MainTab::Chat;
        }
    }

    pub fn load_transcript(&mut self, id: &str) {
        if !self.data.loading_transcript.insert(id.to_string()) {
            return;
        }
        let tid = id.to_string();
        self.call("thread.transcript", json!({ "id": id }), move |app, r| {
            app.data.loading_transcript.remove(&tid);
            let mut tr = Transcript::new();
            match r {
                Ok(Value::Array(msgs)) => {
                    for m in &msgs {
                        tr.apply(m);
                    }
                }
                Ok(_) => {}
                Err(e) => tr.add_error(&e.message),
            }
            // eventos que chegaram durante a leitura ficam por cima do histórico
            if let Some(live) = app.data.transcripts.remove(&tid) {
                for i in live.items {
                    if !tr.items.iter().any(|x| x.key() == i.key()) {
                        tr.items.push(i);
                    }
                }
            }
            app.data.transcripts.insert(tid, tr);
            app.eph.scroll_to_bottom = true;
        });
    }

    pub fn refresh_thread_meta(&mut self, id: &str) {
        let (a, b) = (id.to_string(), id.to_string());
        self.call("thread.diff-stats", json!({ "id": id }), move |app, r| match r {
            Ok(v) => {
                app.data.diff_stats.insert(a, v);
            }
            Err(_) => {
                app.data.diff_stats.remove(&a);
            }
        });
        self.call("thread.branch-info", json!({ "id": id }), move |app, r| {
            if let Ok(v) = r {
                app.data.branch.insert(b, v);
            }
        });
    }

    /// Projeto da thread aberta (ou da tela do repositório).
    pub fn current_project(&self) -> Option<&lazyaicode_doc::Project> {
        match (self.selected(), &self.state.repo_project) {
            (Some(t), _) => self.data.project(&t.thread.project_id),
            (None, Some(p)) => self.data.project(p),
            _ => None,
        }
    }

    /// Pasta onde as abas git e o terminal trabalham.
    pub fn current_path(&self) -> Option<String> {
        match self.selected() {
            Some(t) => Some(t.thread.cwd.clone()),
            None => self.current_project().map(|p| p.path.clone()),
        }
    }

    pub fn send_message(&mut self, id: &str, text: String) {
        let images = self.data.attachments.remove(id).unwrap_or_default();
        let urls: Vec<String> = images.iter().map(|a| format!("data:{};base64,{}", a.mime, a.data)).collect();
        let tr = self.data.transcripts.entry(id.to_string()).or_default();
        tr.add_local_user(&text, urls);
        self.data.set_status(id, Status::Running);
        self.eph.scroll_to_bottom = true;
        let imgs: Vec<Value> = images.iter().map(|a| json!({ "mime": a.mime, "data": a.data })).collect();
        let tid = id.to_string();
        self.call("thread.send", json!({ "id": id, "text": text, "images": imgs }), move |app, r| {
            if let Err(e) = r {
                app.data.set_status(&tid, Status::Idle);
                app.data.transcripts.entry(tid).or_default().add_error(&e.message);
            }
        });
    }

    pub fn respond_permission(&mut self, thread: &str, request_id: &str, decision: Decision) {
        let item = self.data.transcripts.get(thread).and_then(|tr| {
            tr.items.iter().find_map(|i| match i {
                lazyaicode_doc::transcript::Item::Permission { request_id: r, input, tool_use_id, suggestions, .. } if r == request_id => {
                    Some((input.clone(), tool_use_id.clone(), suggestions.clone()))
                }
                _ => None,
            })
        });
        let Some((input, tool_use_id, suggestions)) = item else { return };
        if let Some(tr) = self.data.transcripts.get_mut(thread) {
            tr.decide(request_id, decision);
        }
        let allow = matches!(decision, Decision::Allow | Decision::Always);
        let mut p = json!({ "id": thread, "requestId": request_id, "allow": allow, "input": input, "toolUseId": tool_use_id });
        if decision == Decision::Always {
            p["updatedPermissions"] = json!(suggestions);
        }
        if decision == Decision::Deny {
            p["message"] = json!(t("permission.deniedByUser"));
        }
        self.fire("thread.respond-permission", p);
    }

    // ---------- o laço ----------

    /// Processa respostas, eventos e a inbox. Chamado no começo de cada quadro.
    pub fn pump(&mut self) {
        for inc in self.client.poll() {
            match inc {
                Incoming::Reply(ticket, r) => {
                    if let Some(cb) = self.pending.remove(&ticket) {
                        cb(self, r);
                    }
                }
                Incoming::Event(e) => self.on_event(&e.event, e.payload),
                Incoming::Connected => {
                    let was = self.data.connected;
                    self.data.connected = true;
                    self.data.connection_error = None;
                    if !was && self.data.booted {
                        self.refresh_all();
                    }
                }
                Incoming::Disconnected(why) => {
                    self.data.connected = false;
                    self.data.connection_error = Some(why);
                }
            }
        }
        for m in self.inbox.drain() {
            self.handle_ui_msg(m);
        }
        self.persist_prefs();
    }

    fn handle_ui_msg(&mut self, m: UiMsg) {
        let r = crate::commands::run_ui(self, &m.command, &m.params);
        if let Some(tx) = m.reply {
            match r {
                // a captura só sai depois que o quadro for desenhado
                Ok(Value::String(s)) if s == crate::commands::DEFERRED => self.screenshot_waiters.push(tx),
                other => {
                    let _ = tx.send(other.map_err(|e| ErrorBody { code: "failed".into(), message: e, data: None }));
                }
            }
        } else if let Err(e) = r {
            self.toast_err(&e);
        }
    }

    fn on_event(&mut self, name: &str, p: Value) {
        let tid = p["threadId"].as_str().unwrap_or("").to_string();
        match name {
            events::AGENT_EVENT => self.on_agent_message(&tid, &p["msg"]),
            events::THREAD_STATUS => {
                let st = Status::parse(p["status"].as_str().unwrap_or(""));
                self.data.set_status(&tid, st);
            }
            events::THREAD_UPDATED => {
                if let Ok(t) = serde_json::from_value::<ThreadView>(p) {
                    if t.thread.workspace_id == self.data.active_workspace {
                        self.data.upsert(t);
                    }
                }
            }
            events::AGENT_EXIT => {
                if let Some(err) = p["stderr"].as_str().filter(|s| !s.trim().is_empty()) {
                    let tail: String = err.trim().chars().rev().take(2000).collect::<Vec<_>>().into_iter().rev().collect();
                    self.data.transcripts.entry(tid).or_default().add_error(&tail);
                }
                self.refresh_threads();
            }
            events::TERM_DATA => {
                let id = p["termId"].as_str().unwrap_or("");
                if let Some(term) = self.terms.get_mut(id) {
                    match p["data"].as_str() {
                        Some(b64) => term.feed_b64(b64),
                        None => term.term.exited = true,
                    }
                }
            }
            events::STATE_CHANGED => match p["what"].as_str() {
                Some("workspaces") => self.refresh_workspaces(false),
                Some("threads") => self.refresh_threads(),
                Some("settings") => self.refresh_settings(),
                _ => {}
            },
            events::NOTICE => {
                let kind = if p["kind"] == "error" { ToastKind::Error } else { ToastKind::Info };
                self.toast(p["text"].as_str().unwrap_or("").to_string(), kind);
            }
            events::REPO_CHANGED => crate::views::git::on_repo_changed(self, &p),
            _ => crate::views::git::on_git_event(self, name, &p),
        }
    }

    fn on_agent_message(&mut self, tid: &str, m: &Value) {
        let ty = m["type"].as_str().unwrap_or("");
        match ty {
            "result" => {
                if let Some(c) = lazyaicode_doc::usage::from_result(m) {
                    self.data.context.insert(tid.to_string(), c);
                }
                self.on_finished(tid, m);
            }
            "control_request" if m["request"]["subtype"] == "can_use_tool" => {
                let tool = m["request"]["tool_name"].as_str().unwrap_or("").to_string();
                self.attention(tid, &format!("⏸ {}", self.thread_title(tid)), &tf("app.needsPermission", &[("tool", &tool)]));
            }
            "system" if m["subtype"] == "init" => {
                if m["mcp_servers"].is_array() {
                    self.data.thread_mcp.insert(tid.to_string(), m["mcp_servers"].clone());
                }
                if m["acp"].is_object() {
                    self.data.acp_session.insert(tid.to_string(), m["acp"].clone());
                }
            }
            "lazy_commands" => {
                let s = self.data.acp_session.entry(tid.to_string()).or_insert_with(|| json!({}));
                s["commands"] = m["commands"].clone();
            }
            "lazy_config" => {
                if let Some(s) = self.data.acp_session.get_mut(tid) {
                    if let Some(mode) = m["currentModeId"].as_str() {
                        s["currentMode"] = json!(mode);
                    }
                    for c in m["configOptions"].as_array().into_iter().flatten() {
                        match c["category"].as_str() {
                            Some("mode") => s["currentMode"] = c["currentValue"].clone(),
                            Some("model") => s["currentModel"] = c["currentValue"].clone(),
                            _ => {}
                        }
                    }
                }
            }
            "lazy_usage" => {
                if let (Some(used), Some(size)) = (m["used"].as_u64(), m["size"].as_u64().filter(|s| *s > 0)) {
                    let model = m["model"].as_str().unwrap_or("").to_string();
                    self.data.context.insert(tid.to_string(), lazyaicode_doc::usage::ContextUse { used, window: size, model });
                }
            }
            _ => {}
        }
        // a transcrição só cresce ao vivo se já foi carregada (senão vem toda do histórico)
        let loaded = self.data.transcripts.contains_key(tid) || self.data.loading_transcript.contains(tid);
        if loaded || self.state.selected.as_deref() == Some(tid) {
            self.data.transcripts.entry(tid.to_string()).or_default().apply(m);
            if self.state.selected.as_deref() == Some(tid) {
                self.eph.scroll_to_bottom |= ty == "assistant" || ty == "stream_event" || ty == "result";
            }
        }
    }

    fn thread_title(&self, id: &str) -> String {
        self.data.thread(id).map(|t| t.thread.title.clone()).unwrap_or_else(|| t("app.thread"))
    }

    /// Avisa quando um turno termina numa thread que você não está olhando.
    fn on_finished(&mut self, tid: &str, result: &Value) {
        self.refresh_thread_meta(tid);
        crate::views::git::on_turn_finished(self, tid);
        if self.state.selected.as_deref() == Some(tid) {
            let base = self.state.review_base.id();
            crate::views::review::load_changes(self, tid, base);
        }
        let last_text = self.data.transcripts.get(tid).and_then(|tr| {
            tr.items.iter().rev().find_map(|i| match i {
                lazyaicode_doc::transcript::Item::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
        });
        let body = if result["is_error"].as_bool().unwrap_or(false) {
            match result["result"].as_str() {
                Some(r) => tf("app.finishedWithErrorDetail", &[("detail", &r.chars().take(120).collect::<String>())]),
                None => t("app.finishedWithError"),
            }
        } else {
            last_text.map(|s| s.replace(['#', '*', '`', '_', '>'], "").trim().chars().take(160).collect()).unwrap_or_else(|| t("app.done"))
        };
        let title = format!("✓ {}", self.thread_title(tid));
        self.attention(tid, &title, &body);
    }

    fn attention(&mut self, tid: &str, title: &str, body: &str) {
        let looking = self.focused && self.state.selected.as_deref() == Some(tid);
        let notify = self.data.settings.as_ref().is_some_and(|s| s.notify_on_finish);
        if !looking && notify {
            self.host.notify(title, body);
        }
    }

    // ---------- desenho ----------

    fn apply_theme(&mut self, ctx: &egui::Context) {
        let a = &self.state.appearance;
        let key = (
            a.theme.clone(),
            self.system_dark,
            a.scale,
            (a.text_size * 10.0) as u32,
            (a.code_size * 10.0) as u32,
            a.density as u32 * 10 + a.content_width as u32,
        );
        if self.applied.as_ref() == Some(&key) {
            return;
        }
        let def = theme::resolve(&a.theme, self.system_dark);
        self.tokens = Tokens::new(def, a);
        self.tokens.apply(ctx);
        ctx.set_zoom_factor((a.scale.clamp(50, 250) as f32) / 100.0);
        let fonts_missing = !ctx.fonts(|f| f.definitions().font_data.contains_key("phosphor"));
        if fonts_missing {
            ctx.set_fonts(theme::fonts());
        }
        self.applied = Some(key);
    }

    /// Desenha o app inteiro dentro de `ui` (a janela do eframe, ou um
    /// harness de teste). Chamar [`LazyApp::pump`] antes.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.now = ctx.input(|i| i.time);
        self.focused = ctx.input(|i| i.focused);
        if let Some(theme) = ctx.input(|i| i.raw.system_theme) {
            self.system_dark = theme == egui::Theme::Dark;
        }
        self.apply_theme(&ctx);
        crate::commands::handle_shortcuts(self, &ctx);
        crate::views::titlebar::show(self, ui);
        if self.state.picking_workspaces || self.data.active_workspace.is_none() {
            crate::views::workspace_picker::show(self, ui);
        } else {
            if self.state.sidebar_open {
                crate::views::sidebar::show(self, ui);
            }
            crate::views::main::show(self, ui);
        }
        crate::views::palette::show(self, &ctx);
        crate::views::settings::show(self, &ctx);
        crate::views::workspace_picker::new_workspace_modal(self, &ctx);
        crate::views::dialogs::show(self, &ctx);
        crate::views::toasts::show(self, &ctx);
        if let Some(text) = self.eph.copy_request.take() {
            ctx.copy_text(text);
        }
        for cmd in std::mem::take(&mut self.eph.viewport) {
            crate::views::titlebar::apply_viewport(&ctx, cmd);
        }
        let busy = self.data.threads.iter().any(|t| t.status == Status::Running) || !self.pending.is_empty();
        if busy || !self.toasts.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    /// Respostas a `ui.screenshot` que esperavam este quadro, e se a captura
    /// deve sair do renderizador em CPU.
    pub fn take_screenshot_waiters(&mut self) -> (Vec<std::sync::mpsc::Sender<Result<Value, ErrorBody>>>, bool) {
        (std::mem::take(&mut self.screenshot_waiters), std::mem::take(&mut self.screenshot_cpu))
    }

    /// Força reaplicar tema e fontes no próximo quadro (um `Context` novo,
    /// como o da captura em CPU, começa sem eles).
    pub fn invalidate_theme(&mut self) {
        self.applied = None;
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }
}

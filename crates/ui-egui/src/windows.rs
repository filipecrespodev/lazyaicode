//! Uma janela por workspace (desktop). A janela principal é dona das
//! outras e as desenha como viewports imediatos do egui; cada uma é um
//! [`LazyApp`] inteiro, com o próprio cliente no mesmo motor. Pedidos de
//! abrir um workspace (o "+", a paleta, a atividade) sobem para a principal,
//! que foca a janela onde ele já está ou abre uma nova.
//!
//! Sem fábrica de janelas (a web, as capturas, os testes) o app segue com os
//! workspaces em abas na barra de título.

use egui::{ViewportCommand, ViewportId};
use serde_json::json;

use crate::app::LazyApp;
use crate::client::Client;
use crate::host::{Host, Inbox};

/// O que a casca entrega para cada janela nova: um cliente e uma plataforma
/// só dela (as respostas dos diálogos voltam para a inbox dela) e como a
/// janela nasce (ícone, tamanho, decorações).
pub struct NewWindow {
    pub client: Box<dyn Client>,
    pub host: Box<dyn Host>,
    pub inbox: Inbox,
    pub builder: egui::ViewportBuilder,
}

pub type WindowFactory = Box<dyn Fn() -> NewWindow>;

/// Abrir um workspace (e, se dado, uma thread dele) onde ele estiver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reveal {
    pub workspace: String,
    pub thread: Option<String>,
}

pub struct Child {
    pub vid: ViewportId,
    pub app: LazyApp,
    builder: egui::ViewportBuilder,
    /// sem janelas de verdade (capturas em CPU, testes) a filha desenha num
    /// contexto próprio, fora da tela, em vez de dentro da principal
    offscreen: Option<egui::Context>,
}

/// Aparência e atalhos valem para todas as janelas: o que uma mudou, as
/// outras copiam.
type Shared = (crate::theme::Appearance, std::collections::HashMap<String, String>);

fn shared_of(app: &LazyApp) -> Shared {
    (app.state.appearance.clone(), app.state.shortcuts.clone())
}

/// Desenha as janelas filhas e atende os pedidos de abrir workspace. Só a
/// principal tem filhas; chamado no fim do quadro dela.
pub fn show(app: &mut LazyApp, ctx: &egui::Context) {
    if app.is_child {
        return;
    }
    // sair (de qualquer janela) fecha todas
    if app.eph.quitting || app.windows.iter().any(|c| c.app.eph.quitting) {
        for c in std::mem::take(&mut app.windows) {
            close_child(c);
        }
        ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
        return;
    }
    // fechar a principal com outras abertas fecha só ela: a primeira filha
    // passa a morar nesta janela
    if !app.windows.is_empty() && ctx.input(|i| i.viewport().close_requested()) {
        ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        promote_first_child(app);
    }
    let mut requests = std::mem::take(&mut app.eph.window_requests);
    let mut closed = vec![];
    let all: Vec<String> =
        app.data.active_workspace.iter().cloned().chain(app.windows.iter().filter_map(|c| c.app.data.active_workspace.clone())).collect();
    for (i, child) in app.windows.iter_mut().enumerate() {
        let mine = child.app.data.active_workspace.clone();
        child.app.eph.other_windows = all.iter().filter(|w| Some(*w) != mine.as_ref()).cloned().collect();
        let (vid, builder) = (child.vid, child.builder.clone());
        let capp = &mut child.app;
        if ctx.embed_viewports() {
            let off = child.offscreen.get_or_insert_with(|| {
                let c = egui::Context::default();
                crate::install(&c);
                c
            });
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
                time: Some(ctx.input(|i| i.time)),
                ..Default::default()
            };
            let mut out = off.run_ui(input, |ui| {
                capp.pump();
                egui::CentralPanel::no_frame().show(ui, |ui| capp.ui(ui));
            });
            out.textures_delta.clear();
        } else {
            ctx.show_viewport_immediate(vid, builder, |ui, _| {
                capp.pump();
                capp.ui(ui);
                if ui.ctx().input(|inp| inp.viewport().close_requested()) {
                    closed.push(i);
                }
            });
        }
        requests.append(&mut child.app.eph.window_requests);
    }
    app.eph.other_windows = all.iter().filter(|w| Some(*w) != app.data.active_workspace.as_ref()).cloned().collect();
    for i in closed.into_iter().rev() {
        if i < app.windows.len() {
            let c = app.windows.remove(i);
            close_child(c);
        }
    }
    sync_shared(app);
    for r in requests {
        reveal(app, ctx, r);
    }
}

fn reveal(app: &mut LazyApp, ctx: &egui::Context, r: Reveal) {
    if app.data.active_workspace.as_deref() == Some(r.workspace.as_str()) {
        if let Some(t) = r.thread {
            app.reveal_thread(&t);
        }
        ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Focus);
        return;
    }
    if let Some(c) = app.windows.iter_mut().find(|c| c.app.data.active_workspace.as_deref() == Some(r.workspace.as_str())) {
        if let Some(t) = r.thread {
            c.app.reveal_thread(&t);
        }
        ctx.send_viewport_cmd_to(c.vid, ViewportCommand::Focus);
        return;
    }
    let Some(factory) = &app.factory else {
        // sem janelas (não devia chegar aqui): vira aba
        app.switch_workspace(&r.workspace);
        if let Some(t) = r.thread {
            app.reveal_thread(&t);
        }
        return;
    };
    let nw = factory();
    app.window_seq += 1;
    let vid = ViewportId::from_hash_of(("lazyaicode-window", app.window_seq));
    let mut child = LazyApp::new_window(nw.client, nw.host, nw.inbox, r);
    child.state.appearance = app.state.appearance.clone();
    child.state.shortcuts = app.state.shortcuts.clone();
    app.windows.push(Child { vid, app: child, builder: nw.builder, offscreen: None });
    ctx.request_repaint();
}

/// A janela some: os terminais dela morrem junto (as threads seguem no motor).
fn close_child(mut c: Child) {
    let terms: Vec<String> = c.app.terms.keys().cloned().collect();
    for id in terms {
        c.app.fire("terminal.kill", json!({ "termId": id }));
    }
}

fn promote_first_child(app: &mut LazyApp) {
    let mut kids = std::mem::take(&mut app.windows);
    if kids.is_empty() {
        return;
    }
    let mut first = kids.remove(0);
    // a inbox e a plataforma da principal (onde o canal de controle entrega) ficam
    std::mem::swap(&mut app.inbox, &mut first.app.inbox);
    std::mem::swap(&mut app.host, &mut first.app.host);
    first.app.is_child = false;
    first.app.factory = app.factory.take();
    first.app.window_seq = app.window_seq;
    first.app.windows = kids;
    first.app.eph.last_title = None;
    let old = std::mem::replace(app, first.app);
    close_child(Child { vid: first.vid, app: old, builder: first.builder, offscreen: None });
}

fn sync_shared(app: &mut LazyApp) {
    if app.windows.is_empty() {
        app.eph.shared = None;
        return;
    }
    let base = app.eph.shared.clone().unwrap_or_else(|| shared_of(app));
    let changed = std::iter::once(&*app).chain(app.windows.iter().map(|c| &c.app)).map(shared_of).find(|s| *s != base);
    let now = changed.unwrap_or(base);
    (app.state.appearance, app.state.shortcuts) = now.clone();
    for c in &mut app.windows {
        (c.app.state.appearance, c.app.state.shortcuts) = now.clone();
    }
    app.eph.shared = Some(now);
}

/// Workspaces abertos em todas as janelas, na ordem: esta, depois as outras.
pub fn open_everywhere(app: &LazyApp) -> Vec<String> {
    let mut v = app.state.open_workspaces.clone();
    for w in &app.eph.other_windows {
        if !v.contains(w) {
            v.push(w.clone());
        }
    }
    v
}

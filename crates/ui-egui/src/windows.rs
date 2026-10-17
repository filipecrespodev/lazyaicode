//! Uma janela por workspace (desktop). Cada janela é um [`LazyApp`] inteiro,
//! com o próprio cliente no mesmo motor. As filhas são viewports *deferred*
//! do egui penduradas na principal: cada uma desenha com os próprios
//! quadros. (Como viewports imediatos, desenhados dentro do quadro da
//! principal, todas congelavam quando a principal ficava minimizada ou
//! coberta — no Wayland, janela escondida não recebe quadros.)
//!
//! As filhas moram num registro da thread da interface (o egui pede
//! callbacks `Send`; o callback leva só o id do viewport e acha a janela
//! aqui). Pedidos de abrir um workspace (o "+", a paleta, a atividade) vão
//! para o registro e qualquer janela os atende no próprio quadro: foca a
//! janela onde o workspace já está ou abre uma nova — mesmo com a principal
//! escondida.
//!
//! Sem fábrica de janelas (a web, as capturas, os testes de tela) o app
//! segue com os workspaces em abas na barra de título.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use egui::{ViewportCommand, ViewportId};
use serde_json::{json, Value};

use crate::app::LazyApp;
use crate::client::Client;
use crate::host::{Host, Inbox};

/// O que a casca entrega para cada janela nova: um cliente e uma plataforma
/// só dela (as respostas dos diálogos voltam para a inbox dela; o cliente
/// acorda o viewport dela) e como a janela nasce (ícone, tamanho, decorações).
pub struct NewWindow {
    pub client: Box<dyn Client>,
    pub host: Box<dyn Host>,
    pub inbox: Inbox,
    pub builder: egui::ViewportBuilder,
}

pub type WindowFactory = Box<dyn Fn(ViewportId) -> NewWindow>;

/// Abrir um workspace (e, se dado, uma thread dele) onde ele estiver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reveal {
    pub workspace: String,
    pub thread: Option<String>,
}

struct Child {
    vid: ViewportId,
    app: Rc<RefCell<LazyApp>>,
    builder: egui::ViewportBuilder,
    /// a principal já a mostrou (antes disso, quem a criou a mantém viva)
    adopted: bool,
    /// a pessoa fechou a janela: some já; sai do registro no quadro da principal
    closing: bool,
    /// sem janelas de verdade (capturas em CPU, testes) a filha desenha num
    /// contexto próprio, fora da tela
    offscreen: Option<egui::Context>,
}

/// Aparência e atalhos valem para todas as janelas: o que uma mudou, as
/// outras copiam.
type Shared = (crate::theme::Appearance, HashMap<String, String>);

#[derive(Default)]
struct Registry {
    factory: Option<WindowFactory>,
    children: Vec<Child>,
    requests: Vec<Reveal>,
    /// threads a abrir em cada janela no próximo quadro dela
    reveals: HashMap<ViewportId, Vec<String>>,
    root_workspace: Option<String>,
    seq: u64,
    quitting: bool,
    shared: Option<Shared>,
}

thread_local! {
    static REG: RefCell<Registry> = RefCell::new(Registry::default());
}

fn with<R>(f: impl FnOnce(&mut Registry) -> R) -> Option<R> {
    REG.with(|r| r.try_borrow_mut().ok().map(|mut r| f(&mut r)))
}

fn shared_of(app: &LazyApp) -> Shared {
    (app.state.appearance.clone(), app.state.shortcuts.clone())
}

pub(crate) fn set_factory(f: WindowFactory) {
    let _ = with(|r| {
        *r = Registry::default();
        r.factory = Some(f);
    });
}

/// Workspaces abertos nas janelas, `(viewport, workspace)`; a principal primeiro.
fn open_windows() -> Vec<(ViewportId, String)> {
    with(|r| {
        let mut v: Vec<(ViewportId, String)> = r.root_workspace.clone().map(|w| (ViewportId::ROOT, w)).into_iter().collect();
        for c in r.children.iter().filter(|c| !c.closing) {
            if let Some(w) = c.app.try_borrow().ok().and_then(|a| a.data.active_workspace.clone()) {
                v.push((c.vid, w));
            }
        }
        v
    })
    .unwrap_or_default()
}

/// Começo do quadro de uma janela: o que ela sabe das outras e as threads
/// que pediram para ela abrir.
fn begin(app: &mut LazyApp, vid: ViewportId) {
    if vid == ViewportId::ROOT {
        let ws = app.data.active_workspace.clone();
        let _ = with(|r| r.root_workspace = ws);
    }
    let mine = app.data.active_workspace.clone();
    app.eph.other_windows = open_windows().into_iter().map(|(_, w)| w).filter(|w| Some(w) != mine.as_ref()).collect();
    for t in with(|r| r.reveals.remove(&vid).unwrap_or_default()).unwrap_or_default() {
        app.reveal_thread(&t);
    }
}

/// Fim do quadro de uma janela: os pedidos dela vão para o registro.
fn end(app: &mut LazyApp) {
    let reqs = std::mem::take(&mut app.eph.window_requests);
    let quitting = app.eph.quitting;
    let _ = with(|r| {
        r.requests.extend(reqs);
        r.quitting |= quitting;
    });
}

/// Atende os pedidos de abrir workspace. `root` é a principal, quando é ela
/// quem está no quadro (senão a thread espera o próximo quadro dela).
fn dispatch(ctx: &egui::Context, mut root: Option<&mut LazyApp>, look: &Shared) {
    let reqs = with(|r| std::mem::take(&mut r.requests)).unwrap_or_default();
    for req in reqs {
        let open = open_windows();
        if let Some((vid, _)) = open.iter().find(|(_, w)| *w == req.workspace) {
            if let Some(t) = req.thread {
                match (*vid == ViewportId::ROOT, root.as_deref_mut()) {
                    (true, Some(app)) => app.reveal_thread(&t),
                    _ => {
                        let _ = with(|r| r.reveals.entry(*vid).or_default().push(t));
                    }
                }
            }
            ctx.send_viewport_cmd_to(*vid, ViewportCommand::Focus);
            ctx.request_repaint_of(*vid);
            continue;
        }
        let made = with(|r| {
            let factory = r.factory.as_ref()?;
            r.seq += 1;
            let vid = ViewportId::from_hash_of(("lazyaicode-window", r.seq));
            Some((vid, factory(vid)))
        })
        .flatten();
        let Some((vid, nw)) = made else {
            // sem janelas: vira aba (não devia chegar aqui)
            if let Some(app) = root.as_deref_mut() {
                app.switch_workspace(&req.workspace);
                if let Some(t) = req.thread {
                    app.reveal_thread(&t);
                }
            }
            continue;
        };
        let mut app = LazyApp::new_window(nw.client, nw.host, nw.inbox, req);
        (app.state.appearance, app.state.shortcuts) = look.clone();
        let child = Child { vid, app: Rc::new(RefCell::new(app)), builder: nw.builder, adopted: false, closing: false, offscreen: None };
        let _ = with(|r| r.children.push(child));
        ctx.request_repaint();
    }
}

/// Mostra as filhas. A principal mostra todas (e as adota); uma filha só
/// mantém vivas as que ela mesma criou enquanto a principal não desenha.
fn show_children(ctx: &egui::Context, from_root: bool) {
    if ctx.embed_viewports() {
        // sem janelas de verdade: cada filha num contexto próprio, fora da tela
        if from_root {
            offscreen_passes(ctx.input(|i| i.time));
        }
        return;
    }
    let list: Vec<(ViewportId, egui::ViewportBuilder)> = with(|r| {
        r.children
            .iter_mut()
            .filter(|c| !c.closing && (from_root || !c.adopted))
            .map(|c| {
                c.adopted |= from_root;
                (c.vid, c.builder.clone())
            })
            .collect()
    })
    .unwrap_or_default();
    for (vid, builder) in list {
        ctx.show_viewport_deferred(vid, builder, move |ui, _| child_pass(vid, ui));
    }
}

/// O quadro de uma janela filha (o egui chama quando ela precisa desenhar).
fn child_pass(vid: ViewportId, ui: &mut egui::Ui) {
    let Some(app) = with(|r| r.children.iter().find(|c| c.vid == vid).map(|c| c.app.clone())).flatten() else { return };
    let ctx = ui.ctx().clone();
    let look = {
        let Ok(mut a) = app.try_borrow_mut() else { return };
        begin(&mut a, vid);
        a.pump();
        a.ui(ui);
        if ctx.input(|i| i.viewport().close_requested()) {
            // some na hora; a principal tira do registro no quadro dela
            let _ = with(|r| r.children.iter_mut().filter(|c| c.vid == vid).for_each(|c| c.closing = true));
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            ctx.request_repaint_of(ViewportId::ROOT);
        }
        end(&mut a);
        shared_of(&a)
    };
    if with(|r| r.quitting).unwrap_or(false) {
        ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
        ctx.request_repaint_of(ViewportId::ROOT);
        return;
    }
    dispatch(&ctx, None, &look);
    show_children(&ctx, false);
}

/// O quadro da principal: sair, fechar a principal com outras abertas,
/// pedidos, filhas e aparência comum. Chamado no fim do quadro dela.
pub fn root_pass(app: &mut LazyApp, ctx: &egui::Context) {
    if app.is_child || !app.multi_window {
        return;
    }
    begin(app, ViewportId::ROOT);
    end(app);
    // sair (de qualquer janela) fecha todas
    if with(|r| r.quitting).unwrap_or(false) {
        for c in with(|r| std::mem::take(&mut r.children)).unwrap_or_default() {
            close_child(c);
        }
        ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
        return;
    }
    // fechar a principal com outras abertas fecha só ela: a primeira filha
    // passa a morar nesta janela
    if ctx.input(|i| i.viewport().close_requested()) && with(|r| r.children.iter().any(|c| !c.closing)).unwrap_or(false) {
        ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        promote_first_child(app);
    }
    // as fechadas saem
    let gone = with(|r| {
        let (gone, keep): (Vec<Child>, Vec<Child>) = std::mem::take(&mut r.children).into_iter().partition(|c| c.closing);
        r.children = keep;
        gone
    })
    .unwrap_or_default();
    for c in gone {
        close_child(c);
    }
    let look = sync_shared(app);
    dispatch(ctx, Some(app), &look);
    show_children(ctx, true);
}

/// Sem janelas de verdade (testes, capturas): um quadro de cada filha num
/// contexto próprio. Os testes chamam direto para simular a principal
/// escondida (sem quadros dela).
pub fn offscreen_passes(time: f64) {
    let kids: Vec<(ViewportId, Rc<RefCell<LazyApp>>, egui::Context)> = with(|r| {
        r.children
            .iter_mut()
            .filter(|c| !c.closing)
            .map(|c| {
                let off = c.offscreen.get_or_insert_with(|| {
                    let ctx = egui::Context::default();
                    crate::install(&ctx);
                    ctx
                });
                (c.vid, c.app.clone(), off.clone())
            })
            .collect()
    })
    .unwrap_or_default();
    for (vid, app, off) in kids {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
            time: Some(time),
            ..Default::default()
        };
        let mut look = None;
        let mut out = off.run_ui(input, |ui| {
            let Ok(mut a) = app.try_borrow_mut() else { return };
            begin(&mut a, vid);
            a.pump();
            egui::CentralPanel::no_frame().show(ui, |ui| a.ui(ui));
            end(&mut a);
            look = Some(shared_of(&a));
        });
        out.textures_delta.clear();
        if let Some(look) = look {
            dispatch(&off, None, &look);
        }
    }
}

/// A janela some: os terminais dela morrem junto (as threads seguem no motor).
fn close_child(c: Child) {
    let Ok(mut app) = c.app.try_borrow_mut() else { return };
    let terms: Vec<String> = app.terms.keys().cloned().collect();
    for id in terms {
        app.fire("terminal.kill", json!({ "termId": id }));
    }
}

fn promote_first_child(app: &mut LazyApp) {
    let first = with(|r| {
        let i = r.children.iter().position(|c| !c.closing)?;
        Some(r.children.remove(i))
    })
    .flatten();
    let Some(first) = first else { return };
    {
        let Ok(mut child) = first.app.try_borrow_mut() else { return };
        // a inbox e a plataforma da principal (onde o canal de controle entrega)
        // ficam; o resto troca de lugar
        std::mem::swap(&mut app.inbox, &mut child.inbox);
        std::mem::swap(&mut app.host, &mut child.host);
        std::mem::swap(app, &mut *child);
        app.is_child = false;
        child.is_child = true;
        app.eph.last_title = None;
    }
    let ws = app.data.active_workspace.clone();
    let _ = with(|r| r.root_workspace = ws);
    // a filha (agora com o workspace que estava na principal) fecha
    close_child(first);
}

/// Aparência e atalhos: o que uma janela mudou vale para todas.
fn sync_shared(app: &mut LazyApp) -> Shared {
    let apps: Vec<Rc<RefCell<LazyApp>>> = with(|r| r.children.iter().map(|c| c.app.clone()).collect()).unwrap_or_default();
    let mine = shared_of(app);
    let base = with(|r| r.shared.clone()).flatten().unwrap_or_else(|| mine.clone());
    let now = if mine != base {
        mine
    } else {
        apps.iter().filter_map(|a| a.try_borrow().ok().map(|a| shared_of(&a))).find(|s| *s != base).unwrap_or(base)
    };
    (app.state.appearance, app.state.shortcuts) = now.clone();
    for a in &apps {
        if let Ok(mut a) = a.try_borrow_mut() {
            (a.state.appearance, a.state.shortcuts) = now.clone();
        }
    }
    let _ = with(|r| r.shared = Some(now.clone()));
    now
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

/// Há outras janelas abertas.
pub(crate) fn has_children() -> bool {
    with(|r| r.children.iter().any(|c| !c.closing)).unwrap_or(false)
}

/// As outras janelas, para o `ui.inspect` da principal.
pub fn children_inspect() -> Vec<Value> {
    with(|r| {
        r.children
            .iter()
            .filter(|c| !c.closing)
            .map(|c| match c.app.try_borrow() {
                Ok(a) => json!({
                    "workspace": a.data.active_workspace,
                    "selected": a.state.selected,
                    "threads": a.data.threads.len(),
                }),
                Err(_) => json!({ "busy": true }),
            })
            .collect()
    })
    .unwrap_or_default()
}

/// Para os testes: a inbox de cada filha, na ordem em que abriram.
pub fn children_inboxes() -> Vec<Inbox> {
    with(|r| r.children.iter().filter(|c| !c.closing).filter_map(|c| c.app.try_borrow().ok().map(|a| a.inbox())).collect())
        .unwrap_or_default()
}

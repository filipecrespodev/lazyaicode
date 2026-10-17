//! O que pede atenção em todos os workspaces: na barra de título, quantas
//! sessões esperam você, quantas rodam e quantas terminaram sem ser lidas;
//! clicando, a lista — e cada item abre a thread na janela do workspace dela.

use egui::{RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular as icon;
use lazyaicode_doc::{Status, ThreadView};
use lazyaicode_i18n::t;
use serde_json::json;

use crate::app::LazyApp;
use crate::theme::Tokens;
use crate::views::widgets;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub awaiting: usize,
    pub running: usize,
    pub error: usize,
    pub unread: usize,
}

impl Counts {
    pub fn total(self) -> usize {
        self.awaiting + self.running + self.error + self.unread
    }
}

/// O estado que a lista mostra para a thread (o que pede você vem antes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Awaiting,
    Error,
    Unread,
    Running,
}

pub fn kind_of(t: &ThreadView) -> Option<Kind> {
    match t.status {
        Status::Awaiting => Some(Kind::Awaiting),
        Status::Error => Some(Kind::Error),
        Status::Running => Some(Kind::Running),
        Status::Idle if t.thread.unread => Some(Kind::Unread),
        Status::Idle => None,
    }
}

pub fn counts<'a>(threads: impl Iterator<Item = &'a ThreadView>) -> Counts {
    let mut c = Counts::default();
    for t in threads {
        match kind_of(t) {
            Some(Kind::Awaiting) => c.awaiting += 1,
            Some(Kind::Running) => c.running += 1,
            Some(Kind::Error) => c.error += 1,
            Some(Kind::Unread) => c.unread += 1,
            None => {}
        }
    }
    c
}

fn kind_style(tk: &Tokens, k: Kind) -> (&'static str, egui::Color32, &'static str) {
    match k {
        Kind::Awaiting => (icon::PAUSE_CIRCLE, tk.p.amber, "activity.awaiting"),
        Kind::Error => (icon::WARNING_CIRCLE, tk.p.red, "activity.error"),
        Kind::Unread => (icon::CHECK_CIRCLE, tk.p.accent, "activity.unread"),
        Kind::Running => (icon::CIRCLE_NOTCH, tk.p.blue, "activity.running"),
    }
}

/// As threads da lista, na ordem em que aparecem: esperando você, com
/// erro, terminadas sem ler, rodando; dentro de cada uma, a mais recente
/// primeiro.
pub fn ordered(app: &LazyApp) -> Vec<(Kind, ThreadView)> {
    let mut v: Vec<(Kind, ThreadView)> = app.data.activity.iter().filter_map(|t| kind_of(t).map(|k| (k, t.clone()))).collect();
    v.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.thread.updated_at.cmp(&a.1.thread.updated_at)));
    v
}

/// Abre a próxima thread que pede atenção (depois da aberta).
pub fn open_next(app: &mut LazyApp) -> Result<serde_json::Value, String> {
    let list: Vec<ThreadView> = ordered(app).into_iter().filter(|(k, _)| *k != Kind::Running).map(|(_, t)| t).collect();
    let cur = app.state.selected.as_ref().and_then(|s| list.iter().position(|t| &t.thread.id == s));
    let next = match cur {
        Some(i) => list.get(i + 1).or_else(|| list.first()),
        None => list.first(),
    };
    let Some(t) = next.cloned() else { return Err(t("activity.empty")) };
    open(app, &t);
    Ok(serde_json::Value::Null)
}

fn open(app: &mut LazyApp, t: &ThreadView) {
    let Some(ws) = t.thread.workspace_id.clone() else { return };
    app.state.activity_open = false;
    app.open_workspace(&ws, Some(t.thread.id.clone()));
}

/// O botão da barra de título: os contadores, ou um sino apagado.
pub fn indicator(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    let c = counts(app.data.activity.iter());
    let segs: Vec<(Kind, usize)> =
        [(Kind::Awaiting, c.awaiting), (Kind::Error, c.error), (Kind::Unread, c.unread), (Kind::Running, c.running)]
            .into_iter()
            .filter(|(_, n)| *n > 0)
            .collect();
    let font = egui::FontId::proportional(tk.text_size - 1.0);
    let icon_font = egui::FontId::proportional(13.0);
    let mut width = 12.0;
    for (_, n) in &segs {
        width += 14.0 + 3.0 + ui.painter().layout_no_wrap(n.to_string(), font.clone(), tk.p.text).size().x + 8.0;
    }
    if segs.is_empty() {
        width = 28.0;
    }
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::click());
    let p = ui.painter_at(rect);
    let open = app.state.activity_open;
    if resp.hovered() || open {
        p.rect_filled(rect, egui::CornerRadius::same(6), tk.p.hover);
    }
    if segs.is_empty() {
        p.text(rect.center(), egui::Align2::CENTER_CENTER, icon::BELL, egui::FontId::proportional(15.0), tk.p.muted);
    } else {
        p.rect_stroke(rect.shrink(1.0), egui::CornerRadius::same(6), Stroke::new(1.0, tk.p.border), egui::StrokeKind::Inside);
        let mut x = rect.left() + 8.0;
        for (k, n) in &segs {
            let (ic, color, _) = kind_style(&tk, *k);
            let at = egui::pos2(x + 7.0, rect.center().y);
            if *k == Kind::Running {
                spinner(&p, at, color, app.now);
            } else {
                p.text(at, egui::Align2::CENTER_CENTER, ic, icon_font.clone(), color);
            }
            x += 17.0;
            let g = p.layout_no_wrap(n.to_string(), font.clone(), tk.p.text);
            let w = g.size().x;
            p.galley(egui::pos2(x, rect.center().y - g.size().y / 2.0), g, tk.p.text);
            x += w + 8.0;
        }
        if c.running > 0 {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(120));
        }
    }
    let tip = lazyaicode_i18n::tf("activity.tooltip", &[("key", &widgets::pretty_shortcut("Ctrl+Shift+D"))]);
    let resp = resp.on_hover_text(tip);
    if resp.clicked() {
        app.state.activity_open = !open;
        if !open {
            app.refresh_activity();
        }
    }
    let mut keep = app.state.activity_open;
    egui::Popup::from_response(&resp)
        .open_bool(&mut keep)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .align(egui::RectAlign::BOTTOM_END)
        .width(380.0)
        .show(|ui| list(app, ui));
    // o clique que abriu não fecha; o de fora (ou Esc) sim
    if !keep && !resp.clicked() {
        app.state.activity_open = false;
    }
}

/// O anel girando do "rodando", como na barra lateral.
fn spinner(p: &egui::Painter, c: egui::Pos2, color: egui::Color32, now: f64) {
    let a = (now * 4.0) as f32;
    p.circle_stroke(c, 5.0, Stroke::new(1.5, color.gamma_multiply(0.3)));
    let pts: Vec<egui::Pos2> = (0..12).map(|i| c + Vec2::angled(a + i as f32 * 0.15) * 5.0).collect();
    p.add(egui::Shape::line(pts, Stroke::new(1.5, color)));
}

fn list(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    ui.set_width(380.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(t("activity.title")).strong().color(tk.p.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let unread: Vec<String> =
                app.data.activity.iter().filter(|t| kind_of(t) == Some(Kind::Unread)).map(|t| t.thread.id.clone()).collect();
            if !unread.is_empty() && ui.small_button(t("activity.markAllRead")).clicked() {
                for id in unread {
                    app.fire("thread.update", json!({ "id": id, "unread": false }));
                }
            }
        });
    });
    ui.separator();
    let items = ordered(app);
    if items.is_empty() {
        ui.add_space(6.0);
        ui.label(widgets::muted(&tk, t("activity.empty")));
        ui.add_space(6.0);
        return;
    }
    let now = app.host.now_ms();
    let mine = app.data.active_workspace.clone();
    let mut clicked = None;
    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
        let mut last = None;
        for (k, th) in &items {
            if last != Some(*k) {
                let (_, color, key) = kind_style(&tk, *k);
                let n = items.iter().filter(|(x, _)| x == k).count();
                ui.add_space(4.0);
                ui.label(RichText::new(format!("{}  ·  {n}", t(key))).size(tk.text_size - 1.5).color(color).strong());
                last = Some(*k);
            }
            let ws_name = th.thread.workspace_id.as_deref().and_then(|w| app.data.workspace(w)).map(|w| w.name.clone()).unwrap_or_default();
            let here = th.thread.workspace_id == mine;
            if row(app, ui, *k, th, &ws_name, here, now).clicked() {
                clicked = Some(th.clone());
            }
        }
    });
    if let Some(th) = clicked {
        open(app, &th);
    }
}

fn row(app: &LazyApp, ui: &mut egui::Ui, k: Kind, th: &ThreadView, ws_name: &str, here: bool, now: i64) -> egui::Response {
    let tk = app.tokens;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
    let p = ui.painter_at(rect);
    if resp.hovered() {
        p.rect_filled(rect, egui::CornerRadius::same(6), tk.p.hover);
    }
    let (ic, color, _) = kind_style(&tk, k);
    let at = egui::pos2(rect.left() + 14.0, rect.top() + 13.0);
    if k == Kind::Running {
        spinner(&p, at, color, app.now);
    } else {
        p.text(at, egui::Align2::CENTER_CENTER, ic, egui::FontId::proportional(14.0), color);
    }
    let x = rect.left() + 30.0;
    let time = widgets::rel_time(th.thread.updated_at, now);
    let tg = p.layout_no_wrap(time, egui::FontId::proportional(10.5), tk.p.faint);
    let tw = tg.size().x;
    p.galley(egui::pos2(rect.right() - 8.0 - tw, rect.top() + 13.0 - tg.size().y / 2.0), tg, tk.p.faint);
    let font = if k == Kind::Running { egui::FontId::proportional(tk.text_size) } else { egui::FontId::proportional(tk.text_size + 0.5) };
    let mut job = egui::text::LayoutJob::single_section(
        th.thread.title.clone(),
        egui::TextFormat { font_id: font, color: tk.p.text, ..Default::default() },
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width((rect.right() - 16.0 - tw - x).max(10.0));
    let g = p.layout_job(job);
    p.galley(egui::pos2(x, rect.top() + 13.0 - g.size().y / 2.0), g, tk.p.text);
    let where_ = if here { format!("{ws_name} · {}", t("activity.thisWindow")) } else { ws_name.to_string() };
    let mut job = egui::text::LayoutJob::single_section(
        where_,
        egui::TextFormat { font_id: egui::FontId::proportional(tk.text_size - 2.0), color: tk.p.muted, ..Default::default() },
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width((rect.right() - 8.0 - x).max(10.0));
    let g = p.layout_job(job);
    p.galley(egui::pos2(x, rect.top() + 29.0 - g.size().y / 2.0), g, tk.p.muted);
    resp.on_hover_text(widgets::short_path(&th.thread.cwd))
}

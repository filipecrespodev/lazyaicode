//! A paleta (Ctrl+K): threads, pastas, workspaces e **todos** os comandos
//! (do motor e de tela). Um comando que precisa de parâmetros que a tela não
//! sabe abre um formulário gerado da descrição dele.

use egui::{Key, Modifiers, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular as icon;
use lazyaicode_i18n::t;
use lazyaicode_protocol::{CommandInfo, ParamType};
use serde_json::{json, Map, Value};

use crate::app::{LazyApp, ToastKind};
use crate::views::widgets;

enum Entry {
    Thread(String, String, String),
    Project(String, String),
    Workspace(String, String),
    Command(CommandInfo),
}

fn label_of(c: &CommandInfo) -> String {
    let l = t(&c.label);
    if l == c.label && c.label.contains('.') && !c.label.contains(' ') {
        c.id.clone()
    } else {
        l
    }
}

fn score(hay: &str, q: &str) -> i32 {
    if q.is_empty() {
        return 1;
    }
    let h = hay.to_lowercase();
    if h.starts_with(q) {
        4
    } else if h.split([' ', '.', '-', '/']).any(|w| w.starts_with(q)) {
        3
    } else if h.contains(q) {
        2
    } else {
        // subsequência (letras na ordem)
        let mut it = h.chars();
        if q.chars().all(|c| it.any(|x| x == c)) {
            1
        } else {
            0
        }
    }
}

fn entries(app: &LazyApp, query: &str) -> Vec<Entry> {
    let (only_cmds, q) = match query.strip_prefix('>') {
        Some(rest) => (true, rest.trim().to_lowercase()),
        None => (false, query.trim().to_lowercase()),
    };
    let mut scored: Vec<(i32, Entry)> = Vec::new();
    if !only_cmds {
        for th in &app.data.threads {
            let project = app.data.project(&th.thread.project_id).map(|p| p.name.clone()).unwrap_or_default();
            let s = score(&th.thread.title, &q).max(score(&project, &q) - 1);
            if s > 0 && (!q.is_empty() || !th.thread.archived) {
                scored.push((s + 1, Entry::Thread(th.thread.id.clone(), th.thread.title.clone(), project)));
            }
        }
        for p in &app.data.projects {
            let s = score(&p.name, &q);
            if s > 0 && !q.is_empty() {
                scored.push((s, Entry::Project(p.id.clone(), p.name.clone())));
            }
        }
        for w in &app.data.workspaces {
            let s = score(&w.name, &q);
            if s > 0 && !q.is_empty() {
                scored.push((s, Entry::Workspace(w.id.clone(), w.name.clone())));
            }
        }
    }
    if only_cmds || !q.is_empty() {
        for c in &app.data.commands {
            if c.id == "ui.palette.toggle" {
                continue;
            }
            let s = score(&label_of(c), &q).max(score(&c.id, &q)).max(score(&c.group, &q) - 1);
            if s > 0 {
                scored.push((s, Entry::Command(c.clone())));
            }
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().take(80).map(|(_, e)| e).collect()
}

fn run_command(app: &mut LazyApp, c: &CommandInfo, params: Map<String, Value>) {
    app.state.palette_open = false;
    app.eph.palette_form = None;
    if c.scope == "ui" {
        if let Err(e) = crate::commands::run_ui(app, &c.id, &Value::Object(params)) {
            app.toast_err(&e);
        }
        return;
    }
    let read = c.read_only;
    let label = label_of(c);
    app.call(&c.id, Value::Object(params), move |app, r| match r {
        Ok(v) if read && !v.is_null() => {
            let text = serde_json::to_string_pretty(&v).unwrap_or_default();
            app.eph.palette_result = Some((label, text));
        }
        Ok(_) => app.toast(format!("✓ {label}"), ToastKind::Ok),
        Err(e) => app.toast_err(&e.message),
    });
}

fn choose(app: &mut LazyApp, e: &Entry) {
    match e {
        Entry::Thread(id, ..) => {
            app.state.palette_open = false;
            if app.data.thread(id).is_some_and(|t| t.thread.archived) {
                app.state.show_archived = true;
            }
            app.select_thread(id);
        }
        Entry::Project(id, _) => {
            app.state.palette_open = false;
            let _ = crate::commands::run_ui(app, "ui.repo.open", &json!({ "projectId": id }));
        }
        Entry::Workspace(id, _) => {
            app.state.palette_open = false;
            app.switch_workspace(id);
        }
        Entry::Command(c) => {
            let mut m = Map::new();
            crate::commands::fill_context(app, c, &mut m);
            let missing = crate::commands::missing_params(c, &m);
            let unfilled_optional = c.params.iter().any(|p| !p.required && !m.contains_key(&p.name));
            if missing.is_empty() && (c.read_only || c.scope == "ui" || !unfilled_optional) {
                run_command(app, c, m);
            } else {
                app.eph.palette_form = Some((c.clone(), m));
            }
        }
    }
}

pub fn show(app: &mut LazyApp, ctx: &egui::Context) {
    if let Some((title, text)) = app.eph.palette_result.clone() {
        let mut open = true;
        egui::Window::new(title).open(&mut open).default_size(Vec2::new(620.0, 460.0)).show(ctx, |ui| {
            if ui.button(format!("{} {}", icon::COPY, t("app.copy"))).clicked() {
                app.eph.copy_request = Some(text.clone());
            }
            egui::ScrollArea::both().show(ui, |ui| {
                ui.add(egui::Label::new(RichText::new(&text).monospace()).selectable(true));
            });
        });
        if !open {
            app.eph.palette_result = None;
        }
    }
    if !app.state.palette_open {
        return;
    }
    let tk = app.tokens;
    let screen = ctx.content_rect();
    let width = 640.0f32.min(screen.width() - 40.0);
    let mut close = ctx.input(|i| i.key_pressed(Key::Escape));
    egui::Area::new(egui::Id::new("palette"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(screen.center().x - width / 2.0, screen.top() + 60.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(tk.p.elev)
                .stroke(Stroke::new(1.0, tk.p.border_strong))
                .corner_radius(egui::CornerRadius::same(tk.radius + 2))
                .shadow(ui.visuals().popup_shadow)
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(width);
                    if let Some((c, mut m)) = app.eph.palette_form.take() {
                        let run = form(app, ui, &c, &mut m);
                        if run {
                            run_command(app, &c, m);
                        } else if app.state.palette_open && !close {
                            app.eph.palette_form = Some((c, m));
                        }
                        return;
                    }
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut app.state.palette_query)
                            .hint_text(t("palette.placeholder"))
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Heading)
                            .frame(egui::Frame::NONE),
                    );
                    r.request_focus();
                    if r.changed() {
                        app.eph.palette_index = 0;
                    }
                    widgets::hline(ui, &tk);
                    let list = entries(app, &app.state.palette_query.clone());
                    let n = list.len();
                    ui.input_mut(|i| {
                        if i.consume_key(Modifiers::NONE, Key::ArrowDown) && n > 0 {
                            app.eph.palette_index = (app.eph.palette_index + 1) % n;
                        }
                        if i.consume_key(Modifiers::NONE, Key::ArrowUp) && n > 0 {
                            app.eph.palette_index = (app.eph.palette_index + n - 1) % n;
                        }
                    });
                    let enter = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                    let mut chosen: Option<usize> = enter.then_some(app.eph.palette_index.min(n.saturating_sub(1))).filter(|_| n > 0);
                    if n == 0 {
                        ui.label(widgets::faint(&tk, t("palette.empty")));
                        ui.label(widgets::faint(&tk, t("palette.tip")));
                    }
                    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                        for (i, e) in list.iter().enumerate() {
                            let on = i == app.eph.palette_index;
                            let (ic, title, detail, sc) = match e {
                                Entry::Thread(_, title, project) => (icon::CHAT_CIRCLE, title.clone(), project.clone(), None),
                                Entry::Project(_, name) => (icon::FOLDER, name.clone(), t("palette.project"), None),
                                Entry::Workspace(_, name) => (icon::BRIEFCASE, name.clone(), t("palette.workspace"), None),
                                Entry::Command(c) => (
                                    if c.scope == "ui" { icon::CURSOR_CLICK } else { icon::TERMINAL },
                                    label_of(c),
                                    c.id.clone(),
                                    c.shortcut.clone(),
                                ),
                            };
                            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), tk.row_h + 4.0), Sense::click());
                            if on || resp.hovered() {
                                ui.painter().rect_filled(
                                    rect,
                                    egui::CornerRadius::same(tk.radius_sm),
                                    if on { tk.p.active } else { tk.p.hover },
                                );
                            }
                            if on {
                                resp.scroll_to_me(None);
                            }
                            let p = ui.painter_at(rect);
                            p.text(
                                rect.left_center() + Vec2::new(10.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                ic,
                                egui::FontId::proportional(tk.text_size + 1.0),
                                tk.p.muted,
                            );
                            p.text(
                                rect.left_center() + Vec2::new(32.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                &title,
                                egui::FontId::proportional(tk.text_size),
                                tk.p.text,
                            );
                            let right = sc.map(|s| widgets::pretty_shortcut(&s)).unwrap_or(detail);
                            p.text(
                                rect.right_center() - Vec2::new(10.0, 0.0),
                                egui::Align2::RIGHT_CENTER,
                                right,
                                egui::FontId::proportional(tk.text_size - 2.0),
                                tk.p.faint,
                            );
                            if resp.clicked() {
                                chosen = Some(i);
                            }
                        }
                    });
                    if let Some(i) = chosen {
                        if let Some(e) = list.get(i) {
                            choose(app, e);
                        }
                    }
                });
        });
    if close || ctx.input(|i| i.pointer.any_click()) && !ctx.is_pointer_over_egui() {
        close = true;
    }
    if close {
        app.state.palette_open = false;
        app.eph.palette_form = None;
    }
}

/// Formulário dos parâmetros de um comando. Devolve `true` para executar.
fn form(app: &mut LazyApp, ui: &mut egui::Ui, c: &CommandInfo, m: &mut Map<String, Value>) -> bool {
    let tk = app.tokens;
    ui.horizontal(|ui| {
        ui.label(RichText::new(label_of(c)).strong().size(tk.text_size + 2.0));
        ui.label(widgets::faint(&tk, &c.id));
    });
    if !c.doc.is_empty() {
        ui.label(widgets::muted(&tk, &c.doc));
    }
    ui.add_space(6.0);
    let mut first = true;
    egui::Grid::new(("form", &c.id)).num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        for p in &c.params {
            let name = if p.required { format!("{} *", p.name) } else { p.name.clone() };
            ui.label(RichText::new(name).monospace()).on_hover_text(&p.doc);
            let v = m.entry(p.name.clone()).or_insert(Value::Null);
            match &p.ty {
                ParamType::Bool => {
                    let mut b = v.as_bool().unwrap_or(false);
                    if ui.checkbox(&mut b, "").changed() || !v.is_null() || p.required {
                        *v = json!(b);
                    }
                }
                ParamType::Enum(opts) => {
                    let cur = v.as_str().unwrap_or("").to_string();
                    egui::ComboBox::from_id_salt(("enum", &c.id, &p.name))
                        .selected_text(if cur.is_empty() { "—".into() } else { cur.clone() })
                        .show_ui(ui, |ui| {
                            for o in opts {
                                if ui.selectable_label(cur == *o, o).clicked() {
                                    *v = json!(o);
                                }
                            }
                        });
                }
                ParamType::Int | ParamType::Number => {
                    let mut s = match v {
                        Value::Number(n) => n.to_string(),
                        _ => String::new(),
                    };
                    let r = ui.add(egui::TextEdit::singleline(&mut s).desired_width(160.0));
                    if first {
                        r.request_focus();
                        first = false;
                    }
                    *v = if s.trim().is_empty() {
                        Value::Null
                    } else if p.ty == ParamType::Int {
                        s.trim().parse::<i64>().map(|n| json!(n)).unwrap_or(Value::Null)
                    } else {
                        s.trim().parse::<f64>().map(|n| json!(n)).unwrap_or(Value::Null)
                    };
                }
                ParamType::Strings => {
                    let mut s = v.as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n")).unwrap_or_default();
                    ui.add(egui::TextEdit::multiline(&mut s).desired_rows(2).hint_text(t("palette.onePerLine")));
                    *v = if s.trim().is_empty() {
                        Value::Null
                    } else {
                        json!(s.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>())
                    };
                }
                ParamType::Json => {
                    let mut s = if v.is_null() { String::new() } else { v.to_string() };
                    ui.add(egui::TextEdit::multiline(&mut s).desired_rows(2).code_editor().hint_text("{ }"));
                    *v = if s.trim().is_empty() { Value::Null } else { serde_json::from_str(&s).unwrap_or(Value::String(s)) };
                }
                ParamType::String | ParamType::Path => {
                    let mut s = v.as_str().unwrap_or("").to_string();
                    ui.horizontal(|ui| {
                        let r = ui.add(egui::TextEdit::singleline(&mut s).desired_width(360.0));
                        if first {
                            r.request_focus();
                            first = false;
                        }
                    });
                    *v = if s.is_empty() { Value::Null } else { json!(s) };
                }
            }
            ui.end_row();
        }
    });
    m.retain(|_, v| !v.is_null());
    let missing = crate::commands::missing_params(c, m);
    ui.add_space(8.0);
    let mut run = false;
    ui.horizontal(|ui| {
        let enabled = missing.is_empty();
        if ui
            .add_enabled(
                enabled,
                egui::Button::new(RichText::new(format!("{}  {}", icon::PLAY, t("palette.run"))).color(tk.on_accent)).fill(tk.p.accent),
            )
            .clicked()
        {
            run = true;
        }
        if ui.button(t("app.cancel")).clicked() {
            app.state.palette_open = false;
        }
        if !missing.is_empty() {
            ui.label(widgets::faint(&tk, format!("{}: {}", t("palette.missing"), missing.join(", "))));
        }
    });
    if missing.is_empty() && ui.input(|i| i.modifiers.command && i.key_pressed(Key::Enter)) {
        run = true;
    }
    run
}

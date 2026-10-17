//! A barra lateral: as pastas do workspace com as threads de cada uma (ou
//! agrupadas por data), o filtro e o perfil.

use egui::{Align, Color32, Layout, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular as icon;
use lazyaicode_doc::tree::{self, GroupKind, Row, TreeOpts};
use lazyaicode_i18n::t;
use serde_json::json;

use crate::app::LazyApp;
use crate::state::TreeMode;
use crate::views::widgets;

const PER_GROUP: usize = 8;

fn groups(app: &LazyApp) -> Vec<tree::Group> {
    let now = app.host.now_ms();
    let today = crate::host::today_start(now, app.host.tz_offset_ms());
    let mode = match app.state.tree_mode {
        TreeMode::Project => tree::TreeMode::Project,
        TreeMode::Recent => tree::TreeMode::Recent,
    };
    tree::build_groups(
        &app.data.threads,
        &app.data.projects,
        &TreeOpts { mode, filter: &app.state.filter, archived: app.state.show_archived, today_start: today },
    )
}

fn rows(app: &LazyApp, groups: &[tree::Group]) -> Vec<Row> {
    tree::flatten(groups, &app.state.collapsed, &app.state.show_all, PER_GROUP, !app.state.filter.trim().is_empty())
}

pub fn group_keys(app: &LazyApp) -> Vec<String> {
    groups(app).into_iter().map(|g| g.key).collect()
}

/// As threads na ordem em que aparecem (setas e `Alt+1…9`).
pub fn visible_thread_ids(app: &LazyApp) -> Vec<String> {
    let g = groups(app);
    rows(app, &g)
        .into_iter()
        .filter_map(|r| match r {
            Row::Thread { group, index, .. } => g.get(group).and_then(|g| g.threads.get(index)).map(|t| t.thread.id.clone()),
            _ => None,
        })
        .collect()
}

pub fn show(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    let frame = egui::Frame::new().fill(tk.p.panel).inner_margin(egui::Margin::ZERO);
    let resp =
        egui::Panel::left("sidebar").default_size(app.state.sidebar_width).size_range(180.0..=600.0).resizable(true).frame(frame).show(
            ui,
            |ui| {
                let panel = ui.max_rect();
                ui.painter().vline(panel.right() - 0.5, panel.y_range(), Stroke::new(1.0, tk.p.border));
                header(app, ui);
                search(app, ui);
                let footer_h = 41.0;
                let avail = ui.available_height() - footer_h;
                egui::Frame::new().inner_margin(egui::Margin { left: 6, right: 6, top: 0, bottom: 0 }).show(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(avail.max(50.0)).auto_shrink([false, false]).show(ui, |ui| {
                        tree_rows(app, ui);
                        ui.add_space(10.0);
                    });
                });
                ui.with_layout(Layout::bottom_up(Align::Min), |ui| footer(app, ui));
            },
        );
    let w = resp.response.rect.width();
    if (w - app.state.sidebar_width).abs() > 1.0 {
        app.state.sidebar_width = w;
    }
}

/// Botão de 24 px do cabeçalho da barra lateral.
fn tool(ui: &mut egui::Ui, tk: &crate::theme::Tokens, ic: &str, tip: &str, on: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
    if on {
        ui.painter().rect_filled(rect, egui::CornerRadius::same(5), tk.p.active);
    } else if r.hovered() {
        ui.painter().rect_filled(rect, egui::CornerRadius::same(5), tk.p.hover);
    }
    let color = if on || r.hovered() { tk.p.text } else { tk.p.muted };
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, ic, egui::FontId::proportional(13.5), color);
    r.on_hover_text(tip)
}

fn header(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    egui::Frame::new().inner_margin(egui::Margin { left: 14, right: 8, top: 8, bottom: 4 }).show(ui, |ui| {
        ui.horizontal(|ui| {
            let label = t(if app.state.show_archived { "sidebar.archived" } else { "sidebar.threads" }).to_uppercase();
            ui.label(RichText::new(label).size(11.0).strong().color(tk.p.faint).extra_letter_spacing(0.6));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 1.0;
                let tip = lazyaicode_i18n::tf("sidebar.newThread", &[("key", &"Ctrl+N")]);
                if tool(ui, &tk, icon::PLUS, &tip, false).clicked() {
                    if let Err(e) = crate::commands::new_thread(app, &json!({}), false) {
                        app.toast_err(&e);
                    }
                }
                let archived = app.state.show_archived;
                let tip = t(if archived { "sidebar.showActive" } else { "sidebar.showArchived" });
                if tool(ui, &tk, icon::ARCHIVE, &tip, archived).clicked() {
                    app.state.show_archived = !archived;
                }
                let g = groups(app);
                let any_open = g.iter().any(|gr| tree::is_open(gr, &app.state.collapsed, false));
                let (ic, tip) = if any_open {
                    (icon::ARROWS_IN_LINE_VERTICAL, t("sidebar.collapseAll"))
                } else {
                    (icon::ARROWS_OUT_LINE_VERTICAL, t("sidebar.expandAll"))
                };
                if tool(ui, &tk, ic, &tip, false).clicked() {
                    let id = if any_open { "ui.sidebar.collapse-all" } else { "ui.sidebar.expand-all" };
                    let _ = crate::commands::run_ui(app, id, &serde_json::Value::Null);
                }
                let recent = app.state.tree_mode == TreeMode::Recent;
                let (ic, tip) = if recent { (icon::FOLDER, t("sidebar.groupByProject")) } else { (icon::CLOCK, t("sidebar.groupByDate")) };
                if tool(ui, &tk, ic, &tip, recent).clicked() {
                    app.state.tree_mode = if recent { TreeMode::Project } else { TreeMode::Recent };
                }
            });
        });
    });
}

/// O filtro: uma caixa com a lupa, como no app anterior.
fn search(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    egui::Frame::new().inner_margin(egui::Margin { left: 8, right: 8, top: 4, bottom: 6 }).show(ui, |ui| {
        let id = egui::Id::new("sidebar-filter");
        let focused = ui.memory(|m| m.has_focus(id));
        egui::Frame::new()
            .fill(tk.p.input)
            .stroke(Stroke::new(1.0, if focused { tk.p.border_strong } else { tk.p.border }))
            .corner_radius(egui::CornerRadius::same(tk.radius_sm))
            .inner_margin(egui::Margin { left: 8, right: 6, top: 5, bottom: 5 })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(RichText::new(icon::MAGNIFYING_GLASS).size(12.0).color(tk.p.faint));
                    let clear_w = if app.state.filter.is_empty() { 0.0 } else { 18.0 };
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut app.state.filter)
                            .id(id)
                            .hint_text(RichText::new(t("sidebar.filterPlaceholder")).color(tk.p.faint))
                            .frame(egui::Frame::NONE)
                            .font(egui::FontId::proportional(tk.text_size - 0.5))
                            .desired_width(ui.available_width() - clear_w),
                    );
                    if r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        app.state.filter.clear();
                    }
                    if !app.state.filter.is_empty()
                        && ui
                            .add(egui::Button::new(RichText::new(icon::X).size(11.0).color(tk.p.faint)).frame(false))
                            .on_hover_text(t("sidebar.clear"))
                            .clicked()
                    {
                        app.state.filter.clear();
                    }
                });
            });
    });
}

/// Fundo de uma linha da árvore e a linha-guia da hierarquia.
fn row_bg(ui: &egui::Ui, tk: &crate::theme::Tokens, rect: egui::Rect, depth: usize, on: bool, hovered: bool, guide: bool) {
    let p = ui.painter();
    if on {
        p.rect_filled(rect, egui::CornerRadius::same(5), tk.p.active);
    } else if hovered {
        p.rect_filled(rect, egui::CornerRadius::same(5), tk.p.hover);
    }
    if guide && depth > 0 {
        let x = rect.left() + depth as f32 * 14.0 - 4.0 + 0.5;
        p.vline(x, rect.y_range(), Stroke::new(1.0, tk.p.border));
    }
}

fn tree_rows(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    let g = groups(app);
    let rows = rows(app, &g);
    if app.data.projects.is_empty() && app.data.threads.is_empty() {
        ui.add_space(14.0);
        ui.label(widgets::muted(&tk, t("sidebar.noProjects")));
        return;
    }
    if rows.is_empty() && !app.state.filter.trim().is_empty() {
        ui.add_space(14.0);
        ui.label(widgets::muted(&tk, lazyaicode_i18n::tf("sidebar.nothingMatches", &[("filter", &app.state.filter)])));
        return;
    }
    let now = app.host.now_ms();
    let width = ui.available_width();
    ui.spacing_mut().item_spacing.y = 0.0;
    for row in rows {
        match row {
            Row::Group { key, group, open, depth } => {
                let Some(gr) = g.get(group) else { continue };
                let label = if gr.kind == GroupKind::Project { gr.label.clone() } else { t(&gr.label) };
                let ic = match gr.kind {
                    GroupKind::Favorites => Some(icon::BOOKMARK_SIMPLE),
                    GroupKind::Time => None,
                    GroupKind::Project => Some(icon::FOLDER),
                };
                ui.add_space(2.0);
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, tk.row_h), Sense::click());
                let project_id = gr.project.as_ref().map(|p| p.id.clone());
                let is_git = gr.project.as_ref().is_some_and(|p| p.is_git);
                let repo_on = project_id.is_some() && app.state.repo_project == project_id && app.state.selected.is_none();
                row_bg(ui, &tk, rect, depth, repo_on, resp.hovered(), false);
                let p = ui.painter_at(rect);
                let cy = rect.center().y;
                let mut x = rect.left() + 6.0 + depth as f32 * 14.0;
                // a seta recolhe/expande; o nome abre a tela do repositório
                let chev = egui::Rect::from_min_size(egui::pos2(x - 2.0, rect.top()), Vec2::new(16.0, rect.height()));
                let chev_r = ui.interact(chev, resp.id.with("chev"), Sense::click()).on_hover_text(t("sidebar.collapseExpand"));
                p.text(
                    egui::pos2(x + 5.0, cy),
                    egui::Align2::CENTER_CENTER,
                    if open { icon::CARET_DOWN } else { icon::CARET_RIGHT },
                    egui::FontId::proportional(11.0),
                    if chev_r.hovered() { tk.p.text } else { tk.p.faint },
                );
                x += 16.0;
                if let Some(ic) = ic {
                    p.text(egui::pos2(x, cy), egui::Align2::LEFT_CENTER, ic, egui::FontId::proportional(12.5), tk.p.muted);
                    x += 18.0;
                }
                let color = if gr.kind == GroupKind::Project { tk.p.text } else { tk.p.muted };
                // à direita: a contagem (ou o + sob o mouse) e a atividade
                let count = gr.threads.len().to_string();
                let count_g = p.layout_no_wrap(count, egui::FontId::proportional(11.0), tk.p.faint);
                let mut right = rect.right() - 6.0;
                let hover = resp.hovered() || chev_r.hovered();
                let mut plus_clicked = false;
                if hover && project_id.is_some() {
                    let plus = egui::Rect::from_center_size(egui::pos2(right - 10.0, cy), Vec2::splat(20.0));
                    let name = gr.label.clone();
                    let r = ui
                        .interact(plus, resp.id.with("new"), Sense::click())
                        .on_hover_text(lazyaicode_i18n::tf("sidebar.newThreadIn", &[("name", &name)]));
                    if r.hovered() {
                        ui.painter().rect_filled(plus, egui::CornerRadius::same(4), tk.p.active);
                    }
                    ui.painter().text(
                        plus.center(),
                        egui::Align2::CENTER_CENTER,
                        icon::PLUS,
                        egui::FontId::proportional(12.0),
                        if r.hovered() { tk.p.text } else { tk.p.muted },
                    );
                    plus_clicked = r.clicked();
                    right = plus.left() - 4.0;
                } else {
                    let w = count_g.size().x;
                    p.galley(egui::pos2(right - w, cy - count_g.size().y / 2.0), count_g, tk.p.faint);
                    right -= w + 6.0;
                }
                let running = gr.threads.iter().filter(|t| t.status == lazyaicode_doc::Status::Running).count();
                if running > 0 {
                    let txt = format!("{} {running}", icon::CIRCLE_NOTCH);
                    let g2 = p.layout_no_wrap(txt, egui::FontId::proportional(11.0), tk.p.accent);
                    let w = g2.size().x;
                    p.galley(egui::pos2(right - w, cy - g2.size().y / 2.0), g2, tk.p.accent);
                    right -= w + 6.0;
                } else if !open {
                    if let Some(a) = gr.activity {
                        let c = match a {
                            tree::Activity::Awaiting => tk.p.amber,
                            tree::Activity::Running => tk.p.blue,
                            tree::Activity::Error => tk.p.red,
                            tree::Activity::Unread => tk.p.accent,
                        };
                        p.circle_filled(egui::pos2(right - 4.0, cy), 3.5, c);
                        right -= 14.0;
                    }
                }
                let mut job = egui::text::LayoutJob::single_section(
                    label,
                    egui::TextFormat { font_id: egui::FontId::proportional(tk.text_size), color, ..Default::default() },
                );
                job.wrap = egui::text::TextWrapping::truncate_at_width((right - x).max(10.0));
                let lg = p.layout_job(job);
                p.galley(egui::pos2(x, cy - lg.size().y / 2.0), lg, color);
                if plus_clicked {
                    if let Some(pid) = &project_id {
                        if let Err(e) = crate::commands::new_thread(app, &json!({ "projectId": pid }), false) {
                            app.toast_err(&e);
                        }
                    }
                    continue;
                }
                let toggle = |app: &mut LazyApp| {
                    let cur = tree::is_open(gr, &app.state.collapsed, false);
                    app.state.collapsed.insert(key.clone(), cur);
                };
                if chev_r.clicked() {
                    toggle(app);
                } else if resp.clicked() {
                    match &project_id {
                        Some(pid) if is_git => {
                            let _ = crate::commands::run_ui(app, "ui.repo.open", &json!({ "projectId": pid }));
                        }
                        _ => toggle(app),
                    }
                }
                let resp = match gr.project.as_ref() {
                    Some(pr) if is_git => resp.on_hover_text(format!("{}\n{}", widgets::short_path(&pr.path), t("sidebar.openRepoHint"))),
                    Some(pr) => resp.on_hover_text(widgets::short_path(&pr.path)),
                    None => resp,
                };
                if let Some(pid) = project_id {
                    let path = gr.project.as_ref().map(|p| p.path.clone()).unwrap_or_default();
                    resp.context_menu(|ui| {
                        if ui.button(format!("{}  {}", icon::NOTE_PENCIL, t("sidebar.newThreadShort"))).clicked() {
                            let _ = crate::commands::new_thread(app, &json!({ "projectId": pid }), false);
                            ui.close();
                        }
                        if is_git && ui.button(format!("{}  {}", icon::STACK, t("sidebar.newIsolated"))).clicked() {
                            let _ = crate::commands::new_thread(app, &json!({ "projectId": pid }), true);
                            ui.close();
                        }
                        if is_git && ui.button(format!("{}  {}", icon::GIT_BRANCH, t("sidebar.openRepo"))).clicked() {
                            let _ = crate::commands::run_ui(app, "ui.repo.open", &json!({ "projectId": pid }));
                            ui.close();
                        }
                        if ui.button(format!("{}  {}", icon::FOLDER_OPEN, t("sidebar.openFolder"))).clicked() {
                            app.fire("system.open-path", json!({ "path": path }));
                            ui.close();
                        }
                        if ui.button(format!("{}  {}", icon::COPY, t("sidebar.copyPath"))).clicked() {
                            app.eph.copy_request = Some(path.clone());
                            ui.close();
                        }
                    });
                }
            }
            Row::Thread { group, index, depth, .. } => {
                let Some(th) = g.get(group).and_then(|g| g.threads.get(index)) else { continue };
                let show_project = app.state.tree_mode == TreeMode::Recent || g.get(group).is_some_and(|g| g.kind == GroupKind::Favorites);
                thread_row(app, ui, th, now, width, depth, show_project);
            }
            Row::More { key, hidden, depth, .. } => {
                let parent = key.trim_end_matches(":more").to_string();
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, tk.row_h - 4.0), Sense::click());
                row_bg(ui, &tk, rect, depth, false, false, true);
                let color = if resp.hovered() { tk.p.text } else { tk.p.faint };
                ui.painter().text(
                    egui::pos2(rect.left() + 20.0 + depth as f32 * 14.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    lazyaicode_i18n::tf("sidebar.showMore", &[("n", &hidden)]),
                    egui::FontId::proportional(tk.text_size - 1.5),
                    color,
                );
                if resp.clicked() {
                    app.state.show_all.insert(parent, true);
                }
            }
            Row::Empty { depth, .. } => {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(width, tk.row_h - 4.0), Sense::hover());
                row_bg(ui, &tk, rect, depth, false, false, true);
                ui.painter().text(
                    egui::pos2(rect.left() + 20.0 + depth as f32 * 14.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    t("sidebar.noThreads"),
                    egui::FontId::proportional(tk.text_size - 1.5),
                    tk.p.faint,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn thread_row(
    app: &mut LazyApp,
    ui: &mut egui::Ui,
    th: &lazyaicode_doc::ThreadView,
    now: i64,
    width: f32,
    depth: usize,
    show_project: bool,
) {
    let tk = app.tokens;
    let id = th.thread.id.clone();
    let selected = app.state.selected.as_deref() == Some(id.as_str());
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, tk.row_h), Sense::click());
    row_bg(ui, &tk, rect, depth, selected, resp.hovered(), true);
    let painter = ui.painter_at(rect);
    // esperando você: a linha inteira chama atenção
    if th.status == lazyaicode_doc::Status::Awaiting {
        painter.rect_filled(rect, egui::CornerRadius::same(4), tk.p.amber.gamma_multiply(0.10));
        painter.rect_filled(
            egui::Rect::from_min_size(rect.left_top(), Vec2::new(2.0, rect.height())),
            egui::CornerRadius::same(1),
            tk.p.amber,
        );
    }
    let cy = rect.center().y;
    let mut x = rect.left() + 6.0 + depth as f32 * 14.0;
    // coluna de status (10 px)
    let dot = egui::pos2(x + 5.0, cy);
    match th.status {
        lazyaicode_doc::Status::Running => {
            let a = (app.now * 4.0) as f32;
            painter.circle_stroke(dot, 4.0, Stroke::new(1.5, tk.p.accent.gamma_multiply(0.3)));
            let pts: Vec<egui::Pos2> = (0..10).map(|i| dot + Vec2::angled(a + i as f32 * 0.17) * 4.0).collect();
            painter.add(egui::Shape::line(pts, Stroke::new(1.5, tk.p.accent)));
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(60));
        }
        lazyaicode_doc::Status::Idle if th.thread.unread => {
            painter.circle_filled(dot, 3.5, tk.p.accent);
        }
        lazyaicode_doc::Status::Idle => {}
        s => {
            painter.circle_filled(dot, 3.5, widgets::status_color(&tk, s));
        }
    }
    x += 16.0;
    // à direita: o tempo (ou o arquivar, sob o mouse), o isolado, o provedor, o projeto
    let mut right = rect.right() - 6.0;
    let hovered = resp.hovered();
    let mut archive_clicked = false;
    if hovered {
        let ar = egui::Rect::from_center_size(egui::pos2(right - 10.0, cy), Vec2::splat(20.0));
        let r = ui.interact(ar, resp.id.with("archive"), Sense::click()).on_hover_text(t(if th.thread.archived {
            "sidebar.unarchive"
        } else {
            "sidebar.archive"
        }));
        if r.hovered() {
            ui.painter().rect_filled(ar, egui::CornerRadius::same(4), tk.p.active);
        }
        ui.painter().text(
            ar.center(),
            egui::Align2::CENTER_CENTER,
            icon::ARCHIVE,
            egui::FontId::proportional(12.0),
            if r.hovered() { tk.p.text } else { tk.p.muted },
        );
        archive_clicked = r.clicked();
        right = ar.left() - 4.0;
    } else {
        // rodando, esperando ou com erro: um selo no lugar da hora
        let badge = match th.status {
            lazyaicode_doc::Status::Running => Some(("sidebar.badgeRunning", tk.p.blue)),
            lazyaicode_doc::Status::Awaiting => Some(("sidebar.badgeAwaiting", tk.p.amber)),
            lazyaicode_doc::Status::Error => Some(("sidebar.badgeError", tk.p.red)),
            lazyaicode_doc::Status::Idle => None,
        };
        match badge {
            Some((key, c)) => {
                let tg = painter.layout_no_wrap(t(key), egui::FontId::proportional(10.0), c);
                let size = tg.size() + Vec2::new(10.0, 3.0);
                let br = egui::Rect::from_min_size(egui::pos2(right - size.x, cy - size.y / 2.0), size);
                painter.rect_filled(br, egui::CornerRadius::same(4), c.gamma_multiply(0.16));
                painter.galley(br.min + Vec2::new(5.0, 1.5), tg, c);
                right -= size.x + 6.0;
            }
            None => {
                let time = widgets::rel_time(th.thread.updated_at, now);
                let color = if th.thread.unread { tk.p.accent } else { tk.p.faint };
                let tg = painter.layout_no_wrap(time, egui::FontId::proportional(10.5), color);
                let w = tg.size().x;
                painter.galley(egui::pos2(right - w, cy - tg.size().y / 2.0), tg, color);
                right -= w + 6.0;
            }
        }
    }
    let mut badge = |text: &str, size: f32, color: Color32| {
        let g = painter.layout_no_wrap(text.to_string(), egui::FontId::proportional(size), color);
        let w = g.size().x;
        painter.galley(egui::pos2(right - w, cy - g.size().y / 2.0), g, color);
        right -= w + 5.0;
    };
    if th.thread.isolated {
        badge(icon::STACK, 11.0, tk.p.faint);
    }
    if th.thread.provider != "claude" {
        badge(if th.thread.provider == "codex" { "O" } else { "G" }, 10.0, tk.p.blue);
    }
    if th.thread.skip_permissions {
        badge(icon::WARNING, 11.0, tk.p.red);
    }
    if show_project {
        if let Some(p) = app.data.project(&th.thread.project_id) {
            let mut job = egui::text::LayoutJob::single_section(
                p.name.clone(),
                egui::TextFormat { font_id: egui::FontId::proportional(10.5), color: tk.p.faint, ..Default::default() },
            );
            job.wrap = egui::text::TextWrapping::truncate_at_width(64.0);
            let g = painter.layout_job(job);
            let w = g.size().x;
            painter.galley(egui::pos2(right - w, cy - g.size().y / 2.0), g, tk.p.faint);
            right -= w + 6.0;
        }
    }
    let unread = th.thread.unread;
    let color = if unread || selected { tk.p.text } else { tk.p.muted };
    let font = if unread {
        egui::FontId::new(tk.text_size, egui::FontFamily::Name("bold".into()))
    } else {
        egui::FontId::proportional(tk.text_size)
    };
    let font = if ui.fonts(|f| f.families().contains(&font.family)) { font } else { egui::FontId::proportional(tk.text_size) };
    let mut job =
        egui::text::LayoutJob::single_section(th.thread.title.clone(), egui::TextFormat { font_id: font, color, ..Default::default() });
    job.wrap = egui::text::TextWrapping::truncate_at_width((right - x).max(10.0));
    let g = painter.layout_job(job);
    painter.galley(egui::pos2(x, cy - g.size().y / 2.0), g, color);
    if th.thread.bookmarked {
        painter.text(
            egui::pos2(x - 16.0 + 5.0, rect.top() + 6.0),
            egui::Align2::CENTER_CENTER,
            icon::STAR,
            egui::FontId::proportional(8.0),
            tk.p.amber,
        );
    }
    if archive_clicked {
        let archived = !th.thread.archived;
        app.call_ok("thread.update", json!({ "id": id, "archived": archived }), |app, v| {
            if let Ok(t) = serde_json::from_value(v) {
                app.data.upsert(t);
            }
        });
        return;
    }
    let state = match th.status {
        lazyaicode_doc::Status::Running => t("sidebar.statusRunning"),
        lazyaicode_doc::Status::Awaiting => t("sidebar.statusAwaiting"),
        lazyaicode_doc::Status::Error => t("sidebar.statusError"),
        lazyaicode_doc::Status::Idle if th.thread.unread => t("activity.unread"),
        lazyaicode_doc::Status::Idle => String::new(),
    };
    let tip = if state.is_empty() {
        format!("{}\n{}", th.thread.title, widgets::short_path(&th.thread.cwd))
    } else {
        format!("{}\n{}\n{state}", th.thread.title, widgets::short_path(&th.thread.cwd))
    };
    let resp = resp.on_hover_text(tip);
    if resp.clicked() {
        app.select_thread(&id);
    }
    resp.context_menu(|ui| {
        app.state.selected = Some(id.clone());
        let item = |ui: &mut egui::Ui, ic: &str, key: &str| ui.button(format!("{ic}  {}", t(key))).clicked();
        if item(ui, icon::PENCIL_SIMPLE, "sidebar.rename") {
            app.select_thread(&id);
            app.eph.editing_title = Some(th.thread.title.clone());
            ui.close();
        }
        if item(ui, icon::STAR, if th.thread.bookmarked { "sidebar.removeFavorite" } else { "sidebar.favorite" }) {
            let _ = crate::commands::run_ui(app, "ui.thread.toggle-bookmark", &serde_json::Value::Null);
            ui.close();
        }
        if item(ui, icon::ENVELOPE_SIMPLE, "sidebar.markUnread") {
            app.fire("thread.update", json!({ "id": id, "unread": true }));
            if let Some(t) = app.data.thread_mut(&id) {
                t.thread.unread = true;
            }
            ui.close();
        }
        if item(ui, icon::COPY, "sidebar.copyPath") {
            app.eph.copy_request = Some(th.thread.cwd.clone());
            ui.close();
        }
        if item(ui, icon::FOLDER_OPEN, "sidebar.openFolder") {
            app.fire("system.open-path", json!({ "path": th.thread.cwd }));
            ui.close();
        }
        ui.separator();
        if item(ui, icon::ARCHIVE, if th.thread.archived { "sidebar.unarchive" } else { "thread.archiveThread" }) {
            let _ = crate::commands::run_ui(app, "ui.thread.archive", &serde_json::Value::Null);
            ui.close();
        }
        if ui.button(RichText::new(format!("{}  {}", icon::TRASH, t("sidebar.delete"))).color(tk.p.red)).clicked() {
            let _ = crate::commands::run_ui(app, "ui.thread.delete", &serde_json::Value::Null);
            ui.close();
        }
    });
}

fn footer(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    egui::Frame::new().inner_margin(egui::Margin { left: 10, right: 8, top: 8, bottom: 8 }).show(ui, |ui| {
        ui.horizontal(|ui| {
            let name = app.data.profile.as_ref().and_then(|p| p["name"].as_str()).unwrap_or("").to_string();
            let shown = if name.is_empty() { t("sidebar.you") } else { name.clone() };
            widgets::avatar(ui, &tk, &shown, false);
            ui.add_space(2.0);
            let email = app.data.profile.as_ref().and_then(|p| p["email"].as_str()).unwrap_or("").to_string();
            ui.add(egui::Label::new(RichText::new(&shown).color(tk.p.text).strong()).truncate()).on_hover_text(email);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let tip = lazyaicode_i18n::tf("sidebar.settings", &[("key", &"Ctrl+,")]);
                if tool(ui, &tk, icon::GEAR_SIX, &tip, false).clicked() {
                    app.state.settings_open = true;
                }
                let running = app.data.threads.iter().filter(|t| t.status == lazyaicode_doc::Status::Running).count();
                if running > 0 {
                    ui.label(
                        RichText::new(lazyaicode_i18n::tf("sidebar.running", &[("n", &running)]))
                            .color(tk.p.accent)
                            .size(tk.text_size - 2.0),
                    );
                }
            });
        });
    });
    let r = ui.min_rect();
    ui.painter().hline(r.x_range(), r.top() + 0.5, Stroke::new(1.0, tk.p.border));
}

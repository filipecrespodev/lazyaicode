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
    let frame = egui::Frame::new().fill(tk.p.panel).inner_margin(egui::Margin { left: 6, right: 6, top: 8, bottom: 6 });
    let resp = egui::Panel::left("sidebar")
        .default_size(app.state.sidebar_width)
        .size_range(180.0..=600.0)
        .resizable(true)
        .frame(frame)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                widgets::section(ui, &tk, &t(if app.state.show_archived { "sidebar.archived" } else { "sidebar.projects" }));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_button(ui, &tk, icon::NOTE_PENCIL, &format!("{} (Ctrl+N)", t("sidebar.newThreadShort"))).clicked() {
                        if let Err(e) = crate::commands::new_thread(app, &json!({}), false) {
                            app.toast_err(&e);
                        }
                    }
                    let archived = app.state.show_archived;
                    if widgets::icon_button_on(ui, &tk, icon::ARCHIVE, &t("sidebar.showArchived"), archived).clicked() {
                        app.state.show_archived = !archived;
                    }
                    let recent = app.state.tree_mode == TreeMode::Recent;
                    let (ic, tip) =
                        if recent { (icon::CLOCK, t("sidebar.groupByDate")) } else { (icon::FOLDERS, t("sidebar.groupByProject")) };
                    if widgets::icon_button(ui, &tk, ic, &tip).clicked() {
                        app.state.tree_mode = if recent { TreeMode::Project } else { TreeMode::Recent };
                    }
                });
            });
            ui.add_space(4.0);
            let filter = egui::TextEdit::singleline(&mut app.state.filter)
                .hint_text(format!("{}  {}", icon::FUNNEL_SIMPLE, t("sidebar.filterPlaceholder")))
                .desired_width(f32::INFINITY);
            ui.add(filter);
            ui.add_space(6.0);
            let footer_h = 40.0;
            let avail = ui.available_height() - footer_h;
            egui::ScrollArea::vertical().max_height(avail.max(50.0)).auto_shrink([false, false]).show(ui, |ui| tree_rows(app, ui));
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| footer(app, ui));
        });
    let w = resp.response.rect.width();
    if (w - app.state.sidebar_width).abs() > 1.0 {
        app.state.sidebar_width = w;
    }
}

fn tree_rows(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    let g = groups(app);
    let rows = rows(app, &g);
    if app.data.projects.is_empty() && app.data.threads.is_empty() {
        ui.label(widgets::faint(&tk, t("sidebar.noProjects")));
        return;
    }
    let now = app.host.now_ms();
    let width = ui.available_width();
    for row in rows {
        match row {
            Row::Group { key, group, open, .. } => {
                let Some(gr) = g.get(group) else { continue };
                let label = if gr.kind == GroupKind::Project { gr.label.clone() } else { t(&gr.label) };
                let ic = match gr.kind {
                    GroupKind::Favorites => icon::STAR,
                    GroupKind::Time => icon::CALENDAR_BLANK,
                    GroupKind::Project if gr.project.as_ref().is_some_and(|p| !p.is_git) => icon::FOLDER_SIMPLE,
                    GroupKind::Project => {
                        if open {
                            icon::FOLDER_OPEN
                        } else {
                            icon::FOLDER
                        }
                    }
                };
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, tk.row_h), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, egui::CornerRadius::same(tk.radius_sm), tk.p.hover);
                }
                let p = ui.painter();
                p.text(
                    rect.left_center() + Vec2::new(8.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    ic,
                    egui::FontId::proportional(tk.text_size + 1.0),
                    tk.p.muted,
                );
                p.text(
                    rect.left_center() + Vec2::new(28.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    &label,
                    egui::FontId::proportional(tk.text_size),
                    tk.p.text,
                );
                if !open {
                    if let Some(a) = gr.activity {
                        let c = match a {
                            tree::Activity::Awaiting => tk.p.amber,
                            tree::Activity::Running => tk.p.blue,
                            tree::Activity::Error => tk.p.red,
                            tree::Activity::Unread => tk.p.accent,
                        };
                        p.circle_filled(rect.right_center() - Vec2::new(12.0, 0.0), 3.5, c);
                    }
                }
                let project_id = gr.project.as_ref().map(|p| p.id.clone());
                if resp.hovered() {
                    if let Some(pid) = &project_id {
                        let plus = egui::Rect::from_center_size(rect.right_center() - Vec2::new(14.0, 0.0), Vec2::splat(20.0));
                        let r = ui.interact(plus, resp.id.with("new"), Sense::click()).on_hover_text(t("sidebar.newThreadHere"));
                        ui.painter().text(
                            plus.center(),
                            egui::Align2::CENTER_CENTER,
                            icon::PLUS,
                            egui::FontId::proportional(tk.text_size + 1.0),
                            if r.hovered() { tk.p.text } else { tk.p.muted },
                        );
                        if r.clicked() {
                            if let Err(e) = crate::commands::new_thread(app, &json!({ "projectId": pid }), false) {
                                app.toast_err(&e);
                            }
                            continue;
                        }
                    }
                }
                if resp.clicked() {
                    let cur = tree::is_open(gr, &app.state.collapsed, false);
                    app.state.collapsed.insert(key.clone(), cur);
                }
                if let Some(pid) = project_id {
                    let is_git = gr.project.as_ref().is_some_and(|p| p.is_git);
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
            Row::Thread { group, index, .. } => {
                let Some(th) = g.get(group).and_then(|g| g.threads.get(index)) else { continue };
                thread_row(app, ui, th, now, width);
            }
            Row::More { key, hidden, .. } => {
                let parent = key.trim_end_matches(":more").to_string();
                if ui
                    .add(egui::Button::new(widgets::muted(&tk, lazyaicode_i18n::tf("sidebar.showMore", &[("n", &hidden)]))).frame(false))
                    .clicked()
                {
                    app.state.show_all.insert(parent, true);
                }
            }
            Row::Empty { .. } => {
                ui.horizontal(|ui| {
                    ui.add_space(30.0);
                    ui.label(widgets::faint(&tk, t("sidebar.noThreads")));
                });
            }
        }
    }
}

fn thread_row(app: &mut LazyApp, ui: &mut egui::Ui, th: &lazyaicode_doc::ThreadView, now: i64, width: f32) {
    let tk = app.tokens;
    let id = th.thread.id.clone();
    let selected = app.state.selected.as_deref() == Some(id.as_str());
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, tk.row_h), Sense::click());
    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect, egui::CornerRadius::same(tk.radius_sm), tk.p.active);
    } else if resp.hovered() {
        painter.rect_filled(rect, egui::CornerRadius::same(tk.radius_sm), tk.p.hover);
    }
    // ponto de status na margem
    let dot = rect.left_center() + Vec2::new(14.0, 0.0);
    match th.status {
        lazyaicode_doc::Status::Running => {
            let a = (app.now * 4.0) as f32;
            painter.circle_stroke(dot, 4.0, Stroke::new(1.5, tk.p.blue.gamma_multiply(0.3)));
            let pts: Vec<egui::Pos2> = (0..10).map(|i| dot + Vec2::angled(a + i as f32 * 0.17) * 4.0).collect();
            painter.add(egui::Shape::line(pts, Stroke::new(1.5, tk.p.blue)));
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
    let time = widgets::rel_time(th.thread.updated_at, now);
    let time_g = painter.layout_no_wrap(time, egui::FontId::proportional(tk.text_size - 2.0), tk.p.faint);
    let time_w = time_g.size().x;
    painter.galley(rect.right_center() - Vec2::new(time_w + 8.0, time_g.size().y / 2.0), time_g, tk.p.faint);
    let mut x = rect.left() + 28.0;
    if th.thread.isolated {
        painter.text(
            egui::pos2(x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            icon::STACK,
            egui::FontId::proportional(tk.text_size - 1.0),
            tk.p.faint,
        );
        x += 16.0;
    }
    if th.thread.provider != "claude" {
        let short = if th.thread.provider == "codex" { "O" } else { "G" };
        painter.text(
            egui::pos2(x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            short,
            egui::FontId::proportional(tk.text_size - 2.0),
            tk.p.blue,
        );
        x += 12.0;
    }
    let color = if th.thread.unread || selected { tk.p.text } else { tk.p.text.gamma_multiply(0.85) };
    let max_w = (rect.right() - time_w - 14.0 - x).max(10.0);
    let mut job = egui::text::LayoutJob::single_section(
        th.thread.title.clone(),
        egui::TextFormat { font_id: egui::FontId::proportional(tk.text_size), color, ..Default::default() },
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w);
    let g = painter.layout_job(job);
    painter.galley(egui::pos2(x, rect.center().y - g.size().y / 2.0), g, color);
    if th.thread.bookmarked {
        painter.text(rect.left_center() + Vec2::new(4.0, 0.0), egui::Align2::LEFT_CENTER, "•", egui::FontId::proportional(8.0), tk.p.amber);
    }
    let resp = resp.on_hover_text(format!("{}\n{}", th.thread.title, widgets::short_path(&th.thread.cwd)));
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
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        let name = app.data.profile.as_ref().and_then(|p| p["name"].as_str()).unwrap_or("").to_string();
        let shown = if name.is_empty() { t("thread.you") } else { name.clone() };
        widgets::avatar(ui, &tk, &shown, false);
        ui.label(RichText::new(&shown).color(tk.p.text));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let running = app.data.threads.iter().filter(|t| t.status == lazyaicode_doc::Status::Running).count();
            if running > 0 {
                ui.label(
                    RichText::new(lazyaicode_i18n::tf("sidebar.running", &[("n", &running)])).color(tk.p.blue).size(tk.text_size - 2.0),
                );
            }
            if let Some(ws) = app.data.active() {
                let acc = ws.claude_dir.clone().unwrap_or_else(|| app.data.default_claude_dir.clone());
                let logged = app.data.auth.get(ws.claude_dir.as_deref().unwrap_or("")).and_then(|a| a["loggedIn"].as_bool());
                let c = match logged {
                    Some(true) => tk.p.accent,
                    Some(false) => tk.p.red,
                    None => tk.p.faint,
                };
                ui.label(RichText::new(icon::USER_CIRCLE).color(c)).on_hover_text(format!(
                    "{}: {}",
                    t("sidebar.account"),
                    widgets::short_path(&acc)
                ));
            }
        });
    });
    let _ = Color32::TRANSPARENT;
}

//! A escolha de quais workspaces abrir (na abertura do app, com mais de
//! um) e o diálogo de workspace novo.

use egui::{RichText, Stroke, Vec2};
use egui_phosphor::regular as icon;
use lazyaicode_i18n::t;
use serde_json::json;

use crate::app::LazyApp;
use crate::views::widgets;

pub fn show(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    egui::CentralPanel::default().frame(egui::Frame::new().fill(tk.p.bg)).show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.12);
            ui.label(RichText::new("LazyAICode").size(26.0).strong());
            ui.label(widgets::muted(&tk, t("workspacePicker.subtitle")));
            ui.add_space(18.0);
            let w = 460.0f32.min(ui.available_width() - 40.0);
            ui.allocate_ui(Vec2::new(w, 0.0), |ui| {
                egui::Frame::new()
                    .fill(tk.p.panel)
                    .stroke(Stroke::new(1.0, tk.p.border))
                    .corner_radius(egui::CornerRadius::same(tk.radius))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_width(w);
                        if app.data.workspaces.is_empty() {
                            ui.label(widgets::faint(&tk, t("workspacePicker.none")));
                        }
                        for ws in app.data.workspaces.clone() {
                            let mut on = app.eph.picker_checked.contains(&ws.id);
                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut on, RichText::new(&ws.name).strong()).changed() {
                                    if on {
                                        app.eph.picker_checked.push(ws.id.clone());
                                    } else {
                                        app.eph.picker_checked.retain(|x| *x != ws.id);
                                    }
                                }
                                ui.label(widgets::faint(&tk, widgets::short_path(&ws.path)));
                            });
                        }
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            let n = app.eph.picker_checked.len();
                            let label =
                                if n > 1 { lazyaicode_i18n::tf("workspacePicker.openN", &[("n", &n)]) } else { t("workspacePicker.open") };
                            if ui
                                .add_enabled(n > 0, egui::Button::new(RichText::new(label).color(tk.on_accent)).fill(tk.p.accent))
                                .clicked()
                            {
                                let ids = app.eph.picker_checked.clone();
                                open_selected(app, ids);
                            }
                            if widgets::button(ui, &tk, &format!("{}  {}", icon::FOLDER_PLUS, t("newWorkspace.title"))).clicked() {
                                app.state.new_workspace_open = true;
                            }
                            if app.data.active_workspace.is_some() && ui.button(t("app.cancel")).clicked() {
                                app.state.picking_workspaces = false;
                            }
                        });
                    });
            });
        });
    });
}

fn open_selected(app: &mut LazyApp, ids: Vec<String>) {
    if ids.is_empty() {
        return;
    }
    let active = app.data.active_workspace.clone();
    app.state.open_workspaces = ids.clone();
    app.state.picking_workspaces = false;
    match active.filter(|a| ids.contains(a)) {
        Some(a) if !app.data.threads.is_empty() || !app.data.projects.is_empty() => {
            let _ = a;
        }
        Some(a) => app.switch_workspace(&a),
        None => {
            if let Some(first) = ids.first() {
                app.switch_workspace(&first.clone());
            }
        }
    }
}

pub fn new_workspace_modal(app: &mut LazyApp, ctx: &egui::Context) {
    if !app.state.new_workspace_open {
        return;
    }
    let tk = app.tokens;
    let mut close = false;
    let modal = egui::Modal::new(egui::Id::new("new-workspace")).show(ctx, |ui| {
        ui.set_width(460.0);
        ui.heading(t("newWorkspace.title"));
        ui.label(widgets::muted(&tk, t("newWorkspace.hint")));
        ui.add_space(8.0);
        ui.label(t("newWorkspace.folder"));
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut app.eph.new_workspace_path).hint_text("/home/você/projetos").desired_width(330.0));
            if !app.host.is_web() && ui.button(format!("{} {}", icon::FOLDER_OPEN, t("newWorkspace.browse"))).clicked() {
                app.host.pick_folder(&t("newWorkspace.folder"), "ui.workspace.set-path", json!({}));
            }
        });
        ui.label(t("newWorkspace.name"));
        ui.add(
            egui::TextEdit::singleline(&mut app.eph.new_workspace_name).hint_text(t("newWorkspace.nameHint")).desired_width(f32::INFINITY),
        );
        ui.label(t("newWorkspace.account"));
        let accounts = app.data.accounts.clone();
        let suggested = app.data.suggested_claude_dir.clone();
        let cur = app.eph.new_workspace_account.clone().unwrap_or(suggested);
        egui::ComboBox::from_id_salt("nw-account").selected_text(widgets::short_path(&cur)).width(330.0).show_ui(ui, |ui| {
            for a in &accounts {
                let p = a["path"].as_str().unwrap_or("").to_string();
                if ui.selectable_label(p == cur, widgets::short_path(&p)).clicked() {
                    app.eph.new_workspace_account = Some(p);
                }
            }
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let ok = !app.eph.new_workspace_path.trim().is_empty();
            if ui
                .add_enabled(ok, egui::Button::new(RichText::new(t("newWorkspace.create")).color(tk.on_accent)).fill(tk.p.accent))
                .clicked()
            {
                let p = json!({
                    "path": app.eph.new_workspace_path.trim(),
                    "name": app.eph.new_workspace_name.trim(),
                    "claudeDir": app.eph.new_workspace_account.clone().unwrap_or_else(|| app.data.suggested_claude_dir.clone()),
                });
                app.call_ok("workspace.create", p, |app, v| {
                    app.eph.new_workspace_path.clear();
                    app.eph.new_workspace_name.clear();
                    app.eph.new_workspace_account = None;
                    app.state.new_workspace_open = false;
                    app.refresh_workspaces(false);
                    if let Some(id) = v["id"].as_str() {
                        app.data.workspaces.push(serde_json::from_value(v.clone()).unwrap_or_else(|_| lazyaicode_doc::Workspace {
                            id: id.into(),
                            name: String::new(),
                            path: String::new(),
                            claude_dir: None,
                            ignored_paths: vec![],
                            created_at: 0,
                        }));
                        let id = id.to_string();
                        app.switch_workspace(&id);
                    }
                });
            }
            if ui.button(t("app.cancel")).clicked() {
                close = true;
            }
        });
    });
    if close || modal.should_close() {
        app.state.new_workspace_open = false;
    }
}

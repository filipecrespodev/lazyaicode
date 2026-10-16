//! A barra de título do app: abas dos workspaces, busca (paleta),
//! configurações e, sem a barra do sistema, minimizar/maximizar/fechar.

use egui::{Align, Layout, RichText, Sense, Stroke, Vec2, ViewportCommand};
use egui_phosphor::regular as icon;
use lazyaicode_i18n::t;

use crate::app::LazyApp;
use crate::views::widgets;

pub fn viewport(app: &mut LazyApp, cmd: &'static str) {
    app.eph.viewport.push(cmd);
}

pub fn apply_viewport(ctx: &egui::Context, cmd: &str) {
    match cmd {
        "minimize" => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
        "maximize" => {
            let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!max));
        }
        "fullscreen" => {
            let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fs));
        }
        "close" => ctx.send_viewport_cmd(ViewportCommand::Close),
        "on-top" => {
            let id = egui::Id::new("always-on-top");
            let on = !ctx.data(|d| d.get_temp::<bool>(id).unwrap_or(false));
            ctx.data_mut(|d| d.insert_temp(id, on));
            ctx.send_viewport_cmd(ViewportCommand::WindowLevel(if on {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            }));
        }
        _ => {}
    }
}

pub fn show(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    let frame = egui::Frame::new().fill(tk.p.panel).inner_margin(egui::Margin::symmetric(6, 3)).stroke(Stroke::NONE);
    egui::Panel::top("titlebar").exact_size(36.0).frame(frame).show(ui, |ui| {
        // o fundo inteiro arrasta a janela (e duplo clique maximiza)
        let bar = ui.max_rect();
        let drag = ui.interact(bar, ui.id().with("drag"), Sense::click_and_drag());
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            viewport(app, "maximize");
        }
        ui.painter().hline(bar.x_range(), bar.bottom(), Stroke::new(1.0, tk.p.border));
        ui.horizontal_centered(|ui| {
            if widgets::icon_button_on(ui, &tk, icon::SIDEBAR_SIMPLE, &t("titlebar.toggleSidebar"), app.state.sidebar_open).clicked() {
                app.state.sidebar_open = !app.state.sidebar_open;
            }
            ui.add_space(4.0);
            workspace_tabs(app, ui);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !app.host.native_decorations() {
                    window_controls(app, ui);
                    ui.add_space(6.0);
                }
                if widgets::icon_button(ui, &tk, icon::GEAR_SIX, &format!("{} (Ctrl+,)", t("settings.title"))).clicked() {
                    app.state.settings_open = !app.state.settings_open;
                }
                let search = egui::Button::new(
                    RichText::new(format!("{}  {}", icon::MAGNIFYING_GLASS, t("titlebar.search")))
                        .color(tk.p.muted)
                        .size(tk.text_size - 1.0),
                )
                .fill(tk.p.input)
                .stroke(Stroke::new(1.0, tk.p.border))
                .min_size(Vec2::new(220.0, 24.0));
                if ui.add(search).on_hover_text("Ctrl+K").clicked() {
                    app.state.palette_open = true;
                }
                if !app.data.connected && app.data.connection_error.is_some() {
                    ui.label(RichText::new(format!("{} {}", icon::PLUGS, t("titlebar.disconnected"))).color(tk.p.red))
                        .on_hover_text(app.data.connection_error.clone().unwrap_or_default());
                }
            });
        });
    });
}

fn workspace_tabs(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    let ids = app.state.open_workspaces.clone();
    let mut close = None;
    let mut switch = None;
    for id in &ids {
        let Some(w) = app.data.workspace(id) else { continue };
        let active = app.data.active_workspace.as_deref() == Some(id.as_str());
        let fill = if active { tk.p.bg } else { tk.p.panel };
        let text = RichText::new(&w.name).color(if active { tk.p.text } else { tk.p.muted });
        let tab = egui::Frame::new()
            .fill(fill)
            .stroke(Stroke::new(1.0, if active { tk.p.border } else { egui::Color32::TRANSPARENT }))
            .corner_radius(egui::CornerRadius::same(tk.radius_sm))
            .inner_margin(egui::Margin::symmetric(8, 3))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let r = ui.add(egui::Label::new(text).sense(Sense::click()));
                    if r.clicked() && !active {
                        switch = Some(id.clone());
                    }
                    r.on_hover_text(widgets::short_path(&w.path));
                    if ui
                        .add(egui::Button::new(RichText::new(icon::X).size(10.0).color(tk.p.faint)).frame(false))
                        .on_hover_text(t("titlebar.closeWorkspace"))
                        .clicked()
                    {
                        close = Some(id.clone());
                    }
                });
            });
        tab.response.context_menu(|ui| {
            if ui.button(t("titlebar.editWorkspace")).clicked() {
                app.state.settings_workspace = Some(id.clone());
                app.state.settings_tab = crate::state::SettingsTab::Workspaces;
                app.state.settings_open = true;
                ui.close();
            }
            if ui.button(t("titlebar.closeWorkspace")).clicked() {
                close = Some(id.clone());
                ui.close();
            }
        });
    }
    if widgets::icon_button(ui, &tk, icon::PLUS, &t("titlebar.openWorkspaces")).clicked() {
        app.eph.picker_checked = app.state.open_workspaces.clone();
        app.state.picking_workspaces = true;
    }
    if let Some(id) = switch {
        app.switch_workspace(&id);
    }
    if let Some(id) = close {
        app.close_workspace(&id);
    }
}

fn window_controls(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    if widgets::icon_button(ui, &tk, icon::X, &t("windowControls.close")).clicked() {
        viewport(app, "close");
    }
    let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
    if widgets::icon_button(ui, &tk, if max { icon::CORNERS_IN } else { icon::SQUARE }, &t("windowControls.maximize")).clicked() {
        viewport(app, "maximize");
    }
    if widgets::icon_button(ui, &tk, icon::MINUS, &t("windowControls.minimize")).clicked() {
        viewport(app, "minimize");
    }
}

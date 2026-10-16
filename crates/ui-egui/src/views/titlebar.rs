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

pub const LOGO: &[u8] = include_bytes!("../../assets/logo-64.png");

pub fn show(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    let frame =
        egui::Frame::new().fill(tk.p.panel).inner_margin(egui::Margin { left: 6, right: 4, top: 0, bottom: 0 }).stroke(Stroke::NONE);
    egui::Panel::top("titlebar").exact_size(38.0).frame(frame).show(ui, |ui| {
        // o fundo inteiro arrasta a janela (e duplo clique maximiza)
        let bar = ui.max_rect();
        let drag = ui.interact(bar, ui.id().with("drag"), Sense::click_and_drag());
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            viewport(app, "maximize");
        }
        ui.painter().hline(bar.x_range(), bar.bottom() - 0.5, Stroke::new(1.0, tk.p.border));
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let open = app.state.sidebar_open;
            let tip = lazyaicode_i18n::tf(if open { "titlebar.hideSidebar" } else { "titlebar.showSidebar" }, &[("key", &"Ctrl+B")]);
            if bar_icon(ui, &tk, icon::SIDEBAR_SIMPLE, &tip, open).clicked() {
                app.state.sidebar_open = !open;
            }
            // a marca, como no app anterior
            ui.add(egui::Image::from_bytes("bytes://lazyaicode-logo.png", LOGO).fit_to_exact_size(Vec2::splat(18.0)));
            ui.label(RichText::new("LazyAICode").strong().color(tk.p.text));
            ui.add_space(10.0);
            workspace_tabs(app, ui);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if !app.host.native_decorations() {
                    window_controls(app, ui);
                    ui.add_space(4.0);
                }
                if bar_icon(ui, &tk, icon::GEAR_SIX, &format!("{} (Ctrl+,)", t("settings.title")), false).clicked() {
                    app.state.settings_open = !app.state.settings_open;
                }
                if bar_icon(ui, &tk, icon::MAGNIFYING_GLASS, &format!("{} (Ctrl+K)", t("palette.title")), false).clicked() {
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

/// Botão quadrado de 28 px da barra de título.
fn bar_icon(ui: &mut egui::Ui, tk: &crate::theme::Tokens, ic: &str, tip: &str, on: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    if r.hovered() {
        ui.painter().rect_filled(rect, egui::CornerRadius::same(6), tk.p.hover);
    }
    let color = if r.hovered() || on { tk.p.text } else { tk.p.muted };
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, ic, egui::FontId::proportional(15.0), color);
    if tip.is_empty() {
        r
    } else {
        r.on_hover_text(tip)
    }
}

/// `~/.claude-trabalho` → `claude-trabalho`, como a aba mostra a conta.
pub fn account_short(dir: &str) -> String {
    let d = dir.trim_end_matches(['/', '\\']);
    let name = d.rsplit(['/', '\\']).next().unwrap_or(d);
    name.trim_start_matches('.').to_string()
}

fn workspace_tabs(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    let ids = app.state.open_workspaces.clone();
    let mut close = None;
    let mut switch = None;
    for id in &ids {
        let Some(w) = app.data.workspace(id) else { continue };
        let active = app.data.active_workspace.as_deref() == Some(id.as_str()) && !app.state.picking_workspaces;
        let dir = w.claude_dir.clone().unwrap_or_else(|| app.data.default_claude_dir.clone());
        let logged = app.data.auth.get(w.claude_dir.as_deref().unwrap_or("")).and_then(|a| a["loggedIn"].as_bool());
        let dot = match logged {
            Some(true) => tk.p.accent,
            Some(false) => tk.p.red,
            None => tk.p.faint,
        };
        let font = egui::FontId::proportional(tk.text_size);
        let name_g = ui.painter().layout_no_wrap(w.name.clone(), font, if active { tk.p.text } else { tk.p.muted });
        let acct_g = ui.painter().layout_no_wrap(account_short(&dir), egui::FontId::monospace(tk.text_size - 2.5), tk.p.faint);
        let width = (10.0 + 7.0 + 7.0 + name_g.size().x + 7.0 + acct_g.size().x + 7.0 + 16.0 + 6.0).min(240.0);
        let (rect, r) = ui.allocate_exact_size(Vec2::new(width, 28.0), Sense::click());
        let hovered = r.hovered();
        let p = ui.painter_at(rect);
        if active {
            p.rect(rect, egui::CornerRadius::same(6), tk.p.active, Stroke::new(1.0, tk.p.border), egui::StrokeKind::Inside);
        } else if hovered {
            p.rect_filled(rect, egui::CornerRadius::same(6), tk.p.hover);
        }
        let mut x = rect.left() + 10.0;
        p.circle_filled(egui::pos2(x + 3.5, rect.center().y), 3.5, dot);
        x += 14.0;
        let name_w = name_g.size().x;
        p.galley(egui::pos2(x, rect.center().y - name_g.size().y / 2.0), name_g, tk.p.text);
        x += name_w + 7.0;
        p.galley(egui::pos2(x, rect.center().y - acct_g.size().y / 2.0), acct_g, tk.p.faint);
        // o X só aparece na aba ativa ou sob o mouse
        let xr = egui::Rect::from_center_size(egui::pos2(rect.right() - 14.0, rect.center().y), Vec2::splat(16.0));
        let xresp = ui.interact(xr, r.id.with("close"), Sense::click()).on_hover_text(t("titlebar.closeWorkspaceHint"));
        if active || hovered || xresp.hovered() {
            if xresp.hovered() {
                ui.painter().rect_filled(xr, egui::CornerRadius::same(4), tk.p.active);
            }
            ui.painter().text(
                xr.center(),
                egui::Align2::CENTER_CENTER,
                icon::X,
                egui::FontId::proportional(11.0),
                if xresp.hovered() { tk.p.text } else { tk.p.faint },
            );
        }
        if xresp.clicked() || r.middle_clicked() {
            close = Some(id.clone());
        } else if r.clicked() && !active {
            switch = Some(id.clone());
        }
        if r.double_clicked() {
            app.state.settings_workspace = Some(id.clone());
            app.state.settings_tab = crate::state::SettingsTab::Workspaces;
            app.state.settings_open = true;
        }
        let r = r.on_hover_text(lazyaicode_i18n::tf(
            "titlebar.accountTitle",
            &[("path", &widgets::short_path(&w.path)), ("account", &widgets::short_path(&dir))],
        ));
        r.context_menu(|ui| {
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
            ui.separator();
            if ui.button(t("titlebar.newWorkspace")).clicked() {
                app.state.new_workspace_open = true;
                ui.close();
            }
        });
    }
    if bar_icon(ui, &tk, icon::PLUS, &t("titlebar.openWorkspace"), false).clicked() {
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
    if bar_icon(ui, &tk, icon::X, &t("windowControls.close"), false).clicked() {
        viewport(app, "close");
    }
    let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
    if bar_icon(ui, &tk, if max { icon::CORNERS_IN } else { icon::SQUARE }, &t("windowControls.maximize"), false).clicked() {
        viewport(app, "maximize");
    }
    if bar_icon(ui, &tk, icon::MINUS, &t("windowControls.minimize"), false).clicked() {
        viewport(app, "minimize");
    }
}

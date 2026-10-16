//! O rodapé da sessão: projeto, branch e linhas mudadas à esquerda; MCP,
//! contexto, modo de permissão, esforço e modelo à direita.

use egui::{Align, Layout, RichText, Stroke};
use egui_phosphor::regular as icon;
use lazyaicode_doc::ThreadView;
use lazyaicode_i18n::{t, tf};
use serde_json::{json, Value};

use crate::app::LazyApp;
use crate::views::widgets;

fn label_of(key: &str) -> String {
    let s = t(key);
    if s == key {
        key.to_string()
    } else {
        s
    }
}

pub fn show(app: &mut LazyApp, ui: &mut egui::Ui, th: &ThreadView) {
    let tk = app.tokens;
    let id = th.thread.id.clone();
    let claude = th.thread.provider == "claude";
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if let Some(p) = app.data.project(&th.thread.project_id) {
            ui.label(RichText::new(format!("{} {}", icon::FOLDER_SIMPLE, p.name)).color(tk.p.text).size(tk.text_size - 1.0));
        }
        let stats = app.data.diff_stats.get(&id).cloned();
        if let Some(b) = stats.as_ref().and_then(|s| s["branch"].as_str()).filter(|b| *b != "HEAD") {
            ui.label(RichText::new(format!("/ {b}")).color(tk.p.muted).size(tk.text_size - 1.0));
        }
        if let Some(s) = &stats {
            let (a, d) = (s["additions"].as_u64().unwrap_or(0), s["deletions"].as_u64().unwrap_or(0));
            if a + d > 0 {
                let r = ui.add(
                    egui::Label::new(RichText::new(format!("+{a}")).color(tk.add_fg).size(tk.text_size - 1.0)).sense(egui::Sense::click()),
                );
                ui.label(RichText::new(format!("−{d}")).color(tk.del_fg).size(tk.text_size - 1.0));
                if r.on_hover_text(tf("footer.filesChanged", &[("n", &s["files"].as_u64().unwrap_or(0))])).clicked() {
                    let _ = crate::commands::run_ui(app, "ui.history.review", &Value::Null);
                }
            }
        }
        if th.thread.isolated {
            ui.label(RichText::new(icon::STACK).color(tk.p.faint)).on_hover_text(t("thread.isolatedWorkspace"));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            model_menu(app, ui, th);
            if claude {
                effort_menu(app, ui, th);
            }
            mode_menu(app, ui, th);
            if claude {
                let skip = th.thread.skip_permissions;
                if widgets::icon_button_on(
                    ui,
                    &tk,
                    if skip { icon::SHIELD_SLASH } else { icon::SHIELD_CHECK },
                    &t(if skip { "footer.skipOn" } else { "footer.skipOff" }),
                    skip,
                )
                .clicked()
                {
                    let _ = crate::commands::run_ui(app, "ui.thread.skip-permissions.toggle", &Value::Null);
                }
            }
            context_meter(app, ui, &id);
            mcp_chip(app, ui, th);
        });
    });
}

fn menu_button(ui: &mut egui::Ui, tk: &crate::theme::Tokens, text: String, tip: &str) -> egui::Response {
    let b = egui::Button::new(RichText::new(text).size(tk.text_size - 1.0).color(tk.p.muted)).frame(false);
    let r = ui.add(b);
    if r.hovered() {
        ui.painter().rect_stroke(r.rect, egui::CornerRadius::same(tk.radius_sm), Stroke::new(1.0, tk.p.border), egui::StrokeKind::Inside);
    }
    r.on_hover_text(tip)
}

fn set(app: &mut LazyApp, cmd: &str, p: Value) {
    app.call_ok(cmd, p, |app, v| {
        if let Ok(t) = serde_json::from_value(v) {
            app.data.upsert(t);
        }
    });
}

fn model_menu(app: &mut LazyApp, ui: &mut egui::Ui, th: &ThreadView) {
    let tk = app.tokens;
    let id = th.thread.id.clone();
    let provider = th.thread.provider.clone();
    let acp = app.data.acp_session.get(&id).cloned();
    let caps_models: Vec<Value> = {
        let path = app.data.project(&th.thread.project_id).map(|p| p.path.clone());
        let key = app.data.caps_key(path.as_deref());
        app.data.caps.get(&key).and_then(|c| c["models"].as_array().cloned()).unwrap_or_default()
    };
    let current = if provider == "claude" {
        lazyaicode_doc::MODELS.iter().find(|m| m.0 == th.thread.model).map(|m| label_of(m.1)).unwrap_or_else(|| th.thread.model.clone())
    } else {
        let cur = acp.as_ref().and_then(|a| a["currentModel"].as_str().map(str::to_string)).unwrap_or_else(|| th.thread.model.clone());
        let name = acp
            .as_ref()
            .and_then(|a| a["models"].as_array())
            .and_then(|ms| ms.iter().find(|m| m["id"] == cur.as_str()))
            .and_then(|m| m["name"].as_str().map(str::to_string));
        name.unwrap_or(if cur.is_empty() { t("footer.agentDefault") } else { cur })
    };
    let pshort = lazyaicode_doc::PROVIDERS.iter().find(|p| p.0 == provider).map_or("", |p| p.1);
    let r = menu_button(ui, &tk, format!("{} {current}", icon::CPU), &format!("{pshort} · {}", t("footer.model")));
    egui::Popup::menu(&r).show(|ui| {
        ui.set_min_width(220.0);
        if th.thread.session_id.is_none() {
            ui.label(widgets::faint(&tk, t("footer.provider")));
            for (p, label, vendor) in lazyaicode_doc::PROVIDERS {
                if ui.selectable_label(provider == p, format!("{label}  ·  {vendor}")).clicked() {
                    set(app, "thread.set-provider", json!({ "id": id, "provider": p }));
                }
            }
            ui.separator();
        }
        ui.label(widgets::faint(&tk, t("footer.model")));
        if provider == "claude" {
            for (m, label) in lazyaicode_doc::MODELS {
                if ui.selectable_label(th.thread.model == m, label_of(label)).clicked() {
                    set(app, "thread.set-model", json!({ "id": id, "model": m }));
                }
            }
            for m in &caps_models {
                let v = m["value"].as_str().unwrap_or("");
                if v.is_empty() || lazyaicode_doc::MODELS.iter().any(|x| x.0 == v) {
                    continue;
                }
                let name = m["displayName"].as_str().unwrap_or(v);
                if ui.selectable_label(th.thread.model == v, name).on_hover_text(m["description"].as_str().unwrap_or("")).clicked() {
                    set(app, "thread.set-model", json!({ "id": id, "model": v }));
                }
            }
        } else {
            let models = acp.as_ref().and_then(|a| a["models"].as_array().cloned()).unwrap_or_default();
            if models.is_empty() {
                ui.label(widgets::faint(&tk, t("footer.modelsAfterFirstMessage")));
            }
            for m in models {
                let mid = m["id"].as_str().unwrap_or("").to_string();
                let cur = acp.as_ref().and_then(|a| a["currentModel"].as_str()) == Some(mid.as_str());
                if ui.selectable_label(cur, m["name"].as_str().unwrap_or(&mid)).clicked() {
                    set(app, "thread.set-model", json!({ "id": id, "model": mid }));
                }
            }
        }
    });
}

fn effort_menu(app: &mut LazyApp, ui: &mut egui::Ui, th: &ThreadView) {
    let tk = app.tokens;
    let id = th.thread.id.clone();
    let cur = th.thread.effort.clone().unwrap_or_default();
    let label = lazyaicode_doc::EFFORTS.iter().find(|e| e.0 == cur).map(|e| label_of(e.1)).unwrap_or(cur.clone());
    let r = menu_button(ui, &tk, format!("{} {label}", icon::LIGHTBULB), &t("footer.effort"));
    egui::Popup::menu(&r).show(|ui| {
        for (e, l) in lazyaicode_doc::EFFORTS {
            if ui.selectable_label(cur == e, label_of(l)).clicked() {
                set(app, "thread.set-options", json!({ "id": id, "effort": e }));
            }
        }
    });
}

fn mode_menu(app: &mut LazyApp, ui: &mut egui::Ui, th: &ThreadView) {
    let tk = app.tokens;
    let id = th.thread.id.clone();
    if th.thread.provider != "claude" {
        let acp = app.data.acp_session.get(&id).cloned();
        let modes = acp.as_ref().and_then(|a| a["modes"].as_array().cloned()).unwrap_or_default();
        if modes.is_empty() {
            return;
        }
        let cur = acp.as_ref().and_then(|a| a["currentMode"].as_str().map(str::to_string)).unwrap_or_default();
        let name = modes.iter().find(|m| m["id"] == cur.as_str()).and_then(|m| m["name"].as_str()).unwrap_or(&cur).to_string();
        let r = menu_button(ui, &tk, format!("{} {name}", icon::HAND), &t("footer.mode"));
        egui::Popup::menu(&r).show(|ui| {
            for m in &modes {
                let mid = m["id"].as_str().unwrap_or("").to_string();
                if ui
                    .selectable_label(cur == mid, m["name"].as_str().unwrap_or(&mid))
                    .on_hover_text(m["description"].as_str().unwrap_or(""))
                    .clicked()
                {
                    set(app, "thread.set-permission-mode", json!({ "id": id, "mode": mid }));
                }
            }
        });
        return;
    }
    let cur = th.thread.permission_mode.clone();
    let label = lazyaicode_doc::PERMISSION_MODES.iter().find(|m| m.0 == cur).map(|m| label_of(m.1)).unwrap_or(cur.clone());
    let color_icon = if cur == "bypassPermissions" {
        icon::WARNING
    } else if cur == "plan" {
        icon::LIST_CHECKS
    } else {
        icon::HAND
    };
    let r = menu_button(ui, &tk, format!("{color_icon} {label}"), &format!("{} (Shift+Tab)", t("footer.mode")));
    egui::Popup::menu(&r).show(|ui| {
        for (m, l) in lazyaicode_doc::PERMISSION_MODES {
            if ui.selectable_label(cur == m, label_of(l)).clicked() {
                set(app, "thread.set-permission-mode", json!({ "id": id, "mode": m }));
            }
        }
    });
}

fn context_meter(app: &mut LazyApp, ui: &mut egui::Ui, id: &str) {
    let tk = app.tokens;
    let Some(c) = app.data.context.get(id).cloned() else { return };
    let frac = (c.used as f32 / c.window.max(1) as f32).clamp(0.0, 1.0);
    let color = if frac > 0.85 {
        tk.p.red
    } else if frac > 0.6 {
        tk.p.amber
    } else {
        tk.p.muted
    };
    let (rect, r) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
    let p = ui.painter();
    p.circle_stroke(rect.center(), 6.0, Stroke::new(2.0, tk.p.border));
    let n = 24;
    let pts: Vec<egui::Pos2> = (0..=n)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + (i as f32 / n as f32) * frac * std::f32::consts::TAU;
            rect.center() + egui::Vec2::angled(a) * 6.0
        })
        .collect();
    if frac > 0.0 {
        p.add(egui::Shape::line(pts, Stroke::new(2.0, color)));
    }
    r.on_hover_text(tf(
        "footer.context",
        &[
            ("used", &lazyaicode_doc::usage::fmt_tokens(c.used)),
            ("window", &lazyaicode_doc::usage::fmt_tokens(c.window)),
            ("pct", &((frac * 100.0).round() as u32)),
        ],
    ));
}

fn mcp_chip(app: &mut LazyApp, ui: &mut egui::Ui, th: &ThreadView) {
    let tk = app.tokens;
    let id = th.thread.id.clone();
    let Some(servers) = app.data.thread_mcp.get(&id).and_then(|v| v.as_array().cloned()) else { return };
    if servers.is_empty() {
        return;
    }
    let ok = servers.iter().filter(|s| s["status"] == "connected").count();
    let failed = servers.iter().any(|s| matches!(s["status"].as_str(), Some("failed" | "needs-auth")));
    let color = if failed { tk.p.amber } else { tk.p.muted };
    let r = ui.add(
        egui::Button::new(RichText::new(format!("{} {ok}/{}", icon::PLUGS_CONNECTED, servers.len())).size(tk.text_size - 1.0).color(color))
            .frame(false),
    );
    egui::Popup::menu(&r).show(|ui| {
        ui.set_min_width(260.0);
        ui.label(widgets::faint(&tk, t("mcpChip.title")));
        for s in &servers {
            let name = s["name"].as_str().unwrap_or("").to_string();
            let status = s["status"].as_str().unwrap_or("");
            let c = match status {
                "connected" => tk.p.accent,
                "failed" | "needs-auth" => tk.p.red,
                "disabled" => tk.p.faint,
                _ => tk.p.amber,
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new("●").color(c));
                ui.label(&name);
                ui.label(widgets::faint(&tk, status));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button(icon::ARROWS_CLOCKWISE).on_hover_text(t("mcpChip.reconnect")).clicked() {
                        mcp_action(app, &id, "reconnect", &name, None);
                    }
                    let on = status != "disabled";
                    if ui.small_button(if on { icon::TOGGLE_RIGHT } else { icon::TOGGLE_LEFT }).on_hover_text(t("mcpChip.toggle")).clicked()
                    {
                        mcp_action(app, &id, "toggle", &name, Some(!on));
                    }
                });
            });
        }
    });
}

fn mcp_action(app: &mut LazyApp, id: &str, action: &str, server: &str, enabled: Option<bool>) {
    let tid = id.to_string();
    app.call_ok("thread.mcp", json!({ "id": id, "action": action, "server": server, "enabled": enabled }), move |app, v| {
        app.data.thread_mcp.insert(tid, v);
    });
}

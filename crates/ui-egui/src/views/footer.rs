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
    let small = tk.text_size - 1.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if let Some(p) = app.data.project(&th.thread.project_id) {
            let r = seg(ui, &tk, RichText::new(format!("{}  {}", icon::STACK, p.name)).size(small).color(tk.p.text).strong());
            if r.on_hover_text(&th.thread.cwd).clicked() {
                app.fire("system.open-path", json!({ "path": th.thread.cwd }));
            }
        }
        let stats = app.data.diff_stats.get(&id).cloned();
        let branch = stats.as_ref().and_then(|s| s["branch"].as_str().map(str::to_string)).or_else(|| th.thread.branch.clone());
        if let Some(b) = branch.filter(|b| !b.is_empty() && b != "HEAD") {
            ui.label(RichText::new("/").size(small).color(tk.p.faint));
            ui.label(RichText::new(b).monospace().size(small - 0.5).color(tk.p.muted)).on_hover_text("Branch");
        }
        if let Some(s) = &stats {
            let (a, d) = (s["additions"].as_u64().unwrap_or(0), s["deletions"].as_u64().unwrap_or(0));
            if a + d > 0 {
                let files = s["files"].as_u64().unwrap_or(0);
                let mut job = egui::text::LayoutJob::default();
                let fmt = |c| egui::TextFormat { font_id: egui::FontId::proportional(small), color: c, ..Default::default() };
                job.append(&format!("+{}", group_digits(a)), 0.0, fmt(tk.add_fg));
                job.append(&format!(" −{}", group_digits(d)), 0.0, fmt(tk.del_fg));
                let r = seg(ui, &tk, job);
                let tip = tf(if files == 1 { "footer.diffTitleOne" } else { "footer.diffTitleOther" }, &[("n", &files)]);
                if r.on_hover_text(tip).clicked() {
                    let _ = crate::commands::run_ui(app, "ui.history.review", &Value::Null);
                }
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            send_button(app, ui, th);
            model_menu(app, ui, th);
            if claude {
                effort_menu(app, ui, th);
            }
            mode_menu(app, ui, th);
            mcp_chip(app, ui, th);
            context_meter(app, ui, &id);
            if widgets::icon_button(ui, &tk, icon::PAPERCLIP, &format!("{} (Ctrl+Shift+I)", t("thread.attach"))).clicked() {
                app.host.pick_images("ui.composer.attach-paths", json!({ "id": id }));
            }
        });
    });
}

/// `12345` → `12,345`, como o rodapé antigo.
fn group_digits(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Um pedaço clicável do rodapé (fundo só sob o mouse).
fn seg(ui: &mut egui::Ui, tk: &crate::theme::Tokens, text: impl Into<egui::WidgetText>) -> egui::Response {
    let r = ui.add(egui::Button::new(text).frame(false).min_size(egui::vec2(0.0, 24.0)));
    if r.hovered() {
        ui.painter().rect_filled(
            r.rect.expand2(egui::vec2(3.0, 0.0)),
            egui::CornerRadius::same(tk.radius_sm),
            tk.p.hover.gamma_multiply(0.7),
        );
    }
    r
}

/// Enviar (ou parar o agente), no canto do rodapé como no app anterior.
fn send_button(app: &mut LazyApp, ui: &mut egui::Ui, th: &ThreadView) {
    let tk = app.tokens;
    let id = th.thread.id.clone();
    let busy = matches!(th.status, lazyaicode_doc::Status::Running | lazyaicode_doc::Status::Awaiting);
    let (rect, r) = ui.allocate_exact_size(egui::vec2(26.0, 24.0), egui::Sense::click());
    if busy {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(tk.radius_sm),
            tk.p.red.gamma_multiply(if r.hovered() { 1.0 } else { 0.85 }),
        );
        ui.painter().rect_filled(
            egui::Rect::from_center_size(rect.center(), egui::vec2(9.0, 9.0)),
            egui::CornerRadius::same(2),
            egui::Color32::WHITE,
        );
        if r.on_hover_text(t("footer.stop")).clicked() {
            app.fire("thread.interrupt", json!({ "id": id }));
        }
        return;
    }
    let draft = app.data.drafts.get(&id).is_some_and(|d| !d.trim().is_empty());
    let atts = app.data.attachments.get(&id).is_some_and(|a| !a.is_empty());
    let can = (draft || atts) && !th.thread.archived;
    let (bg, fg) = if can { (tk.p.accent, tk.on_accent) } else { (tk.p.elev, tk.p.faint) };
    ui.painter().rect_filled(rect, egui::CornerRadius::same(tk.radius_sm), if can && r.hovered() { bg.gamma_multiply(0.9) } else { bg });
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, icon::PAPER_PLANE_RIGHT, egui::FontId::proportional(14.0), fg);
    let with_mod = app.data.settings.as_ref().is_some_and(|s| s.send_with_modifier);
    let tip = if with_mod { t("thread.sendHintMod") } else { t("footer.send") };
    if r.on_hover_text(tip).clicked() && can {
        app.eph.send_now = true;
    }
}

fn menu_button(ui: &mut egui::Ui, tk: &crate::theme::Tokens, text: String, tip: &str) -> egui::Response {
    let r = seg(ui, tk, RichText::new(text).size(tk.text_size - 1.0).color(tk.p.muted));
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
        let from_caps = |v: &str| caps_models.iter().find(|m| m["value"] == v).cloned();
        if th.thread.model == "default" {
            // o modelo padrão da conta, como o `claude` descreve ("Opus 5.5 · …")
            from_caps("default")
                .and_then(|m| m["description"].as_str().and_then(|d| d.split('·').next()).map(|d| d.trim().to_string()))
                .filter(|d| !d.is_empty())
                .unwrap_or_else(|| t("footer.modelDefault"))
        } else {
            from_caps(&th.thread.model)
                .and_then(|m| m["displayName"].as_str().map(str::to_string))
                .or_else(|| lazyaicode_doc::MODELS.iter().find(|m| m.0 == th.thread.model).map(|m| label_of(m.1)))
                .unwrap_or_else(|| th.thread.model.clone())
        }
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
    let r = menu_button(ui, &tk, format!("{}  {current}", icon::SPARKLE), &format!("{pshort} · {}", t("footer.model")));
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
    let r = menu_button(ui, &tk, format!("{}  {label}", icon::BRAIN), &t("footer.effort"));
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
        let r = menu_button(ui, &tk, format!("{}  {name}", icon::SHIELD), &t("footer.mode"));
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
    let skip = th.thread.skip_permissions;
    // o padrão mostra só o escudo; sem permissões é o último "modo", em vermelho
    let label = if skip {
        t("footer.noPermissions")
    } else if cur == "default" {
        String::new()
    } else {
        lazyaicode_doc::PERMISSION_MODES.iter().find(|m| m.0 == cur).map(|m| label_of(m.1)).unwrap_or(cur.clone())
    };
    let ic = if skip { icon::WARNING } else { icon::SHIELD };
    let text = if label.is_empty() { ic.to_string() } else { format!("{ic}  {label}") };
    let color = if skip { tk.p.red } else { tk.p.muted };
    let r = seg(ui, &tk, RichText::new(text).size(tk.text_size - 1.0).color(color));
    let r = r.on_hover_text(format!("{} (Shift+Tab)", t("footer.permissions")));
    egui::Popup::menu(&r).show(|ui| {
        ui.set_min_width(240.0);
        ui.label(widgets::faint(&tk, t("footer.groupMode")));
        for (m, l) in lazyaicode_doc::PERMISSION_MODES {
            if ui.selectable_label(!skip && cur == m, label_of(l)).clicked() {
                if skip {
                    let _ = crate::commands::run_ui(app, "ui.thread.skip-permissions.toggle", &Value::Null);
                }
                set(app, "thread.set-permission-mode", json!({ "id": id, "mode": m }));
            }
        }
        ui.separator();
        ui.label(widgets::faint(&tk, t("footer.groupNoConfirm")));
        let r = ui.selectable_label(skip, RichText::new(format!("{}  {}", icon::WARNING, t("footer.skipAll"))).color(tk.p.red));
        if r.on_hover_text("--dangerously-skip-permissions").clicked() && !skip {
            let _ = crate::commands::run_ui(app, "ui.thread.skip-permissions.toggle", &Value::Null);
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
                ui.label(RichText::new(icon::CIRCLE).color(c));
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

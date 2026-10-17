//! Configurações (Ctrl+,): workspaces, aparência, padrões das threads,
//! contas Claude, provedores, skills/agentes/MCP, git, integrações, atalhos.

use egui::{Align, Color32, Layout, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular as icon;
use lazyaicode_i18n::{t, tf};
use serde_json::{json, Value};

use crate::app::{LazyApp, ToastKind};
use crate::state::{SettingsTab, SETTINGS_TABS};
use crate::theme::{self, ContentWidth, Density, THEMES};
use crate::views::widgets;

fn tab_icon(t: SettingsTab) -> &'static str {
    match t {
        SettingsTab::Workspaces => icon::BRIEFCASE,
        SettingsTab::Appearance => icon::PALETTE,
        SettingsTab::Defaults => icon::SLIDERS,
        SettingsTab::Accounts => icon::USER_CIRCLE,
        SettingsTab::Providers => icon::ROBOT,
        SettingsTab::Skills => icon::LIGHTNING,
        SettingsTab::Agents => icon::USERS_THREE,
        SettingsTab::Mcp => icon::PLUGS,
        SettingsTab::Git => icon::GIT_BRANCH,
        SettingsTab::Integrations => icon::LINK,
        SettingsTab::Shortcuts => icon::KEYBOARD,
        SettingsTab::About => icon::INFO,
    }
}

pub fn show(app: &mut LazyApp, ctx: &egui::Context) {
    if !app.state.settings_open {
        return;
    }
    let tk = app.tokens;
    let screen = ctx.content_rect();
    let size = Vec2::new((screen.width() - 80.0).clamp(400.0, 1040.0), (screen.height() - 80.0).clamp(300.0, 720.0));
    let mut open = true;
    let modal = egui::Modal::new(egui::Id::new("settings"))
        .frame(
            egui::Frame::new()
                .fill(tk.p.bg)
                .stroke(Stroke::new(1.0, tk.p.border_strong))
                .corner_radius(egui::CornerRadius::same(tk.radius + 2))
                .inner_margin(egui::Margin::ZERO),
        )
        .show(ctx, |ui| {
            ui.set_min_size(size);
            ui.set_max_size(size);
            egui::Panel::left("settings-tabs")
                .exact_size(200.0)
                .resizable(false)
                .frame(egui::Frame::new().fill(tk.p.panel).inner_margin(egui::Margin::same(8)))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t("settings.title")).strong().size(tk.text_size + 2.0));
                    });
                    ui.add_space(8.0);
                    for tab in SETTINGS_TABS {
                        let on = app.state.settings_tab == tab;
                        let r =
                            ui.add_sized(
                                [ui.available_width(), tk.row_h],
                                egui::Button::selectable(
                                    on,
                                    RichText::new(format!("{}  {}", tab_icon(tab), t(&format!("settings.tab.{}", tab.id()))))
                                        .color(if on { tk.p.text } else { tk.p.muted }),
                                ),
                            );
                        if r.clicked() {
                            app.state.settings_tab = tab;
                        }
                    }
                });
            egui::CentralPanel::default().frame(egui::Frame::new().fill(tk.p.bg).inner_margin(egui::Margin::same(18))).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(t(&format!("settings.tab.{}", app.state.settings_tab.id())));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::icon_button(ui, &tk, icon::X, &t("app.close")).clicked() {
                            open = false;
                        }
                    });
                });
                ui.add_space(8.0);
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match app.state.settings_tab {
                    SettingsTab::Workspaces => workspaces(app, ui),
                    SettingsTab::Appearance => appearance(app, ui),
                    SettingsTab::Defaults => defaults(app, ui),
                    SettingsTab::Accounts => accounts(app, ui),
                    SettingsTab::Providers => providers(app, ui),
                    SettingsTab::Skills => capabilities(app, ui, "commands", "skill"),
                    SettingsTab::Agents => capabilities(app, ui, "agents", "agent"),
                    SettingsTab::Mcp => mcp(app, ui),
                    SettingsTab::Git => git(app, ui),
                    SettingsTab::Integrations => integrations(app, ui),
                    SettingsTab::Shortcuts => shortcuts(app, ui),
                    SettingsTab::About => about(app, ui),
                });
            });
        });
    if modal.should_close() || !open {
        app.state.settings_open = false;
    }
}

fn row(ui: &mut egui::Ui, tk: &theme::Tokens, label: &str, hint: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(260.0);
            ui.label(RichText::new(label).color(tk.p.text));
            if !hint.is_empty() {
                ui.label(RichText::new(hint).size(tk.text_size - 2.0).color(tk.p.faint));
            }
        });
        body(ui);
    });
    ui.add_space(10.0);
}

fn save_settings(app: &mut LazyApp) {
    if let Some(s) = app.data.settings.clone() {
        app.fire("settings.save", json!({ "settings": s }));
    }
}

fn set_pref(app: &mut LazyApp, key: &str, value: Value) {
    app.data.prefs.insert(key.into(), value.clone());
    app.fire("prefs.set", json!({ "key": key, "value": value }));
}

// ---------- workspaces ----------

fn workspaces(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    let list = app.data.workspaces.clone();
    let sel =
        app.state.settings_workspace.clone().or_else(|| app.data.active_workspace.clone()).or_else(|| list.first().map(|w| w.id.clone()));
    ui.horizontal_wrapped(|ui| {
        for w in &list {
            if ui.selectable_label(sel.as_deref() == Some(w.id.as_str()), &w.name).clicked() {
                app.state.settings_workspace = Some(w.id.clone());
                app.eph.settings_ws_name = w.name.clone();
            }
        }
        if ui.button(format!("{} {}", icon::PLUS, t("newWorkspace.title"))).clicked() {
            app.state.new_workspace_open = true;
        }
    });
    ui.add_space(10.0);
    let Some(w) = sel.and_then(|id| app.data.workspace(&id).cloned()) else {
        ui.label(widgets::faint(&tk, t("settings.noWorkspaces")));
        return;
    };
    if app.eph.settings_ws_name.is_empty() || app.state.settings_workspace.as_deref() != Some(w.id.as_str()) {
        app.eph.settings_ws_name = w.name.clone();
        app.state.settings_workspace = Some(w.id.clone());
    }
    row(ui, &tk, &t("settings.wsName"), "", |ui| {
        let r = ui.add(egui::TextEdit::singleline(&mut app.eph.settings_ws_name).desired_width(280.0));
        if r.lost_focus() && app.eph.settings_ws_name.trim() != w.name {
            let name = app.eph.settings_ws_name.trim().to_string();
            app.call_ok("workspace.update", json!({ "id": w.id, "name": name }), |app, _| app.refresh_workspaces(false));
        }
    });
    row(ui, &tk, &t("settings.wsFolder"), "", |ui| {
        ui.label(RichText::new(widgets::short_path(&w.path)).monospace());
        if ui.small_button(icon::FOLDER_OPEN).clicked() {
            app.fire("system.open-path", json!({ "path": w.path }));
        }
    });
    let accounts = app.data.accounts.clone();
    let cur = w.claude_dir.clone().unwrap_or_default();
    row(ui, &tk, &t("settings.wsAccount"), &t("settings.wsAccountHint"), |ui| {
        let shown = if cur.is_empty() { widgets::short_path(&app.data.default_claude_dir) } else { widgets::short_path(&cur) };
        egui::ComboBox::from_id_salt("ws-account").selected_text(shown).width(300.0).show_ui(ui, |ui| {
            for a in &accounts {
                let path = a["path"].as_str().unwrap_or("").to_string();
                let default = a["is_default"].as_bool().unwrap_or(false);
                let label = format!("{}{}", widgets::short_path(&path), if a["has_credentials"] == true { "" } else { "  (sem login)" });
                let is = if default { cur.is_empty() } else { cur == path };
                if ui.selectable_label(is, label).clicked() && !is {
                    let dir = if default { String::new() } else { path };
                    app.call_ok("workspace.update", json!({ "id": w.id, "claudeDir": dir }), |app, _| {
                        app.toast(t("settings.accountChanged"), ToastKind::Ok);
                        app.refresh_workspaces(false);
                    });
                }
            }
        });
        let auth = app.data.auth.get(&cur).cloned();
        match auth {
            Some(a) if a["loggedIn"] == true => {
                ui.label(RichText::new(format!("{} {}", icon::CHECK_CIRCLE, a["email"].as_str().unwrap_or(""))).color(tk.p.accent));
            }
            Some(_) => {
                ui.label(RichText::new(format!("{} {}", icon::WARNING_CIRCLE, t("settings.notLoggedIn"))).color(tk.p.red));
            }
            None => {
                app.refresh_auth(w.claude_dir.clone());
            }
        }
    });
    widgets::section(ui, &tk, &t("settings.wsFolders"));
    ui.label(widgets::faint(&tk, t("settings.wsFoldersHint")));
    let repos = app.data.workspace_repos.get(&w.id).cloned();
    match repos {
        None => {
            let wid = w.id.clone();
            app.data.workspace_repos.insert(w.id.clone(), vec![]);
            app.call_ok("workspace.repos", json!({ "id": w.id }), move |app, v| {
                app.data.workspace_repos.insert(wid, v.as_array().cloned().unwrap_or_default());
            });
        }
        Some(repos) => {
            for r in repos {
                let path = r["path"].as_str().unwrap_or("").to_string();
                let mut shown = !w.ignored_paths.iter().any(|i| path.starts_with(i.as_str()));
                let label = format!(
                    "{} {}",
                    if r["is_repo"] == true { icon::GIT_BRANCH } else { icon::FOLDER_SIMPLE },
                    r["name"].as_str().unwrap_or("")
                );
                if ui.checkbox(&mut shown, label).changed() {
                    let mut ignored = w.ignored_paths.clone();
                    if shown {
                        ignored.retain(|i| *i != path);
                    } else {
                        ignored.push(path.clone());
                    }
                    app.call_ok("workspace.update", json!({ "id": w.id, "ignoredPaths": ignored }), |app, _| {
                        app.refresh_workspaces(false);
                        app.refresh_all();
                    });
                }
            }
        }
    }
    ui.add_space(16.0);
    if widgets::danger(ui, &tk, &format!("{}  {}", icon::TRASH, t("settings.wsRemove"))).clicked() {
        app.eph.confirm = Some((tf("settings.wsRemoveConfirm", &[("name", &w.name)]), "workspace.remove".into(), json!({ "id": w.id })));
        app.state.dialog = Some("confirm".into());
    }
}

// ---------- aparência ----------

fn swatch(ui: &mut egui::Ui, tk: &theme::Tokens, def: &theme::ThemeDef, on: bool) -> egui::Response {
    let p = def.palette();
    let (rect, r) = ui.allocate_exact_size(Vec2::new(150.0, 70.0), Sense::click());
    let painter = ui.painter();
    painter.rect(
        rect,
        egui::CornerRadius::same(tk.radius),
        p.bg,
        Stroke::new(if on { 2.0 } else { 1.0 }, if on { tk.p.accent } else { tk.p.border }),
        egui::StrokeKind::Inside,
    );
    let side = egui::Rect::from_min_size(rect.min + Vec2::new(6.0, 6.0), Vec2::new(36.0, 40.0));
    painter.rect_filled(side, 3.0, p.panel);
    for (i, c) in [p.accent, p.blue, p.amber, p.red].iter().enumerate() {
        painter.circle_filled(rect.min + Vec2::new(54.0 + i as f32 * 14.0, 16.0), 5.0, *c);
    }
    painter.rect_filled(egui::Rect::from_min_size(rect.min + Vec2::new(50.0, 30.0), Vec2::new(80.0, 5.0)), 2.0, p.text);
    painter.rect_filled(egui::Rect::from_min_size(rect.min + Vec2::new(50.0, 40.0), Vec2::new(56.0, 5.0)), 2.0, p.muted);
    let name = {
        let l = t(def.label);
        if l == def.label && def.label.contains('.') {
            def.id.to_string()
        } else {
            l
        }
    };
    painter.text(rect.left_bottom() + Vec2::new(8.0, -10.0), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(11.0), p.text);
    r
}

fn appearance(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    widgets::section(ui, &tk, &t("appearance.theme"));
    ui.horizontal(|ui| {
        let sys = app.state.appearance.theme == "system";
        if ui.selectable_label(sys, format!("{}  {}", icon::CIRCLE_HALF, t("appearance.themeSystem"))).clicked() {
            app.state.appearance.theme = "system".into();
        }
    });
    ui.horizontal_wrapped(|ui| {
        for def in THEMES {
            let on = app.state.appearance.theme == def.id;
            if swatch(ui, &tk, def, on).clicked() {
                app.state.appearance.theme = def.id.into();
            }
        }
    });
    ui.add_space(14.0);
    let a = &mut app.state.appearance;
    row(ui, &tk, &t("appearance.density"), "", |ui| {
        for (d, k) in [
            (Density::Compact, "appearance.densityCompact"),
            (Density::Normal, "appearance.densityNormal"),
            (Density::Comfortable, "appearance.densityComfortable"),
        ] {
            ui.selectable_value(&mut a.density, d, t(k));
        }
    });
    row(ui, &tk, &t("appearance.contentWidth"), "", |ui| {
        for (w, k) in [
            (ContentWidth::Narrow, "appearance.widthNarrow"),
            (ContentWidth::Normal, "appearance.widthNormal"),
            (ContentWidth::Wide, "appearance.widthWide"),
            (ContentWidth::Full, "appearance.widthFull"),
        ] {
            ui.selectable_value(&mut a.content_width, w, t(k));
        }
    });
    row(ui, &tk, &t("appearance.scale"), "Ctrl + / Ctrl − / Ctrl 0", |ui| {
        ui.add(egui::Slider::new(&mut a.scale, 50..=250).suffix("%"));
    });
    row(ui, &tk, &t("appearance.textSize"), "", |ui| {
        ui.add(egui::Slider::new(&mut a.text_size, 10.0..=20.0).step_by(0.5).suffix(" px"));
    });
    row(ui, &tk, &t("appearance.codeSize"), "", |ui| {
        ui.add(egui::Slider::new(&mut a.code_size, 10.0..=20.0).step_by(0.5).suffix(" px"));
    });
    row(ui, &tk, &t("appearance.terminalSize"), "", |ui| {
        ui.add(egui::Slider::new(&mut a.terminal_size, 9.0..=24.0).step_by(0.5).suffix(" px"));
    });
    let cur = lazyaicode_i18n::locale();
    row(ui, &tk, &t("settings.language"), "", |ui| {
        for (id, name, _) in lazyaicode_i18n::LOCALES {
            if ui.selectable_label(cur == id, name).clicked() {
                let _ = crate::commands::run_ui(app, &format!("ui.language.{id}"), &Value::Null);
            }
        }
    });
    if ui.button(t("appearance.reset")).clicked() {
        app.state.appearance = Default::default();
    }
}

// ---------- padrões ----------

fn defaults(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    let Some(mut s) = app.data.settings.clone() else {
        ui.spinner();
        return;
    };
    let before = s.clone();
    let label = |k: &str| {
        let l = t(k);
        if l == k {
            k.to_string()
        } else {
            l
        }
    };
    row(ui, &tk, &t("settings.defaultProvider"), "", |ui| {
        for (p, l, v) in lazyaicode_doc::PROVIDERS {
            ui.selectable_value(&mut s.default_provider, p.to_string(), format!("{l} · {v}"));
        }
    });
    row(ui, &tk, &t("settings.defaultModel"), &t("settings.defaultModelHint"), |ui| {
        egui::ComboBox::from_id_salt("def-model")
            .selected_text(label(lazyaicode_doc::MODELS.iter().find(|m| m.0 == s.default_model).map_or(s.default_model.as_str(), |m| m.1)))
            .show_ui(ui, |ui| {
                for (m, l) in lazyaicode_doc::MODELS {
                    ui.selectable_value(&mut s.default_model, m.to_string(), label(l));
                }
            });
    });
    row(ui, &tk, &t("settings.defaultMode"), "", |ui| {
        egui::ComboBox::from_id_salt("def-mode")
            .selected_text(label(
                lazyaicode_doc::PERMISSION_MODES
                    .iter()
                    .find(|m| m.0 == s.default_permission_mode)
                    .map_or(s.default_permission_mode.as_str(), |m| m.1),
            ))
            .show_ui(ui, |ui| {
                for (m, l) in lazyaicode_doc::PERMISSION_MODES {
                    ui.selectable_value(&mut s.default_permission_mode, m.to_string(), label(l));
                }
            });
    });
    row(ui, &tk, &t("settings.skipPermissions"), &t("settings.skipPermissionsHint"), |ui| {
        ui.checkbox(&mut s.default_skip_permissions, "");
    });
    row(ui, &tk, &t("settings.sendWithModifier"), &t("settings.sendWithModifierHint"), |ui| {
        ui.checkbox(&mut s.send_with_modifier, "");
    });
    row(ui, &tk, &t("settings.notify"), "", |ui| {
        ui.checkbox(&mut s.notify_on_finish, "");
    });
    row(ui, &tk, &t("settings.sound"), "", |ui| {
        ui.checkbox(&mut s.sound_on_finish, "");
    });
    if s != before {
        app.data.settings = Some(s);
        save_settings(app);
    }
}

// ---------- contas ----------

fn accounts(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    ui.label(widgets::muted(&tk, t("account.hint")));
    ui.add_space(8.0);
    if ui.button(format!("{} {}", icon::ARROWS_CLOCKWISE, t("account.refresh"))).clicked() {
        app.call_ok("account.list", Value::Null, |app, v| app.data.accounts = v.as_array().cloned().unwrap_or_default());
        for a in app.data.accounts.clone() {
            let p = a["path"].as_str().map(str::to_string).filter(|_| a["is_default"] != true);
            app.refresh_auth(p);
        }
    }
    for a in app.data.accounts.clone() {
        let path = a["path"].as_str().unwrap_or("").to_string();
        let key = if a["is_default"] == true { String::new() } else { path.clone() };
        let auth = app.data.auth.get(&key).cloned();
        if auth.is_none() {
            app.refresh_auth(Some(key.clone()).filter(|k| !k.is_empty()));
        }
        egui::Frame::new()
            .fill(tk.p.panel)
            .stroke(Stroke::new(1.0, tk.p.border))
            .corner_radius(egui::CornerRadius::same(tk.radius))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(widgets::short_path(&path)).monospace().strong());
                    if a["is_default"] == true {
                        widgets::chip(ui, &tk, &t("account.default"), tk.p.muted);
                    }
                });
                match auth {
                    Some(v) if v["loggedIn"] == true => {
                        ui.label(
                            RichText::new(format!(
                                "{} {}  ·  {}",
                                icon::CHECK_CIRCLE,
                                v["email"].as_str().unwrap_or(""),
                                v["subscriptionType"].as_str().or_else(|| v["authMethod"].as_str()).unwrap_or("")
                            ))
                            .color(tk.p.accent),
                        );
                    }
                    Some(v) => {
                        ui.label(
                            RichText::new(format!(
                                "{} {}",
                                icon::WARNING_CIRCLE,
                                v["error"].as_str().map(str::to_string).unwrap_or_else(|| t("settings.notLoggedIn"))
                            ))
                            .color(tk.p.red),
                        );
                        ui.label(widgets::faint(&tk, tf("account.loginHint", &[("dir", &widgets::short_path(&path))])));
                    }
                    None => {
                        ui.spinner();
                    }
                }
            });
        ui.add_space(6.0);
    }
}

// ---------- provedores ----------

fn providers(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    ui.label(widgets::muted(&tk, t("providers.hint")));
    ui.add_space(8.0);
    for (p, label, vendor) in lazyaicode_doc::PROVIDERS.iter().skip(1) {
        let st = app.data.providers.get(*p).cloned();
        if st.is_none() {
            app.data.providers.insert(p.to_string(), json!({ "loading": true }));
            let pp = p.to_string();
            app.call("provider.status", json!({ "provider": p }), move |app, r| {
                app.data.providers.insert(pp, r.unwrap_or_else(|e| json!({ "ok": false, "error": e.message })));
            });
        }
        let st = st.unwrap_or(Value::Null);
        egui::Frame::new()
            .fill(tk.p.panel)
            .stroke(Stroke::new(1.0, tk.p.border))
            .corner_radius(egui::CornerRadius::same(tk.radius))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label.to_string()).strong().size(tk.text_size + 1.0));
                    ui.label(widgets::muted(&tk, *vendor));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button(icon::ARROWS_CLOCKWISE).clicked() {
                            app.data.providers.remove(*p);
                        }
                    });
                });
                if st["loading"] == true {
                    ui.spinner();
                    return;
                }
                if let Some(agent) = st["agent"].as_object() {
                    ui.label(widgets::muted(
                        &tk,
                        format!(
                            "{} {}",
                            agent.get("title").or_else(|| agent.get("name")).and_then(Value::as_str).unwrap_or(""),
                            agent.get("version").and_then(Value::as_str).unwrap_or("")
                        ),
                    ));
                }
                if let Some(how) = st["launch"]["how"].as_str() {
                    ui.label(widgets::faint(&tk, format!("{}: {how}", t("providers.launch"))));
                }
                if st["ok"] == true {
                    ui.label(RichText::new(format!("{} {}", icon::CHECK_CIRCLE, t("providers.ready"))).color(tk.p.accent));
                } else {
                    ui.label(
                        RichText::new(format!("{} {}", icon::WARNING_CIRCLE, st["error"].as_str().unwrap_or(&t("providers.notReady"))))
                            .color(tk.p.amber),
                    );
                    if ui.button(t("providers.login")).clicked() {
                        let pp = p.to_string();
                        app.call_ok("provider.auth-methods", json!({ "provider": p }), move |app, v| {
                            let methods = v["methods"].as_array().cloned().unwrap_or_default();
                            if let Some(m) = methods.first() {
                                let id = m["id"].as_str().unwrap_or("").to_string();
                                app.toast(tf("providers.loggingIn", &[("method", &m["name"].as_str().unwrap_or(&id))]), ToastKind::Info);
                                let p2 = pp.clone();
                                app.call_ok("provider.login", json!({ "provider": pp, "method": id }), move |app, _| {
                                    app.data.providers.remove(&p2);
                                });
                            }
                        });
                    }
                }
            });
        ui.add_space(8.0);
    }
}

// ---------- skills, agentes, MCP ----------

fn caps_value(app: &mut LazyApp) -> Option<Value> {
    let key = app.data.caps_key(None);
    if let Some(v) = app.data.caps.get(&key) {
        return Some(v.clone());
    }
    crate::views::composer::load_caps(app, None);
    None
}

fn capabilities(app: &mut LazyApp, ui: &mut egui::Ui, field: &str, kind: &str) {
    let tk = app.tokens;
    let Some(c) = caps_value(app) else {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(widgets::muted(&tk, t("capabilities.loading")));
        });
        return;
    };
    if let Some(e) = c["error"].as_str() {
        ui.label(RichText::new(e).color(tk.p.red));
    }
    ui.horizontal(|ui| {
        if let Some(v) = c["version"].as_str() {
            ui.label(widgets::faint(&tk, format!("Claude Code {v}")));
        }
        if ui.small_button(format!("{} {}", icon::ARROWS_CLOCKWISE, t("capabilities.refresh"))).clicked() {
            let key = app.data.caps_key(None);
            app.data.caps.remove(&key);
        }
    });
    let items = c[field].as_array().cloned().unwrap_or_default();
    let items: Vec<Value> = items.into_iter().filter(|i| kind == "agent" || i["kind"] != "agent").collect();
    if items.is_empty() {
        ui.label(widgets::faint(&tk, t("capabilities.none")));
    }
    let ws = app.data.active_workspace.clone().unwrap_or_default();
    for it in items {
        let name = it["name"].as_str().unwrap_or("");
        let scope = it["scope"].as_str().unwrap_or("");
        egui::Frame::new()
            .fill(tk.p.panel)
            .stroke(Stroke::new(1.0, tk.p.border))
            .corner_radius(egui::CornerRadius::same(tk.radius_sm))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let pre = if kind == "agent" { "@" } else { "/" };
                    ui.label(RichText::new(format!("{pre}{name}")).monospace().strong());
                    widgets::chip(ui, &tk, scope, tk.p.muted);
                    if it["kind"] == "skill" {
                        widgets::chip(ui, &tk, "skill", tk.p.blue);
                    }
                    if let Some(e) = it["error"].as_str() {
                        widgets::chip(ui, &tk, e, tk.p.red);
                    }
                    let path = it["path"].as_str().unwrap_or("").to_string();
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !path.is_empty()
                            && matches!(scope, "global" | "project")
                            && ui.small_button(RichText::new(icon::TRASH).color(tk.p.red)).on_hover_text(t("capabilities.trash")).clicked()
                        {
                            app.eph.confirm = Some((
                                tf("capabilities.trashConfirm", &[("name", &name)]),
                                "capabilities.trash".into(),
                                json!({ "workspaceId": ws, "path": path }),
                            ));
                            app.state.dialog = Some("confirm".into());
                        }
                        if !path.is_empty() && ui.small_button(icon::FOLDER_OPEN).clicked() {
                            app.fire("system.open-path", json!({ "path": path }));
                        }
                    });
                });
                if let Some(d) = it["description"].as_str().filter(|d| !d.is_empty()) {
                    ui.add(egui::Label::new(RichText::new(d).color(tk.p.muted)).wrap());
                }
            });
        ui.add_space(4.0);
    }
}

fn mcp(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    let Some(c) = caps_value(app) else {
        ui.spinner();
        return;
    };
    let servers = c["mcp"].as_array().cloned().unwrap_or_default();
    if servers.is_empty() {
        ui.label(widgets::faint(&tk, t("capabilities.noMcp")));
    }
    for s in servers {
        let status = s["status"].as_str().unwrap_or("");
        let color = match status {
            "connected" => tk.p.accent,
            "failed" | "needs-auth" => tk.p.red,
            _ => tk.p.amber,
        };
        egui::Frame::new()
            .fill(tk.p.panel)
            .stroke(Stroke::new(1.0, tk.p.border))
            .corner_radius(egui::CornerRadius::same(tk.radius_sm))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon::CIRCLE).color(color));
                    ui.label(RichText::new(s["name"].as_str().unwrap_or("")).strong());
                    ui.label(widgets::faint(&tk, status));
                    if let Some(scope) = s["scope"].as_str() {
                        widgets::chip(ui, &tk, scope, tk.p.muted);
                    }
                });
                let detail = s["url"].as_str().or_else(|| s["command"].as_str()).unwrap_or("");
                if !detail.is_empty() {
                    ui.label(RichText::new(detail).monospace().size(tk.code_size - 1.0).color(tk.p.faint));
                }
                if let Some(e) = s["error"].as_str() {
                    ui.label(RichText::new(e).color(tk.p.red));
                }
                let tools = s["tools"].as_array().map_or(0, Vec::len);
                if tools > 0 {
                    ui.label(widgets::muted(&tk, tf("capabilities.tools", &[("n", &tools)])));
                }
            });
        ui.add_space(4.0);
    }
}

// ---------- git ----------

fn pref_str(app: &LazyApp, k: &str, default: &str) -> String {
    app.data.prefs.get(k).and_then(Value::as_str).unwrap_or(default).to_string()
}

fn git(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    let mut sign = app.data.prefs.get("signCommits").and_then(Value::as_bool).unwrap_or(false);
    row(ui, &tk, &t("gitSigning.sign"), &t("gitSigning.hint"), |ui| {
        if ui.checkbox(&mut sign, "").changed() {
            set_pref(app, "signCommits", json!(sign));
        }
    });
    let mut model = pref_str(app, "git.aiModel", "haiku");
    row(ui, &tk, &t("gitAi.model"), &t("gitAi.modelHint"), |ui| {
        egui::ComboBox::from_id_salt("git-ai-model").selected_text(&model).show_ui(ui, |ui| {
            for m in ["haiku", "sonnet", "opus", "fable"] {
                if ui.selectable_value(&mut model, m.to_string(), m).changed() {
                    set_pref(app, "git.aiModel", json!(m));
                }
            }
        });
    });
    let mut instr = pref_str(app, "git.aiInstructions", "");
    row(ui, &tk, &t("gitAi.instructions"), &t("gitAi.instructionsHint"), |ui| {
        let r = ui.add(egui::TextEdit::multiline(&mut instr).desired_rows(4).desired_width(380.0));
        if r.lost_focus() {
            set_pref(app, "git.aiInstructions", json!(instr));
        }
    });
    widgets::section(ui, &tk, &t("gitProfiles.identity"));
    match app.data.prefs.get("\u{0}identity").cloned() {
        Some(v) => {
            ui.label(format!("{} <{}>", v["name"].as_str().unwrap_or("—"), v["email"].as_str().unwrap_or("—")));
        }
        None => {
            app.data.prefs.insert("\u{0}identity".into(), json!({}));
            app.call("git.config.identity", Value::Null, |app, r| {
                if let Ok(v) = r {
                    app.data.prefs.insert("\u{0}identity".into(), v);
                }
            });
        }
    }
}

fn integrations(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    ui.label(widgets::muted(&tk, t("gitIntegrations.hint")));
    ui.add_space(8.0);
    for (prefix, name, default_url) in [("github", "GitHub", "https://api.github.com"), ("gitlab", "GitLab", "https://gitlab.com")] {
        widgets::section(ui, &tk, name);
        let mut url = pref_str(app, &format!("{prefix}.url"), default_url);
        let mut token = pref_str(app, &format!("{prefix}.token"), "");
        row(ui, &tk, &t("gitIntegrations.url"), "", |ui| {
            if ui.add(egui::TextEdit::singleline(&mut url).desired_width(320.0)).lost_focus() {
                set_pref(app, &format!("{prefix}.url"), json!(url));
            }
        });
        row(ui, &tk, &t("gitIntegrations.token"), &t("gitIntegrations.tokenHint"), |ui| {
            if ui.add(egui::TextEdit::singleline(&mut token).password(true).desired_width(320.0)).lost_focus() {
                set_pref(app, &format!("{prefix}.token"), json!(token));
            }
        });
    }
}

// ---------- atalhos ----------

fn shortcuts(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    ui.label(widgets::muted(&tk, t("shortcuts.hint")));
    ui.add_space(6.0);
    let mut filter = app.data.prefs.get("\u{0}sc-filter").and_then(Value::as_str).unwrap_or("").to_string();
    if ui.add(egui::TextEdit::singleline(&mut filter).hint_text(t("sidebar.filterPlaceholder")).desired_width(300.0)).changed() {
        app.data.prefs.insert("\u{0}sc-filter".into(), json!(filter));
    }
    let editing = app.data.prefs.get("\u{0}sc-edit").and_then(Value::as_str).map(str::to_string);
    if let Some(id) = &editing {
        let captured = ui.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } if *key != egui::Key::Escape => {
                    let mut s = String::new();
                    if modifiers.command || modifiers.ctrl {
                        s.push_str("Ctrl+");
                    }
                    if modifiers.alt {
                        s.push_str("Alt+");
                    }
                    if modifiers.shift {
                        s.push_str("Shift+");
                    }
                    s.push_str(key.name());
                    Some(s)
                }
                _ => None,
            })
        });
        if let Some(sc) = captured {
            app.state.shortcuts.insert(id.clone(), sc);
            app.data.prefs.remove("\u{0}sc-edit");
        } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.data.prefs.remove("\u{0}sc-edit");
        }
    }
    let q = filter.to_lowercase();
    egui::Grid::new("shortcuts").striped(true).num_columns(3).spacing([16.0, 4.0]).show(ui, |ui| {
        for c in crate::commands::ui_commands() {
            let label = {
                let l = t(&c.label);
                if l == c.label && c.label.contains('.') && !c.label.contains(' ') {
                    c.id.clone()
                } else {
                    l
                }
            };
            if !q.is_empty() && !label.to_lowercase().contains(&q) && !c.id.contains(&q) {
                continue;
            }
            let sc = crate::commands::shortcut_of(app, c);
            ui.label(&label);
            ui.label(widgets::faint(&tk, &c.id));
            ui.horizontal(|ui| {
                if editing.as_deref() == Some(c.id.as_str()) {
                    ui.label(RichText::new(t("shortcuts.pressKeys")).color(tk.p.amber));
                } else {
                    match &sc {
                        Some(s) => widgets::kbd(ui, &tk, s),
                        None => {
                            ui.label(widgets::faint(&tk, "—"));
                        }
                    }
                    if ui.small_button(icon::PENCIL_SIMPLE).on_hover_text(t("shortcuts.change")).clicked() {
                        app.data.prefs.insert("\u{0}sc-edit".into(), json!(c.id));
                    }
                    if app.state.shortcuts.contains_key(&c.id)
                        && ui.small_button(icon::ARROW_COUNTER_CLOCKWISE).on_hover_text(t("shortcuts.reset")).clicked()
                    {
                        app.state.shortcuts.remove(&c.id);
                    }
                    if sc.is_some() && ui.small_button(icon::X).on_hover_text(t("shortcuts.remove")).clicked() {
                        app.state.shortcuts.insert(c.id.clone(), String::new());
                    }
                }
            });
            ui.end_row();
        }
    });
}

fn about(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    ui.label(RichText::new("LazyAICode").size(24.0).strong());
    let v = app.data.version.clone().unwrap_or(Value::Null);
    ui.label(widgets::muted(
        &tk,
        format!(
            "{} {} · {} {} · {}/{}",
            t("about.version"),
            v["version"].as_str().unwrap_or("?"),
            t("about.protocol"),
            v["protocol"],
            v["os"].as_str().unwrap_or(""),
            v["arch"].as_str().unwrap_or("")
        ),
    ));
    ui.add_space(10.0);
    let engine = app.data.commands.iter().filter(|c| c.scope != "ui").count();
    let screen = app.data.commands.len() - engine;
    egui::Grid::new("about").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
        ui.label(t("about.commands"));
        ui.label(format!("{} ({engine} {} + {screen} {})", app.data.commands.len(), t("about.engine"), t("about.screen")));
        ui.end_row();
        ui.label(t("about.connection"));
        ui.label(app.client.describe());
        ui.end_row();
        ui.label(t("about.dataDir"));
        ui.label(RichText::new(&app.data.data_dir).monospace());
        ui.end_row();
        ui.label("Claude Code");
        ui.label(RichText::new(app.data.claude_binary.clone().unwrap_or_else(|| t("about.claudeMissing"))).monospace());
        ui.end_row();
        ui.label(t("about.platform"));
        ui.label(app.host.platform());
        ui.end_row();
    });
    ui.add_space(10.0);
    if !app.host.is_web() {
        ui.horizontal(|ui| {
            if widgets::button(ui, &tk, &format!("{}  {}", icon::ARROWS_CLOCKWISE, t("update.check"))).clicked() {
                app.check_update(true);
            }
            if let Some(u) = app.data.update.as_ref().filter(|u| u["newer"] == true) {
                ui.label(
                    RichText::new(lazyaicode_i18n::tf("update.available", &[("version", &u["latest"].as_str().unwrap_or(""))]))
                        .color(tk.p.accent),
                );
            }
        });
        if let Some(mut s) = app.data.settings.clone() {
            if ui.checkbox(&mut s.check_updates, t("update.checkSetting")).changed() {
                app.data.settings = Some(s);
                save_settings(app);
            }
        }
        ui.add_space(10.0);
    }
    ui.hyperlink_to("github.com/filipecrespodev/lazyaicode", "https://github.com/filipecrespodev/lazyaicode");
    let _ = Color32::TRANSPARENT;
}

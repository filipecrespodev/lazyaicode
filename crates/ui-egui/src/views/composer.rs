//! O campo de digitar: rascunho por thread, Enter envia (ou Ctrl+Enter, na
//! preferência), menu `/` com os skills e comandos, imagens coladas ou
//! arrastadas.

use base64::Engine as _;
use egui::{Key, Modifiers, RichText, Stroke};
use egui_phosphor::regular as icon;
use lazyaicode_doc::{Status, ThreadView};
use lazyaicode_i18n::t;
use serde_json::{json, Value};

use crate::app::LazyApp;
use crate::data::Attachment;
use crate::views::widgets;

pub const MAX_IMAGES: usize = 10;

/// Texto depois da `/` quando o rascunho é só um comando sendo digitado.
pub fn slash_query(text: &str) -> Option<&str> {
    let q = text.strip_prefix('/')?;
    q.chars().all(|c| c.is_alphanumeric() || matches!(c, ':' | '.' | '-' | '_')).then_some(q)
}

/// Itens que casam com a busca: nome começando com o termo (também depois do
/// `plugin:`) primeiro, depois nome contendo, depois descrição.
pub fn rank(items: &[Value], q: &str, limit: usize) -> Vec<Value> {
    let q = q.to_lowercase();
    let scope_rank = |s: &str| match s {
        "project" => 0,
        "global" => 1,
        "plugin" => 2,
        "other" => 3,
        _ => 4,
    };
    let mut scored: Vec<(i32, &Value)> = items
        .iter()
        .filter(|i| i["loaded"].as_bool().unwrap_or(true) && i["kind"] != "agent")
        .filter_map(|i| {
            let name = i["name"].as_str().unwrap_or("").to_lowercase();
            let short = name.split_once(':').map_or(name.as_str(), |(_, s)| s).to_string();
            let desc = i["description"].as_str().unwrap_or("").to_lowercase();
            let s = if q.is_empty() {
                1
            } else if name.starts_with(&q) || short.starts_with(&q) {
                4
            } else if name.contains(&q) {
                3
            } else if desc.contains(&q) {
                1
            } else {
                0
            };
            (s > 0).then_some((s, i))
        })
        .collect();
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then(scope_rank(a.1["scope"].as_str().unwrap_or("")).cmp(&scope_rank(b.1["scope"].as_str().unwrap_or(""))))
            .then(a.1["name"].as_str().cmp(&b.1["name"].as_str()))
    });
    scored.into_iter().take(limit).map(|(_, v)| v.clone()).collect()
}

fn slash_source(app: &mut LazyApp, th: &ThreadView) -> Option<Vec<Value>> {
    if th.thread.provider != "claude" {
        let cmds = app.data.acp_session.get(&th.thread.id).and_then(|s| s["commands"].as_array().cloned()).unwrap_or_default();
        return Some(
            cmds.into_iter()
                .map(|c| json!({ "kind": "command", "name": c["name"], "description": c["description"], "scope": "other", "loaded": true }))
                .collect(),
        );
    }
    let path = app.data.project(&th.thread.project_id).map(|p| p.path.clone());
    let key = app.data.caps_key(path.as_deref());
    if let Some(c) = app.data.caps.get(&key) {
        return c["commands"].as_array().cloned();
    }
    load_caps(app, path);
    None
}

pub fn load_caps(app: &mut LazyApp, path: Option<String>) {
    let Some(ws) = app.data.active_workspace.clone() else { return };
    let key = app.data.caps_key(path.as_deref());
    if !app.data.caps_loading.insert(key.clone()) {
        return;
    }
    app.call("capabilities.get", json!({ "workspaceId": ws, "path": path }), move |app, r| {
        app.data.caps_loading.remove(&key);
        match r {
            Ok(v) => {
                app.data.caps.insert(key, v);
            }
            Err(e) => {
                app.data.caps.insert(key, json!({ "error": e.message, "commands": [] }));
            }
        }
    });
}

pub fn attach_path(app: &mut LazyApp, id: &str, path: &str) {
    let tid = id.to_string();
    let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    app.call("system.read-image", json!({ "path": path }), move |app, r| match r {
        Ok(v) => {
            let list = app.data.attachments.entry(tid).or_default();
            if list.len() < MAX_IMAGES {
                list.push(Attachment {
                    mime: v["mime"].as_str().unwrap_or("image/png").into(),
                    data: v["data"].as_str().unwrap_or("").into(),
                    name,
                });
            }
        }
        Err(e) => {
            let msg = if e.message.contains("too_large") {
                lazyaicode_i18n::tf("images.tooLarge", &[("name", &name)])
            } else {
                format!("{}: {}", t("thread.onlyImages"), e.message)
            };
            app.toast_err(&msg);
        }
    });
}

pub fn paste_image(app: &mut LazyApp, id: &str) {
    match app.host.clipboard_image() {
        Some(png) => {
            let list = app.data.attachments.entry(id.to_string()).or_default();
            if list.len() < MAX_IMAGES {
                list.push(Attachment {
                    mime: "image/png".into(),
                    data: base64::engine::general_purpose::STANDARD.encode(png),
                    name: "clipboard.png".into(),
                });
            }
        }
        None => app.toast_err(&t("images.noClipboardImage")),
    }
}

pub fn show(app: &mut LazyApp, ui: &mut egui::Ui, th: &ThreadView) {
    let tk = app.tokens;
    let id = th.thread.id.clone();
    let busy = matches!(th.status, Status::Running | Status::Awaiting);
    let cid = egui::Id::new(("composer", &id));

    // arquivos arrastados para a janela viram anexos
    let dropped: Vec<String> =
        ui.ctx().input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_string_lossy().to_string()).filter(|p| !p.is_empty()).collect());
    for p in dropped {
        if th.thread.archived {
            app.toast_err(&t("thread.threadArchived"));
        } else {
            attach_path(app, &id, &p);
        }
    }

    // anexos
    let atts = app.data.attachments.get(&id).cloned().unwrap_or_default();
    if !atts.is_empty() {
        let mut remove = None;
        ui.horizontal_wrapped(|ui| {
            for (n, a) in atts.iter().enumerate() {
                ui.vertical(|ui| {
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&a.data) {
                        let url = format!("data:{};base64,{}", a.mime, a.data);
                        if crate::views::transcript::image_thumb(ui, format!("bytes://att-{id}-{n}-{}.img", a.data.len()), bytes, 72.0)
                            .on_hover_text(&a.name)
                            .clicked()
                        {
                            app.eph.preview_image = Some(url);
                        }
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new(format!("{} {}", icon::X, t("thread.remove"))).size(tk.text_size - 2.0))
                                .frame(false),
                        )
                        .clicked()
                    {
                        remove = Some(n);
                    }
                });
            }
        });
        if let Some(n) = remove {
            if let Some(l) = app.data.attachments.get_mut(&id) {
                if n < l.len() {
                    l.remove(n);
                }
            }
        }
    }

    let mut draft = app.data.drafts.get(&id).cloned().unwrap_or_default();
    let focused = ui.memory(|m| m.has_focus(cid));
    let slash: Option<String> = if app.eph.slash_dismissed { None } else { slash_query(&draft).map(str::to_string) };
    let items: Vec<Value> = match &slash {
        Some(q) => slash_source(app, th).map(|src| rank(&src, q, 60)).unwrap_or_default(),
        None => vec![],
    };
    if slash.is_none() {
        app.eph.slash_index = 0;
    }
    let n = items.len();
    let mut pick: Option<Value> = None;
    let mut send = std::mem::take(&mut app.eph.send_now);
    if focused {
        if n > 0 {
            ui.input_mut(|i| {
                if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                    app.eph.slash_index = (app.eph.slash_index + 1) % n;
                }
                if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                    app.eph.slash_index = (app.eph.slash_index + n - 1) % n;
                }
                if i.consume_key(Modifiers::NONE, Key::Enter) || i.consume_key(Modifiers::NONE, Key::Tab) {
                    pick = items.get(app.eph.slash_index.min(n - 1)).cloned();
                }
                if i.consume_key(Modifiers::NONE, Key::Escape) {
                    app.eph.slash_dismissed = true;
                }
            });
        }
        let with_mod = app.data.settings.as_ref().is_some_and(|s| s.send_with_modifier);
        ui.input_mut(|i| {
            let plain = !with_mod && i.consume_key(Modifiers::NONE, Key::Enter);
            let modded = with_mod && (i.consume_key(Modifiers::COMMAND, Key::Enter) || i.consume_key(Modifiers::CTRL, Key::Enter));
            send |= plain || modded;
            if busy && i.consume_key(Modifiers::NONE, Key::Escape) {
                app.fire("thread.interrupt", json!({ "id": id }));
            }
        });
    }

    let has_items = app.data.transcripts.get(&id).is_some_and(|t| !t.items.is_empty());
    let hint = if th.thread.archived {
        t("thread.threadArchived")
    } else if busy {
        t("thread.agentWorking")
    } else if has_items {
        t("thread.reply")
    } else {
        t("thread.describeTask")
    };
    // o campo é a próxima mensagem da conversa: sem moldura, como no app anterior
    let resp = ui.add_enabled(
        !th.thread.archived,
        egui::TextEdit::multiline(&mut draft)
            .id(cid)
            .hint_text(RichText::new(hint).color(tk.p.faint))
            .font(egui::FontId::proportional(tk.text_size + 1.0))
            .desired_rows(1)
            .desired_width(f32::INFINITY)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .lock_focus(true),
    );
    if std::mem::take(&mut app.eph.focus_composer) {
        resp.request_focus();
    }
    if resp.changed() {
        app.eph.slash_dismissed = false;
    }
    if let Some(item) = pick {
        let name = item["name"].as_str().unwrap_or("");
        draft = format!("/{name} ");
        app.eph.slash_dismissed = true;
        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), cid) {
            let c = egui::text::CCursor::new(draft.chars().count());
            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(c)));
            state.store(ui.ctx(), cid);
        }
    }

    // menu "/"
    if slash.is_some() && focused {
        egui::Frame::new()
            .fill(tk.p.elev)
            .stroke(Stroke::new(1.0, tk.p.border))
            .corner_radius(egui::CornerRadius::same(tk.radius))
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                ui.label(RichText::new(t("thread.commandsAndSkills")).size(tk.text_size - 2.0).color(tk.p.muted));
                if items.is_empty() {
                    let loading = app.data.caps_loading.iter().any(|k| k.starts_with(app.data.active_workspace.as_deref().unwrap_or("")));
                    ui.label(widgets::faint(&tk, if loading { t("thread.loading") } else { t("thread.noCommands") }));
                }
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    for (i, it) in items.iter().enumerate() {
                        let on = i == app.eph.slash_index;
                        let name = it["name"].as_str().unwrap_or("");
                        let desc = it["description"].as_str().unwrap_or("");
                        let scope = it["scope"].as_str().unwrap_or("");
                        let r = ui
                            .horizontal(|ui| {
                                let r = ui.selectable_label(on, RichText::new(format!("/{name}")).monospace().color(tk.p.text));
                                ui.label(RichText::new(desc).size(tk.text_size - 1.0).color(tk.p.muted));
                                if !scope.is_empty() {
                                    widgets::chip(ui, &tk, &t(&format!("thread.scope{}", capitalize(scope))), tk.p.faint);
                                }
                                r
                            })
                            .inner;
                        if on {
                            r.scroll_to_me(None);
                        }
                        if r.clicked() {
                            draft = format!("/{name} ");
                            app.eph.slash_dismissed = true;
                            app.eph.focus_composer = true;
                        }
                    }
                });
            });
    }

    if send && !busy && !th.thread.archived && (!draft.trim().is_empty() || !atts.is_empty()) {
        let text = draft.trim().to_string();
        draft.clear();
        app.data.drafts.remove(&id);
        app.send_message(&id, text);
        app.eph.focus_composer = true;
    } else if draft.is_empty() {
        app.data.drafts.remove(&id);
    } else {
        app.data.drafts.insert(id, draft);
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_menu_ranks_like_before() {
        assert_eq!(slash_query("/rev"), Some("rev"));
        assert_eq!(slash_query("/rev x"), None);
        assert_eq!(slash_query("rev"), None);
        let items = vec![
            json!({"name": "review", "description": "", "scope": "global", "kind": "command"}),
            json!({"name": "plug:revert", "description": "", "scope": "plugin", "kind": "skill"}),
            json!({"name": "init", "description": "review the repo", "scope": "builtin", "kind": "command"}),
            json!({"name": "rev-agent", "description": "", "scope": "global", "kind": "agent"}),
        ];
        let names: Vec<String> = rank(&items, "rev", 10).iter().map(|v| v["name"].as_str().unwrap_or("").to_string()).collect();
        assert_eq!(names, vec!["review", "plug:revert", "init"]);
    }
}

//! Revisão do que o agente mudou: os arquivos desde a base escolhida, o
//! diff de cada um e a marca de "visto" (que se desfaz sozinha quando o diff
//! muda).

use egui::{Align, Layout, RichText, Sense, Vec2};
use egui_phosphor::regular as icon;
use lazyaicode_doc::diff::{parse_patch, LineKind};
use lazyaicode_doc::ThreadView;
use lazyaicode_i18n::t;
use serde_json::{json, Value};

use crate::app::LazyApp;
use crate::state::ReviewBase;
use crate::views::widgets;

pub fn load_changes(app: &mut LazyApp, id: &str, base: &str) {
    let tid = id.to_string();
    app.call("thread.changes", json!({ "id": id, "base": base }), move |app, r| match r {
        Ok(v) => {
            app.data.changes.insert(tid, v);
        }
        Err(e) => {
            app.data.changes.insert(tid, json!({ "error": e.message, "files": [] }));
        }
    });
}

fn files(app: &LazyApp) -> Vec<Value> {
    app.state.selected.as_ref().and_then(|id| app.data.changes.get(id)).and_then(|c| c["files"].as_array().cloned()).unwrap_or_default()
}

pub fn step_file(app: &mut LazyApp, d: i32) -> Result<Value, String> {
    let f = files(app);
    if f.is_empty() {
        return Ok(Value::Null);
    }
    let cur = app.state.review_file.as_ref().and_then(|p| f.iter().position(|x| x["path"] == p.as_str()));
    let i = match cur {
        Some(i) => (i as i32 + d).clamp(0, f.len() as i32 - 1) as usize,
        None => 0,
    };
    app.state.review_file = f.get(i).and_then(|x| x["path"].as_str()).map(str::to_string);
    Ok(Value::Null)
}

pub fn toggle_seen(app: &mut LazyApp, path: Option<String>) -> Result<Value, String> {
    let id = app.state.selected.clone().ok_or_else(|| t("palette.needsThread"))?;
    let path = path.or_else(|| app.state.review_file.clone()).ok_or("nenhum arquivo")?;
    let f = files(app);
    let file = f.iter().find(|x| x["path"] == path.as_str()).ok_or("arquivo fora da revisão")?;
    let seen = file["seen"].as_bool().unwrap_or(false);
    let fp = if seen { Value::Null } else { file["fingerprint"].clone() };
    set_seen(app, &id, vec![json!([path, fp])]);
    Ok(Value::Null)
}

pub fn mark_all(app: &mut LazyApp, seen: bool) -> Result<Value, String> {
    let id = app.state.selected.clone().ok_or_else(|| t("palette.needsThread"))?;
    let entries: Vec<Value> =
        files(app).iter().map(|f| json!([f["path"], if seen { f["fingerprint"].clone() } else { Value::Null }])).collect();
    set_seen(app, &id, entries);
    Ok(Value::Null)
}

fn set_seen(app: &mut LazyApp, id: &str, entries: Vec<Value>) {
    // atualiza já na tela; o motor confirma
    if let Some(c) = app.data.changes.get_mut(id) {
        if let Some(fs) = c["files"].as_array_mut() {
            for e in &entries {
                for f in fs.iter_mut() {
                    if f["path"] == e[0] {
                        f["seen"] = json!(!e[1].is_null());
                    }
                }
            }
        }
    }
    app.fire("thread.set-seen", json!({ "id": id, "entries": entries }));
}

pub fn show(app: &mut LazyApp, ui: &mut egui::Ui, th: &ThreadView) {
    let tk = app.tokens;
    let id = th.thread.id.clone();
    if !app.data.changes.contains_key(&id) {
        let base = app.state.review_base.id();
        load_changes(app, &id, base);
    }
    let ch = app.data.changes.get(&id).cloned().unwrap_or(Value::Null);
    egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 6)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(widgets::muted(&tk, t("agentReview.base")));
            for base in [ReviewBase::Upstream, ReviewBase::Turn, ReviewBase::Head] {
                if ui.selectable_label(app.state.review_base == base, t(&format!("agentReview.base.{}", base.id()))).clicked() {
                    app.state.review_base = base;
                    load_changes(app, &id, base.id());
                }
            }
            if let Some(label) = ch["baseLabel"].as_str() {
                ui.label(widgets::faint(&tk, format!("({label})")));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_button(ui, &tk, icon::ARROWS_CLOCKWISE, &t("agentReview.refresh")).clicked() {
                    let base = app.state.review_base.id();
                    load_changes(app, &id, base);
                }
                let fs = ch["files"].as_array().map_or(0, Vec::len);
                let seen = ch["files"].as_array().map_or(0, |f| f.iter().filter(|x| x["seen"] == true).count());
                if fs > 0 {
                    ui.label(widgets::muted(&tk, format!("{seen}/{fs} {}", t("agentReview.reviewed"))));
                    if ui.small_button(t("agentReview.markAll")).clicked() {
                        let _ = mark_all(app, seen < fs);
                    }
                }
            });
        });
    });
    widgets::hline(ui, &tk);
    if let Some(err) = ch["error"].as_str() {
        ui.add_space(12.0);
        ui.label(RichText::new(err).color(tk.p.red));
        return;
    }
    let list = ch["files"].as_array().cloned().unwrap_or_default();
    if list.is_empty() {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(widgets::muted(&tk, if ch.is_null() { t("thread.loading") } else { t("agentReview.noChanges") }))
        });
        return;
    }
    if app.state.review_file.as_ref().is_none_or(|p| !list.iter().any(|f| f["path"] == p.as_str())) {
        app.state.review_file = list.first().and_then(|f| f["path"].as_str()).map(str::to_string);
    }
    egui::Panel::left(egui::Id::new(("review-files", &id)))
        .default_size(280.0)
        .resizable(true)
        .frame(egui::Frame::new().fill(tk.p.panel).inner_margin(egui::Margin::same(4)))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for f in &list {
                    file_row(app, ui, f);
                }
            });
        });
    egui::CentralPanel::default().frame(egui::Frame::new().fill(tk.p.bg)).show(ui, |ui| {
        let Some(path) = app.state.review_file.clone() else { return };
        let Some(f) = list.iter().find(|x| x["path"] == path.as_str()) else { return };
        diff_view(app, ui, f);
    });
}

fn status_icon(s: &str) -> &'static str {
    match s {
        "added" | "A" | "untracked" => icon::PLUS_SQUARE,
        "deleted" | "D" => icon::MINUS_SQUARE,
        "renamed" | "R" => icon::ARROW_SQUARE_RIGHT,
        _ => icon::PENCIL_SIMPLE_LINE,
    }
}

fn file_row(app: &mut LazyApp, ui: &mut egui::Ui, f: &Value) {
    let tk = app.tokens;
    let path = f["path"].as_str().unwrap_or("").to_string();
    let on = app.state.review_file.as_deref() == Some(path.as_str());
    let seen = f["seen"].as_bool().unwrap_or(false);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), tk.row_h), Sense::click());
    if on {
        ui.painter().rect_filled(rect, egui::CornerRadius::same(tk.radius_sm), tk.p.active);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, egui::CornerRadius::same(tk.radius_sm), tk.p.hover);
    }
    let p = ui.painter_at(rect);
    let check = egui::Rect::from_center_size(rect.left_center() + Vec2::new(12.0, 0.0), Vec2::splat(16.0));
    let cr = ui.interact(check, resp.id.with("seen"), Sense::click()).on_hover_text(t("agentReview.markSeen"));
    p.text(
        check.center(),
        egui::Align2::CENTER_CENTER,
        if seen { icon::CHECK_SQUARE } else { icon::SQUARE },
        egui::FontId::proportional(tk.text_size + 1.0),
        if seen { tk.p.accent } else { tk.p.faint },
    );
    let status = f["status"].as_str().unwrap_or("");
    p.text(
        rect.left_center() + Vec2::new(28.0, 0.0),
        egui::Align2::LEFT_CENTER,
        status_icon(status),
        egui::FontId::proportional(tk.text_size),
        tk.p.muted,
    );
    let (a, d) = (f["additions"].as_u64().unwrap_or(0), f["deletions"].as_u64().unwrap_or(0));
    let nums = format!("+{a} −{d}");
    let ng = p.layout_no_wrap(nums, egui::FontId::monospace(tk.code_size - 2.0), tk.p.faint);
    let nw = ng.size().x;
    p.galley(rect.right_center() - Vec2::new(nw + 6.0, ng.size().y / 2.0), ng, tk.p.faint);
    let name = path.rsplit('/').next().unwrap_or(&path).to_string();
    let dir = path.strip_suffix(&name).unwrap_or("").trim_end_matches('/').to_string();
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &name,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(tk.text_size),
            color: if seen { tk.p.muted } else { tk.p.text },
            ..Default::default()
        },
    );
    if !dir.is_empty() {
        job.append(
            &format!("  {dir}"),
            0.0,
            egui::TextFormat { font_id: egui::FontId::proportional(tk.text_size - 2.0), color: tk.p.faint, ..Default::default() },
        );
    }
    job.wrap = egui::text::TextWrapping::truncate_at_width((rect.width() - 52.0 - nw).max(20.0));
    let g = p.layout_job(job);
    p.galley(egui::pos2(rect.left() + 46.0, rect.center().y - g.size().y / 2.0), g, tk.p.text);
    if cr.clicked() {
        let _ = toggle_seen(app, Some(path.clone()));
    } else if resp.on_hover_text(&path).clicked() {
        app.state.review_file = Some(path);
    }
}

pub fn diff_view(app: &mut LazyApp, ui: &mut egui::Ui, f: &Value) {
    let tk = app.tokens;
    let path = f["path"].as_str().unwrap_or("");
    egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(path).monospace().strong());
            if let Some(old) = f["old_path"].as_str() {
                ui.label(widgets::faint(&tk, format!("← {old}")));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let seen = f["seen"].as_bool().unwrap_or(false);
                if ui.selectable_label(seen, format!("{}  {}", icon::CHECK, t("agentReview.seen"))).clicked() {
                    let _ = toggle_seen(app, Some(path.to_string()));
                }
            });
        });
    });
    if f["binary"].as_bool().unwrap_or(false) {
        ui.label(widgets::muted(&tk, t("diffFile.binary")));
        return;
    }
    let lines = parse_patch(f["patch"].as_str().unwrap_or(""));
    patch_lines(ui, &tk, &lines, ("review", path));
    if f["truncated"].as_bool().unwrap_or(false) {
        ui.label(widgets::faint(&tk, t("diffFile.truncated")));
    }
}

/// Desenha linhas de diff com numeração (rápido para arquivos grandes: só
/// as linhas visíveis são montadas).
pub fn patch_lines(
    ui: &mut egui::Ui,
    tk: &crate::theme::Tokens,
    lines: &[lazyaicode_doc::diff::DiffLine],
    salt: impl std::hash::Hash + std::fmt::Debug,
) {
    let salt = egui::Id::new(salt);
    let font = egui::FontId::monospace(tk.code_size);
    let row_h = ui.ctx().fonts_mut(|f| f.row_height(&font)) + 2.0;
    egui::ScrollArea::both().id_salt(salt).auto_shrink([false, false]).show_rows(ui, row_h, lines.len(), |ui, range| {
        for l in lines.get(range).unwrap_or(&[]) {
            let (bg, fg, sign) = match l.kind {
                LineKind::Add => (tk.add_bg, tk.add_fg, "+"),
                LineKind::Del => (tk.del_bg, tk.del_fg, "-"),
                LineKind::Hunk => (tk.p.panel, tk.p.blue, ""),
                LineKind::Ctx => (egui::Color32::TRANSPARENT, tk.p.text, " "),
            };
            let w = ui.available_width().max(800.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, row_h), Sense::hover());
            let p = ui.painter();
            p.rect_filled(rect, 0.0, bg);
            let num = |n: Option<u32>| n.map_or(String::new(), |n| n.to_string());
            let y = rect.center().y;
            if l.kind == LineKind::Hunk {
                p.text(egui::pos2(rect.left() + 8.0, y), egui::Align2::LEFT_CENTER, &l.text, font.clone(), fg);
                continue;
            }
            p.text(egui::pos2(rect.left() + 40.0, y), egui::Align2::RIGHT_CENTER, num(l.old), font.clone(), tk.p.faint);
            p.text(egui::pos2(rect.left() + 84.0, y), egui::Align2::RIGHT_CENTER, num(l.new), font.clone(), tk.p.faint);
            p.text(
                egui::pos2(rect.left() + 94.0, y),
                egui::Align2::LEFT_CENTER,
                format!("{sign} {}", l.text.replace('\t', "    ")),
                font.clone(),
                fg,
            );
        }
    });
}

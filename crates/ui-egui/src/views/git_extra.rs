//! As janelas do cliente git além do Histórico: conflitos de rebase/merge,
//! staging por hunk e por linha, blame, histórico de um arquivo, comparação
//! de branches, PR/MR, rebase interativo, busca de commits, worktrees,
//! remotos e submódulos. Todas por comandos `git.*`/`github.*`/`gitlab.*`/`ai.*`.

use std::collections::BTreeSet;

use egui::{Align, Layout, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular as icon;
use lazyaicode_doc::diff::{parse_patch, DiffLine, LineKind};
use lazyaicode_i18n::{t, tf};
use lazyaicode_protocol::{ParamInfo, ParamType};
use serde_json::{json, Value};

use crate::app::{LazyApp, ToastKind};
use crate::views::widgets;

/// Uma janela aberta, com os dados dela.
pub enum Win {
    Blame { path: String, data: Option<Value> },
    FileHistory { path: String, data: Option<Value> },
    Compare { a: String, b: String, data: Option<Value> },
    Pr { title: String, body: String, base: String, head: String, busy: bool, result: Option<String> },
    Rebase { from: String, items: Vec<(Value, String, String)>, loaded: bool },
    Search { query: String, mode: String, results: Option<Value> },
    Worktrees { list: Option<Value>, path: String, branch: String },
    Remotes { list: Option<Value>, name: String, url: String },
    Submodules { list: Option<Value> },
}

impl Win {
    fn key(&self) -> &'static str {
        match self {
            Self::Blame { .. } => "blame",
            Self::FileHistory { .. } => "file-history",
            Self::Compare { .. } => "compare",
            Self::Pr { .. } => "pr",
            Self::Rebase { .. } => "rebase",
            Self::Search { .. } => "search",
            Self::Worktrees { .. } => "worktrees",
            Self::Remotes { .. } => "remotes",
            Self::Submodules { .. } => "submodules",
        }
    }
}

#[derive(Default)]
pub struct Extra {
    pub windows: Vec<(String, Win)>,
    /// conflitos do repositório da tela
    pub conflicts: Option<Value>,
    pub conflict_file: Option<String>,
    pub manual: String,
    pub conflicts_for: String,
    /// linhas escolhidas no diff de trabalho: (staged?, caminho) → {"o12", "n7"}
    pub picked: std::collections::HashMap<(bool, String), BTreeSet<String>>,
}

fn repo_id(app: &LazyApp) -> Option<String> {
    crate::views::git::current_repo_id(app)
}

fn path(app: &LazyApp) -> Option<String> {
    app.current_path()
}

pub fn open(app: &mut LazyApp, w: Win) {
    let Some(p) = path(app) else { return };
    app.eph.extra.windows.retain(|(q, x)| !(q == &p && x.key() == w.key()));
    app.eph.extra.windows.push((p, w));
}

fn set_win(app: &mut LazyApp, p: &str, key: &str, f: impl FnOnce(&mut Win)) {
    if let Some((_, w)) = app.eph.extra.windows.iter_mut().find(|(q, w)| q == p && w.key() == key) {
        f(w);
    }
}

// ---------- conflitos ----------

/// Há rebase/merge/cherry-pick/revert parado no meio?
pub fn in_progress(app: &LazyApp, p: &str) -> Option<String> {
    app.eph.git.repos.get(p).and_then(|r| r.status["inProgressOp"].as_str().map(str::to_string))
}

pub fn load_conflicts(app: &mut LazyApp, p: &str) {
    let Some(id) = repo_id(app) else { return };
    let pp = p.to_string();
    app.call("git.rebase.status", json!({ "repoId": id }), move |app, r| {
        app.eph.extra.conflicts_for = pp;
        app.eph.extra.conflicts = Some(r.unwrap_or_else(|e| json!({ "error": e.message })));
    });
}

pub fn conflicts(app: &mut LazyApp, ui: &mut egui::Ui, p: &str, op: &str) {
    let tk = app.tokens;
    if app.eph.extra.conflicts_for != p {
        app.eph.extra.conflicts = None;
        load_conflicts(app, p);
    }
    let Some(st) = app.eph.extra.conflicts.clone() else {
        ui.spinner();
        return;
    };
    let list = st["conflicts"].as_array().cloned().unwrap_or_default();
    egui::Frame::new().fill(tk.p.panel).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} {}", icon::WARNING, tf("gitRebase.inProgress", &[("op", &op)]))).color(tk.p.amber).strong());
            if let (Some(c), Some(n)) = (st["currentStep"].as_u64(), st["totalSteps"].as_u64().filter(|n| *n > 0)) {
                ui.label(widgets::muted(&tk, format!("{c}/{n}")));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::danger(ui, &tk, &t("gitRebase.abort")).clicked() {
                    let _ = crate::commands::run_ui(app, "ui.git.rebase-abort", &Value::Null);
                    app.eph.extra.conflicts = None;
                }
                let can = list.is_empty();
                if ui
                    .add_enabled(can, egui::Button::new(RichText::new(t("gitRebase.continue")).color(tk.on_accent)).fill(tk.p.accent))
                    .clicked()
                {
                    let _ = crate::commands::run_ui(app, "ui.git.rebase-continue", &Value::Null);
                    app.eph.extra.conflicts = None;
                }
                if ui.button(icon::ARROWS_CLOCKWISE).clicked() {
                    load_conflicts(app, p);
                }
            });
        });
        if list.is_empty() {
            ui.label(widgets::muted(&tk, t("gitRebase.noConflicts")));
            return;
        }
        ui.label(widgets::muted(&tk, tf("gitRebase.conflictsN", &[("n", &list.len())])));
        ui.horizontal_wrapped(|ui| {
            for c in &list {
                let fp = c["path"].as_str().unwrap_or("").to_string();
                let on = app.eph.extra.conflict_file.as_deref() == Some(fp.as_str());
                if ui.selectable_label(on, format!("{} {fp}", icon::WARNING_CIRCLE)).clicked() {
                    app.eph.extra.conflict_file = Some(fp.clone());
                    app.eph.extra.manual = c["oursContent"].as_str().unwrap_or("").to_string();
                }
            }
        });
    });
    let Some(fp) = app.eph.extra.conflict_file.clone() else { return };
    let Some(c) = list.iter().find(|c| c["path"] == fp.as_str()).cloned() else { return };
    let Some(id) = repo_id(app) else { return };
    let resolve = |app: &mut LazyApp, res: Value| {
        let pp = p.to_string();
        app.call_ok("git.rebase.resolve-conflict", json!({ "repoId": id, "path": fp, "resolution": res }), move |app, v| {
            app.eph.extra.conflicts = Some(v);
            app.eph.extra.conflict_file = None;
            crate::views::git::refresh(app, &pp, false);
        });
    };
    ui.horizontal(|ui| {
        if ui.button(format!("{}  {}", icon::ARROW_LEFT, t("gitRebase.useOurs"))).clicked() {
            resolve(app, json!({ "kind": "UseOurs" }));
        }
        if ui.button(format!("{}  {}", icon::ARROW_RIGHT, t("gitRebase.useTheirs"))).clicked() {
            resolve(app, json!({ "kind": "UseTheirs" }));
        }
        if ui.button(format!("{}  {}", icon::SPARKLE, t("gitAi.resolve"))).clicked() {
            let model = app.data.prefs.get("git.aiModel").and_then(Value::as_str).unwrap_or("sonnet").to_string();
            let params = json!({
                "model": model, "path": fp,
                "ancestor": c["ancestorContent"], "ours": c["oursContent"].as_str().unwrap_or(""),
                "theirs": c["theirsContent"].as_str().unwrap_or(""), "customInstructions": "",
            });
            app.toast(t("gitAi.working"), ToastKind::Info);
            app.call_ok("ai.resolve-conflict", params, |app, v| {
                app.eph.extra.manual = v.as_str().unwrap_or("").to_string();
            });
        }
    });
    if c["isBinary"] == true {
        ui.label(widgets::muted(&tk, t("diffFile.binary")));
        return;
    }
    ui.columns(2, |cols| {
        cols[0].label(RichText::new(t("gitRebase.ours")).strong());
        egui::ScrollArea::vertical().id_salt(("ours", &fp)).max_height(220.0).show(&mut cols[0], |ui| {
            ui.add(egui::Label::new(RichText::new(c["oursContent"].as_str().unwrap_or("")).monospace()).selectable(true));
        });
        cols[1].label(RichText::new(t("gitRebase.theirs")).strong());
        egui::ScrollArea::vertical().id_salt(("theirs", &fp)).max_height(220.0).show(&mut cols[1], |ui| {
            ui.add(egui::Label::new(RichText::new(c["theirsContent"].as_str().unwrap_or("")).monospace()).selectable(true));
        });
    });
    ui.label(RichText::new(t("gitRebase.manual")).strong());
    egui::ScrollArea::vertical().id_salt(("manual", &fp)).max_height(260.0).show(ui, |ui| {
        ui.add(egui::TextEdit::multiline(&mut app.eph.extra.manual).code_editor().desired_width(f32::INFINITY).desired_rows(10));
    });
    if widgets::primary(ui, &tk, &t("gitRebase.useManual")).clicked() {
        let content = app.eph.extra.manual.clone();
        resolve(app, json!({ "kind": "UseManualContent", "content": content }));
    }
}

// ---------- staging por hunk e linha ----------

/// Um hunk e as linhas mudadas dele: `(início, fim)` nas linhas do diff.
fn hunks(lines: &[DiffLine]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, l) in lines.iter().enumerate() {
        if l.kind == LineKind::Hunk {
            if let Some(s) = start.take() {
                out.push((s, i));
            }
            start = Some(i);
        }
    }
    if let Some(s) = start {
        out.push((s, lines.len()));
    }
    out
}

fn line_key(l: &DiffLine) -> Option<String> {
    match l.kind {
        LineKind::Del => l.old.map(|n| format!("o{n}")),
        LineKind::Add => l.new.map(|n| format!("n{n}")),
        _ => None,
    }
}

fn apply(app: &mut LazyApp, p: &str, file: &str, action: &str, lines: &[&DiffLine]) {
    let Some(id) = repo_id(app) else { return };
    let (mut old_lines, mut new_lines, mut old_texts, mut new_texts) = (vec![], vec![], vec![], vec![]);
    for l in lines {
        match (l.kind, l.old, l.new) {
            (LineKind::Del, Some(n), _) => {
                old_lines.push(n);
                old_texts.push(l.text.clone());
            }
            (LineKind::Add, _, Some(n)) => {
                new_lines.push(n);
                new_texts.push(l.text.clone());
            }
            _ => {}
        }
    }
    if old_lines.is_empty() && new_lines.is_empty() {
        return;
    }
    let pp = p.to_string();
    let params = json!({
        "repoId": id, "path": file, "action": action,
        "selection": { "oldLines": old_lines, "newLines": new_lines, "oldTexts": old_texts, "newTexts": new_texts },
    });
    app.call("git.working.apply-lines", params, move |app, r| {
        if let Err(e) = r {
            app.toast_err(&e.message);
        }
        app.eph.extra.picked.clear();
        crate::views::git::refresh(app, &pp, false);
    });
}

/// O diff de um arquivo das mudanças locais com botões por hunk e seleção de linhas.
pub fn staging_diff(app: &mut LazyApp, ui: &mut egui::Ui, p: &str, file: &str, staged: bool, patch: &str) {
    let tk = app.tokens;
    let lines = parse_patch(patch);
    let key = (staged, file.to_string());
    let picked = app.eph.extra.picked.get(&key).cloned().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(RichText::new(file).monospace().strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if !picked.is_empty() {
                let sel: Vec<&DiffLine> = lines.iter().filter(|l| line_key(l).is_some_and(|k| picked.contains(&k))).collect();
                if staged {
                    if ui.button(tf("gitWorking.unstageNLines", &[("n", &sel.len())])).clicked() {
                        apply(app, p, file, "unstage", &sel);
                    }
                } else {
                    if ui.button(RichText::new(tf("gitWorking.discardNLines", &[("n", &sel.len())])).color(tk.p.red)).clicked() {
                        apply(app, p, file, "discard", &sel);
                    }
                    if ui.button(tf("gitWorking.stageNLines", &[("n", &sel.len())])).clicked() {
                        apply(app, p, file, "stage", &sel);
                    }
                }
                if ui.small_button(t("gitWorking.clearSelection")).clicked() {
                    app.eph.extra.picked.remove(&key);
                }
            } else {
                ui.label(widgets::faint(&tk, t("gitWorking.pickLinesHint")));
            }
        });
    });
    let font = egui::FontId::monospace(tk.code_size);
    let row_h = ui.ctx().fonts_mut(|f| f.row_height(&font)) + 2.0;
    let hs = hunks(&lines);
    let mut toggle: Option<String> = None;
    let mut hunk_action: Option<(usize, &str)> = None;
    egui::ScrollArea::both().id_salt(("staging", file, staged)).auto_shrink([false, false]).show_rows(
        ui,
        row_h,
        lines.len(),
        |ui, range| {
            for i in range {
                let Some(l) = lines.get(i) else { continue };
                let w = ui.available_width().max(800.0);
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, row_h), Sense::click());
                let (bg, fg) = match l.kind {
                    LineKind::Add => (tk.add_bg, tk.add_fg),
                    LineKind::Del => (tk.del_bg, tk.del_fg),
                    LineKind::Hunk => (tk.p.panel, tk.p.blue),
                    LineKind::Ctx => (egui::Color32::TRANSPARENT, tk.p.text),
                };
                let k = line_key(l);
                let sel = k.as_ref().is_some_and(|k| picked.contains(k));
                let p2 = ui.painter_at(rect);
                p2.rect_filled(rect, 0.0, if sel { tk.accent_soft } else { bg });
                let y = rect.center().y;
                if l.kind == LineKind::Hunk {
                    p2.text(egui::pos2(rect.left() + 8.0, y), egui::Align2::LEFT_CENTER, &l.text, font.clone(), fg);
                    // botões do hunk
                    let mut bx = rect.left() + 8.0 + l.text.len() as f32 * (tk.code_size * 0.62) + 16.0;
                    let labels: &[(&str, &str)] = if staged {
                        &[("unstage", "gitWorking.unstageHunk")]
                    } else {
                        &[("stage", "gitWorking.stageHunk"), ("discard", "gitWorking.discardHunk")]
                    };
                    for (action, label) in labels {
                        let text = t(label);
                        let g = ui.painter().layout_no_wrap(text.clone(), egui::FontId::proportional(tk.text_size - 2.0), tk.p.text);
                        let br = egui::Rect::from_min_size(egui::pos2(bx, rect.top() + 1.0), g.size() + Vec2::new(10.0, 0.0));
                        let r = ui.interact(br, resp.id.with(action), Sense::click());
                        ui.painter().rect(
                            br,
                            egui::CornerRadius::same(3),
                            if r.hovered() { tk.p.hover } else { tk.p.elev },
                            Stroke::new(1.0, tk.p.border),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().galley(br.min + Vec2::new(5.0, 0.0), g, if *action == "discard" { tk.p.red } else { tk.p.text });
                        if r.clicked() {
                            hunk_action = Some((i, action));
                        }
                        bx = br.right() + 6.0;
                    }
                    continue;
                }
                let num = |n: Option<u32>| n.map_or(String::new(), |n| n.to_string());
                p2.text(egui::pos2(rect.left() + 40.0, y), egui::Align2::RIGHT_CENTER, num(l.old), font.clone(), tk.p.faint);
                p2.text(egui::pos2(rect.left() + 84.0, y), egui::Align2::RIGHT_CENTER, num(l.new), font.clone(), tk.p.faint);
                let sign = match l.kind {
                    LineKind::Add => "+",
                    LineKind::Del => "-",
                    _ => " ",
                };
                p2.text(
                    egui::pos2(rect.left() + 94.0, y),
                    egui::Align2::LEFT_CENTER,
                    format!("{sign} {}", l.text.replace('\t', "    ")),
                    font.clone(),
                    fg,
                );
                if resp.clicked() {
                    toggle = k;
                }
            }
        },
    );
    if let Some(k) = toggle {
        let set = app.eph.extra.picked.entry(key.clone()).or_default();
        if !set.remove(&k) {
            set.insert(k);
        }
    }
    if let Some((i, action)) = hunk_action {
        if let Some((s, e)) = hs.iter().find(|(s, _)| *s == i) {
            let sel: Vec<&DiffLine> = lines.get(*s..*e).unwrap_or(&[]).iter().collect();
            apply(app, p, file, action, &sel);
        }
    }
}

// ---------- janelas ----------

fn remote_url(app: &LazyApp, p: &str) -> Option<String> {
    let r = app.eph.git.repos.get(p)?;
    let pick = r.remotes.iter().find(|x| x["name"] == "origin").or_else(|| r.remotes.first())?;
    pick["fetchUrl"].as_str().map(str::to_string)
}

/// `git@github.com:dono/repo.git` / `https://gitlab.com/g/s/repo` → (host, caminho).
pub fn parse_remote(url: &str) -> Option<(String, String)> {
    let u = url.trim().trim_end_matches('/').trim_end_matches(".git");
    let rest = if let Some(r) = u.strip_prefix("git@") {
        r.replacen(':', "/", 1)
    } else {
        let i = u.find("://")?;
        let r = u.get(i + 3..)?;
        r.split_once('@').map_or(r, |(_, h)| h).to_string()
    };
    let (host, path) = rest.split_once('/')?;
    (!path.is_empty()).then(|| (host.split(':').next().unwrap_or(host).to_string(), path.to_string()))
}

pub fn show_windows(app: &mut LazyApp, ctx: &egui::Context) {
    let tk = app.tokens;
    let wins = std::mem::take(&mut app.eph.extra.windows);
    let mut keep = Vec::new();
    for (p, mut w) in wins {
        let mut open = true;
        let title = match &w {
            Win::Blame { path, .. } => format!("{}  {} — {path}", icon::USER_LIST, t("gitHistory.blame")),
            Win::FileHistory { path, .. } => format!("{}  {} — {path}", icon::CLOCK_COUNTER_CLOCKWISE, t("gitHistory.fileHistory")),
            Win::Compare { a, b, .. } => format!("{}  {a} ↔ {b}", icon::GIT_DIFF),
            Win::Pr { .. } => format!("{}  {}", icon::GIT_PULL_REQUEST, t("gitIntegrations.newPr")),
            Win::Rebase { .. } => format!("{}  {}", icon::LIST_NUMBERS, t("gitInteractiveRebase.title")),
            Win::Search { .. } => format!("{}  {}", icon::MAGNIFYING_GLASS, t("gitGraph.search")),
            Win::Worktrees { .. } => format!("{}  {}", icon::TREE_STRUCTURE, t("gitWorktrees.title")),
            Win::Remotes { .. } => format!("{}  {}", icon::CLOUD, t("gitRemote.remotes")),
            Win::Submodules { .. } => format!("{}  {}", icon::CUBE, t("gitSubmodules.title")),
        };
        egui::Window::new(title).id(egui::Id::new(("git-win", &p, w.key()))).open(&mut open).default_size(Vec2::new(760.0, 520.0)).show(
            ctx,
            |ui| {
                window_body(app, ui, &p, &mut w);
            },
        );
        if open {
            keep.push((p, w));
        }
    }
    // janelas abertas durante o desenho (ex.: blame a partir do histórico) entram também
    keep.append(&mut app.eph.extra.windows);
    app.eph.extra.windows = keep;
    let _ = tk;
}

fn window_body(app: &mut LazyApp, ui: &mut egui::Ui, p: &str, w: &mut Win) {
    let tk = app.tokens;
    let Some(id) = app.eph.git.repos.get(p).and_then(|r| r.id.clone()) else {
        ui.label(widgets::muted(&tk, t("gitPanel.noRepo")));
        return;
    };
    match w {
        Win::Blame { path, data } => {
            if data.is_none() {
                *data = Some(json!({ "loading": true }));
                let (pp, f) = (p.to_string(), path.clone());
                app.call("git.history.blame", json!({ "repoId": id, "path": path, "startLine": 1, "maxLines": 5000 }), move |app, r| {
                    let v = r.unwrap_or_else(|e| json!({ "error": e.message }));
                    set_win(app, &pp, "blame", |w| {
                        if let Win::Blame { data, path } = w {
                            if *path == f {
                                *data = Some(v);
                            }
                        }
                    });
                });
            }
            let Some(d) = data.clone() else { return };
            if let Some(e) = d["error"].as_str() {
                ui.label(RichText::new(e).color(tk.p.red));
                return;
            }
            let lines = d["lines"].as_array().cloned().unwrap_or_default();
            let hunks = d["hunks"].as_array().cloned().unwrap_or_default();
            let commits = d["commits"].as_array().cloned().unwrap_or_default();
            let start = d["startLine"].as_u64().unwrap_or(1);
            let author_of = |line: u64| -> (String, String, String) {
                let h = hunks.iter().find(|h| {
                    let s = h["startLine"].as_u64().unwrap_or(0);
                    line >= s && line < s + h["lineCount"].as_u64().unwrap_or(0)
                });
                let sha = h.and_then(|h| h["sha"].as_str()).unwrap_or("").to_string();
                let c = commits.iter().find(|c| c["sha"] == sha.as_str());
                (
                    sha.chars().take(8).collect(),
                    c.and_then(|c| c["authorName"].as_str()).unwrap_or("").to_string(),
                    c.and_then(|c| c["summary"].as_str()).unwrap_or("").to_string(),
                )
            };
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new(("blame-grid", p)).num_columns(4).spacing([10.0, 0.0]).show(ui, |ui| {
                    let mut last = String::new();
                    for (i, l) in lines.iter().enumerate() {
                        let n = start + i as u64;
                        let (sha, author, summary) = author_of(n);
                        if sha != last {
                            ui.label(RichText::new(&sha).monospace().color(tk.p.blue)).on_hover_text(&summary);
                            ui.label(widgets::muted(&tk, &author));
                            last = sha;
                        } else {
                            ui.label("");
                            ui.label("");
                        }
                        ui.label(RichText::new(n.to_string()).monospace().color(tk.p.faint));
                        ui.label(RichText::new(l.as_str().unwrap_or("")).monospace());
                        ui.end_row();
                    }
                });
            });
        }
        Win::FileHistory { path, data } => {
            if data.is_none() {
                *data = Some(json!({ "loading": true }));
                let (pp, f) = (p.to_string(), path.clone());
                app.call(
                    "git.history.file",
                    json!({ "repoId": id, "query": { "path": path, "startSha": null, "cursor": null, "limit": 200 } }),
                    move |app, r| {
                        let v = r.unwrap_or_else(|e| json!({ "error": e.message }));
                        set_win(app, &pp, "file-history", |w| {
                            if let Win::FileHistory { data, path } = w {
                                if *path == f {
                                    *data = Some(v);
                                }
                            }
                        });
                    },
                );
            }
            let Some(d) = data.clone() else { return };
            if let Some(e) = d["error"].as_str() {
                ui.label(RichText::new(e).color(tk.p.red));
                return;
            }
            let now = app.host.now_ms();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for e in d["entries"].as_array().into_iter().flatten() {
                    let c = &e["commit"];
                    let sha = c["sha"].as_str().unwrap_or("").to_string();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(c["shortSha"].as_str().unwrap_or("")).monospace().color(tk.p.blue));
                        if ui.link(c["summary"].as_str().unwrap_or("")).clicked() {
                            let _ = crate::commands::run_ui(app, "ui.git.select-commit", &json!({ "sha": sha }));
                        }
                        ui.label(widgets::faint(
                            &tk,
                            format!(
                                "{} · {}",
                                c["authorName"].as_str().unwrap_or(""),
                                widgets::rel_time(c["authorDate"].as_i64().unwrap_or(0) * 1000, now)
                            ),
                        ));
                        if let Some(old) = e["oldPath"].as_str() {
                            widgets::chip(ui, &tk, &format!("← {old}"), tk.p.amber);
                        }
                    });
                }
            });
        }
        Win::Compare { a, b, data } => {
            if data.is_none() {
                *data = Some(json!({ "loading": true }));
                let pp = p.to_string();
                app.call("git.history.compare", json!({ "repoId": id, "a": a, "b": b, "limit": 300 }), move |app, r| {
                    let v = r.unwrap_or_else(|e| json!({ "error": e.message }));
                    set_win(app, &pp, "compare", |w| {
                        if let Win::Compare { data, .. } = w {
                            *data = Some(v);
                        }
                    });
                });
            }
            let Some(d) = data.clone() else { return };
            if let Some(e) = d["error"].as_str() {
                ui.label(RichText::new(e).color(tk.p.red));
                return;
            }
            ui.columns(2, |cols| {
                for (col, key, total, name) in [(0usize, "onlyInA", "onlyInATotal", a.clone()), (1, "onlyInB", "onlyInBTotal", b.clone())] {
                    let ui = &mut cols[col];
                    ui.label(RichText::new(tf("gitHistory.onlyInN", &[("name", &name), ("n", &d[total].as_u64().unwrap_or(0))])).strong());
                    egui::ScrollArea::vertical().id_salt(("cmp", col)).show(ui, |ui| {
                        for c in d[key].as_array().into_iter().flatten() {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(c["shortSha"].as_str().unwrap_or("")).monospace().color(tk.p.blue));
                                ui.label(c["summary"].as_str().unwrap_or(""));
                            });
                        }
                    });
                }
            });
        }
        Win::Pr { title, body, base, head, busy, result } => {
            if let Some(url) = result.clone() {
                ui.label(RichText::new(format!("{} {}", icon::CHECK_CIRCLE, t("gitIntegrations.prCreated"))).color(tk.p.accent));
                ui.hyperlink(url);
                return;
            }
            let remote = remote_url(app, p).and_then(|u| parse_remote(&u));
            let Some((host, project)) = remote else {
                ui.label(widgets::muted(&tk, t("gitIntegrations.noRemote")));
                return;
            };
            let gitlab = host.contains("gitlab");
            ui.label(widgets::muted(&tk, format!("{host} · {project}")));
            egui::Grid::new("pr-grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label(t("gitIntegrations.from"));
                ui.text_edit_singleline(head);
                ui.end_row();
                ui.label(t("gitIntegrations.into"));
                ui.text_edit_singleline(base);
                ui.end_row();
                ui.label(t("gitIntegrations.prTitle"));
                ui.add(egui::TextEdit::singleline(title).desired_width(480.0));
                ui.end_row();
            });
            ui.add(egui::TextEdit::multiline(body).desired_rows(8).desired_width(f32::INFINITY).hint_text(t("gitIntegrations.prBody")));
            ui.horizontal(|ui| {
                if ui.add_enabled(!*busy, egui::Button::new(format!("{} {}", icon::SPARKLE, t("gitAi.prDescription")))).clicked() {
                    *busy = true;
                    let (pp, h, b) = (p.to_string(), head.clone(), base.clone());
                    let model = app.data.prefs.get("git.aiModel").and_then(Value::as_str).unwrap_or("haiku").to_string();
                    let rid = id.clone();
                    app.call("git.history.compare", json!({ "repoId": rid, "a": h, "b": b, "limit": 100 }), move |app, r| {
                        let commits: String = r
                            .ok()
                            .and_then(|v| v["onlyInA"].as_array().cloned())
                            .unwrap_or_default()
                            .iter()
                            .map(|c| format!("- {}", c["summary"].as_str().unwrap_or("")))
                            .collect::<Vec<_>>()
                            .join("\n");
                        let pp2 = pp.clone();
                        app.call("ai.pr-description", json!({ "model": model, "commits": commits, "diff": "" }), move |app, r| {
                            set_win(app, &pp2, "pr", |w| {
                                if let Win::Pr { title, body, busy, .. } = w {
                                    *busy = false;
                                    if let Ok(v) = &r {
                                        *title = v["title"].as_str().unwrap_or(title).to_string();
                                        *body = v["description"].as_str().or_else(|| v["body"].as_str()).unwrap_or("").to_string();
                                    }
                                }
                            });
                            if let Err(e) = r {
                                app.toast_err(&e.message);
                            }
                        });
                    });
                }
                let prefix = if gitlab { "gitlab" } else { "github" };
                let token = app.data.prefs.get(&format!("{prefix}.token")).and_then(Value::as_str).unwrap_or("").to_string();
                let base_url = app
                    .data
                    .prefs
                    .get(&format!("{prefix}.url"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| if gitlab { format!("https://{host}") } else { "https://api.github.com".into() });
                let can = !token.is_empty() && !title.trim().is_empty() && !*busy;
                if token.is_empty() {
                    ui.label(widgets::faint(&tk, t("gitIntegrations.needsToken")));
                }
                if ui.add_enabled(can, egui::Button::new(RichText::new(t("gitIntegrations.createPr")).color(tk.on_accent)).fill(tk.p.accent)).clicked() {
                    *busy = true;
                    let (method, params) = if gitlab {
                        ("gitlab.mr.create", json!({ "baseUrl": base_url, "token": token, "projectPath": project, "sourceBranch": head, "targetBranch": base, "title": title, "description": body, "reviewerIds": [], "removeSourceBranch": false, "squash": false }))
                    } else {
                        ("github.pr.create", json!({ "baseUrl": base_url, "token": token, "projectPath": project, "sourceBranch": head, "targetBranch": base, "title": title, "description": body, "reviewers": "" }))
                    };
                    let pp = p.to_string();
                    app.call(method, params, move |app, r| {
                        let url = r.as_ref().ok().and_then(|v| v["webUrl"].as_str().map(str::to_string));
                        if let Err(e) = &r {
                            app.toast_err(&e.message);
                        }
                        set_win(app, &pp, "pr", |w| {
                            if let Win::Pr { busy, result, .. } = w {
                                *busy = false;
                                *result = url;
                            }
                        });
                    });
                }
            });
        }
        Win::Rebase { from, items, loaded } => {
            if !*loaded {
                *loaded = true;
                let pp = p.to_string();
                app.call("git.irebase.plan", json!({ "repoId": id, "fromSha": from }), move |app, r| match r {
                    Ok(v) => set_win(app, &pp, "rebase", |w| {
                        if let Win::Rebase { items, .. } = w {
                            *items = v["commits"]
                                .as_array()
                                .cloned()
                                .unwrap_or_default()
                                .into_iter()
                                .map(|c| {
                                    let msg = c["summary"].as_str().unwrap_or("").to_string();
                                    (c, "pick".to_string(), msg)
                                })
                                .collect();
                        }
                    }),
                    Err(e) => app.toast_err(&e.message),
                });
            }
            ui.label(widgets::muted(&tk, t("gitInteractiveRebase.hintArrows")));
            let mut swap: Option<(usize, usize)> = None;
            let n = items.len();
            egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                for (i, (c, action, msg)) in items.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        if ui.add_enabled(i > 0, egui::Button::new(icon::ARROW_UP).small()).clicked() {
                            swap = Some((i, i - 1));
                        }
                        if ui.add_enabled(i + 1 < n, egui::Button::new(icon::ARROW_DOWN).small()).clicked() {
                            swap = Some((i, i + 1));
                        }
                        egui::ComboBox::from_id_salt(("todo", i)).width(90.0).selected_text(action.as_str()).show_ui(ui, |ui| {
                            for a in ["pick", "reword", "squash", "fixup", "drop"] {
                                ui.selectable_value(action, a.to_string(), a);
                            }
                        });
                        ui.label(RichText::new(c["shortSha"].as_str().unwrap_or("")).monospace().color(tk.p.blue));
                        if action == "reword" {
                            ui.add(egui::TextEdit::singleline(msg).desired_width(360.0));
                        } else {
                            let s = c["summary"].as_str().unwrap_or("");
                            ui.label(if action == "drop" { RichText::new(s).strikethrough().color(tk.p.faint) } else { RichText::new(s) });
                        }
                    });
                }
            });
            if let Some((a, b)) = swap {
                items.swap(a, b);
            }
            if !items.is_empty() && widgets::primary(ui, &tk, &t("gitInteractiveRebase.start")).clicked() {
                let todo: Vec<Value> = items
                    .iter()
                    .map(|(c, a, m)| json!({ "sha": c["sha"], "action": a, "message": if a == "reword" { json!(m) } else { Value::Null } }))
                    .collect();
                let pp = p.to_string();
                app.call("git.irebase.start", json!({ "repoId": id, "items": todo }), move |app, r| {
                    match r {
                        Ok(_) => app.toast(t("gitInteractiveRebase.done"), ToastKind::Ok),
                        Err(e) => app.toast_err(&e.message),
                    }
                    app.eph.extra.windows.retain(|(q, w)| !(q == &pp && w.key() == "rebase"));
                    crate::views::git::refresh(app, &pp, true);
                });
            }
        }
        Win::Search { query, mode, results } => {
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(query).hint_text(t("gitGraph.searchHint")).desired_width(320.0));
                egui::ComboBox::from_id_salt("search-mode").selected_text(mode.as_str()).show_ui(ui, |ui| {
                    for m in ["message", "author", "sha", "file", "diff"] {
                        ui.selectable_value(mode, m.to_string(), m);
                    }
                });
                if (ui.button(icon::MAGNIFYING_GLASS).clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                    && !query.trim().is_empty()
                {
                    let pp = p.to_string();
                    app.call("git.log.search", json!({ "repoId": id, "query": query, "mode": mode }), move |app, r| {
                        let v = r.unwrap_or_else(|e| json!({ "error": e.message }));
                        set_win(app, &pp, "search", |w| {
                            if let Win::Search { results, .. } = w {
                                *results = Some(v);
                            }
                        });
                    });
                }
            });
            if let Some(v) = results.clone() {
                if let Some(e) = v["error"].as_str() {
                    ui.label(RichText::new(e).color(tk.p.red));
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let list = v.as_array().cloned().or_else(|| v["commits"].as_array().cloned()).unwrap_or_default();
                    if list.is_empty() && v["error"].is_null() {
                        ui.label(widgets::faint(&tk, t("palette.empty")));
                    }
                    for c in list {
                        let sha = c["sha"].as_str().unwrap_or("").to_string();
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(c["shortSha"].as_str().unwrap_or("")).monospace().color(tk.p.blue));
                            if ui.link(c["summary"].as_str().unwrap_or("")).clicked() {
                                let _ = crate::commands::run_ui(app, "ui.git.select-commit", &json!({ "sha": sha }));
                            }
                            ui.label(widgets::faint(&tk, c["authorName"].as_str().unwrap_or("")));
                        });
                    }
                });
            }
        }
        Win::Worktrees { list, path, branch } => {
            if list.is_none() {
                *list = Some(json!([]));
                let pp = p.to_string();
                app.call("git.worktree.list", json!({ "repoId": id }), move |app, r| {
                    let v = r.unwrap_or_else(|e| json!({ "error": e.message }));
                    set_win(app, &pp, "worktrees", |w| {
                        if let Win::Worktrees { list, .. } = w {
                            *list = Some(v);
                        }
                    });
                });
            }
            let l = list.clone().unwrap_or_default();
            for wt in l.as_array().into_iter().flatten() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(wt["branch"].as_str().unwrap_or("(detached)")).strong());
                    ui.label(widgets::muted(&tk, widgets::short_path(wt["path"].as_str().unwrap_or(""))));
                    if wt["isMain"] == true {
                        widgets::chip(ui, &tk, "main", tk.p.muted);
                    }
                    if wt["isMain"] != true {
                        if let Some(name) = wt["name"].as_str() {
                            if ui.small_button(RichText::new(icon::TRASH).color(tk.p.red)).clicked() {
                                let pp = p.to_string();
                                app.call_ok("git.worktree.remove", json!({ "repoId": id, "name": name, "force": false }), move |app, _| {
                                    set_win(app, &pp, "worktrees", |w| {
                                        if let Win::Worktrees { list, .. } = w {
                                            *list = None;
                                        }
                                    });
                                });
                            }
                        }
                    }
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(path).hint_text(t("gitWorktrees.path")).desired_width(280.0));
                ui.add(egui::TextEdit::singleline(branch).hint_text(t("gitWorktrees.branch")).desired_width(160.0));
                if ui.button(t("gitWorktrees.add")).clicked() && !path.is_empty() && !branch.is_empty() {
                    let pp = p.to_string();
                    let args = json!({ "path": path, "branch": branch, "newBranch": true, "startPoint": null });
                    app.call_ok("git.worktree.add", json!({ "repoId": id, "args": args }), move |app, _| {
                        set_win(app, &pp, "worktrees", |w| {
                            if let Win::Worktrees { list, .. } = w {
                                *list = None;
                            }
                        });
                    });
                }
            });
            if ui.button(t("gitWorktrees.prune")).clicked() {
                app.fire("git.worktree.prune", json!({ "repoId": id }));
            }
        }
        Win::Remotes { list, name, url } => {
            if list.is_none() {
                *list = Some(json!([]));
                let pp = p.to_string();
                app.call("git.remote.list", json!({ "repoId": id }), move |app, r| {
                    let v = r.unwrap_or_else(|e| json!({ "error": e.message }));
                    set_win(app, &pp, "remotes", |w| {
                        if let Win::Remotes { list, .. } = w {
                            *list = Some(v);
                        }
                    });
                });
            }
            let reload = |app: &mut LazyApp, pp: String| {
                set_win(app, &pp, "remotes", |w| {
                    if let Win::Remotes { list, .. } = w {
                        *list = None;
                    }
                });
                crate::views::git::refresh(app, &pp, false);
            };
            for r in list.clone().unwrap_or_default().as_array().into_iter().flatten() {
                let rname = r["name"].as_str().unwrap_or("").to_string();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&rname).strong());
                    ui.label(widgets::muted(&tk, r["fetchUrl"].as_str().unwrap_or("")));
                    if ui.small_button(icon::ARROWS_DOWN_UP).on_hover_text(t("gitRemote.fetch")).clicked() {
                        app.fire("git.remote.fetch", json!({ "repoId": id, "remoteName": rname }));
                    }
                    if ui.small_button(RichText::new(icon::TRASH).color(tk.p.red)).clicked() {
                        let pp = p.to_string();
                        app.call_ok("git.remote.remove", json!({ "repoId": id, "name": rname }), move |app, _| reload(app, pp));
                    }
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(name).hint_text("origin").desired_width(100.0));
                ui.add(egui::TextEdit::singleline(url).hint_text("git@github.com:dono/repo.git").desired_width(320.0));
                if ui.button(t("gitRemote.add")).clicked() && !name.is_empty() && !url.is_empty() {
                    let pp = p.to_string();
                    app.call_ok("git.remote.add", json!({ "repoId": id, "name": name, "url": url }), move |app, _| reload(app, pp));
                    name.clear();
                    url.clear();
                }
            });
        }
        Win::Submodules { list } => {
            if list.is_none() {
                *list = Some(json!([]));
                let pp = p.to_string();
                app.call("git.submodule.list", json!({ "repoId": id }), move |app, r| {
                    let v = r.unwrap_or_else(|e| json!({ "error": e.message }));
                    set_win(app, &pp, "submodules", |w| {
                        if let Win::Submodules { list } = w {
                            *list = Some(v);
                        }
                    });
                });
            }
            let l = list.clone().unwrap_or_default();
            let items = l.as_array().cloned().unwrap_or_default();
            if items.is_empty() {
                ui.label(widgets::faint(&tk, t("gitSubmodules.none")));
            }
            for s in &items {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(s["path"].as_str().unwrap_or("")).strong());
                    let ok = s["expectedSha"] == s["currentSha"];
                    widgets::chip(ui, &tk, if ok { "ok" } else { "≠" }, if ok { tk.p.accent } else { tk.p.amber });
                    ui.label(widgets::muted(&tk, s["url"].as_str().unwrap_or("")));
                });
            }
            if !items.is_empty() && ui.button(t("gitSubmodules.update")).clicked() {
                let paths: Vec<String> = items.iter().filter_map(|s| s["path"].as_str().map(str::to_string)).collect();
                app.fire("git.submodule.update", json!({ "repoId": id, "paths": paths.join("\n") }));
            }
        }
    }
}

/// Os comandos de tela destas janelas.
pub fn register(add: &mut crate::views::git::AddUiCmd<'_>) {
    let p_of = |app: &LazyApp| {
        app.current_path().filter(|p| app.eph.git.repos.get(p).is_some_and(|r| r.id.is_some())).ok_or_else(|| t("gitPanel.noRepo"))
    };
    let file = |p: &Value| p.get("path").and_then(Value::as_str).map(str::to_string).ok_or_else(|| "falta `path`".to_string());
    add(
        "ui.git.blame",
        "gitHistory.blame",
        "git",
        None,
        vec![ParamInfo::req("path", ParamType::String)],
        Box::new(move |app, p| {
            p_of(app)?;
            open(app, Win::Blame { path: file(p)?, data: None });
            Ok(Value::Null)
        }),
    );
    add(
        "ui.git.file-history",
        "gitHistory.fileHistory",
        "git",
        None,
        vec![ParamInfo::req("path", ParamType::String)],
        Box::new(move |app, p| {
            p_of(app)?;
            open(app, Win::FileHistory { path: file(p)?, data: None });
            Ok(Value::Null)
        }),
    );
    add(
        "ui.git.compare",
        "gitHistory.compare",
        "git",
        None,
        vec![ParamInfo::opt("a", ParamType::String).doc("padrão: a branch atual"), ParamInfo::req("b", ParamType::String)],
        Box::new(move |app, p| {
            let path = p_of(app)?;
            let cur = app
                .eph
                .git
                .repos
                .get(&path)
                .and_then(|r| r.status["currentBranch"].as_str().map(str::to_string))
                .unwrap_or_else(|| "HEAD".into());
            let a = p.get("a").and_then(Value::as_str).map(str::to_string).unwrap_or(cur);
            let b = p.get("b").and_then(Value::as_str).map(str::to_string).ok_or("falta `b`")?;
            open(app, Win::Compare { a, b, data: None });
            Ok(Value::Null)
        }),
    );
    add(
        "ui.git.open-pr",
        "gitIntegrations.newPr",
        "git",
        None,
        vec![],
        Box::new(move |app, _| {
            let path = p_of(app)?;
            let head =
                app.eph.git.repos.get(&path).and_then(|r| r.status["currentBranch"].as_str().map(str::to_string)).unwrap_or_default();
            let title = app.selected().map(|t| t.thread.title.clone()).unwrap_or_default();
            open(app, Win::Pr { title, body: String::new(), base: "main".into(), head, busy: false, result: None });
            Ok(Value::Null)
        }),
    );
    add(
        "ui.git.interactive-rebase",
        "gitInteractiveRebase.title",
        "git",
        None,
        vec![ParamInfo::opt("sha", ParamType::String).doc("padrão: o commit selecionado")],
        Box::new(move |app, p| {
            let path = p_of(app)?;
            let from = p
                .get("sha")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| app.eph.git.repos.get(&path).and_then(|r| r.selected.clone()).filter(|s| !s.starts_with('\0')))
                .ok_or_else(|| t("gitGraph.selectCommit"))?;
            open(app, Win::Rebase { from, items: vec![], loaded: false });
            Ok(Value::Null)
        }),
    );
    add(
        "ui.git.search",
        "gitGraph.search",
        "git",
        Some("Ctrl+Shift+H"),
        vec![ParamInfo::opt("query", ParamType::String)],
        Box::new(move |app, p| {
            p_of(app)?;
            let query = p.get("query").and_then(Value::as_str).unwrap_or("").to_string();
            open(app, Win::Search { query, mode: "message".into(), results: None });
            Ok(Value::Null)
        }),
    );
    for (id, label) in
        [("ui.git.worktrees", "gitWorktrees.title"), ("ui.git.remotes", "gitRemote.remotes"), ("ui.git.submodules", "gitSubmodules.title")]
    {
        add(
            id,
            label,
            "git",
            None,
            vec![],
            Box::new(move |app, _| {
                p_of(app)?;
                open(
                    app,
                    match id {
                        "ui.git.worktrees" => Win::Worktrees { list: None, path: String::new(), branch: String::new() },
                        "ui.git.remotes" => Win::Remotes { list: None, name: String::new(), url: String::new() },
                        _ => Win::Submodules { list: None },
                    },
                );
                Ok(Value::Null)
            }),
        );
    }
    add(
        "ui.git.conflicts.refresh",
        "Reload conflicts",
        "git",
        None,
        vec![],
        Box::new(move |app, _| {
            let path = p_of(app)?;
            load_conflicts(app, &path);
            Ok(Value::Null)
        }),
    );
    add(
        "ui.git.lfs-pull",
        "gitLfs.pull",
        "git",
        None,
        vec![],
        Box::new(move |app, _| {
            p_of(app)?;
            let id = repo_id(app).unwrap_or_default();
            app.call_ok("git.lfs.pull", json!({ "repoId": id }), |app, _| app.toast("✓ LFS", ToastKind::Ok));
            Ok(Value::Null)
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_owner_and_repo_from_remote_urls() {
        assert_eq!(parse_remote("git@github.com:dono/repo.git"), Some(("github.com".into(), "dono/repo".into())));
        assert_eq!(parse_remote("https://gitlab.com/grupo/sub/repo"), Some(("gitlab.com".into(), "grupo/sub/repo".into())));
        assert_eq!(parse_remote("https://user:tok@github.com/a/b.git/"), Some(("github.com".into(), "a/b".into())));
        assert_eq!(parse_remote("ssh://git@gitlab.empresa.com:2222/x/y.git"), Some(("gitlab.empresa.com".into(), "x/y".into())));
        assert_eq!(parse_remote("/caminho/local"), None);
    }

    #[test]
    fn splits_hunks() {
        let l = parse_patch("@@ -1,2 +1,2 @@\n a\n-b\n+c\n@@ -10 +10 @@\n-x\n+y");
        assert_eq!(hunks(&l), vec![(0, 4), (4, 7)]);
    }
}

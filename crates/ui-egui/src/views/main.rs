//! A área principal: as sessões abertas do projeto em abas, o cabeçalho da
//! thread (ou do repositório), as abas da thread e o conteúdo da aberta.

use egui::{Align, Layout, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular as icon;
use lazyaicode_doc::Status;
use lazyaicode_i18n::t;
use serde_json::json;

use crate::app::{LazyApp, ToastKind};
use crate::state::{HistoryView, MainTab, MAIN_TABS};
use crate::views::widgets;

pub fn show(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    let frame = egui::Frame::new().fill(tk.p.bg);
    egui::CentralPanel::default().frame(frame).show(ui, |ui| {
        if let Some(th) = app.selected().cloned() {
            session_tabs(app, ui, &th.thread.project_id);
            thread_header(app, ui, &th);
            widgets::hline(ui, &tk);
            tabs_bar(app, ui, true);
            content(app, ui, Some(&th));
        } else if let Some(p) = app.current_project().cloned() {
            session_tabs(app, ui, &p.id);
            repo_header(app, ui, &p);
            widgets::hline(ui, &tk);
            tabs_bar(app, ui, false);
            content(app, ui, None);
        } else {
            welcome(app, ui);
        }
    });
}

/// O que a barra de abas das sessões pediu neste quadro.
enum TabAction {
    Open(String),
    Close(String),
    CloseOthers(String),
}

/// As sessões abertas do projeto, lado a lado; some quando não há nenhuma
/// (a tela do repositório sem sessões abertas). Cada aba tem o ponto de
/// status, o título e o "x" (na aberta ou sob o mouse); o clique abre, o
/// botão do meio fecha e o menu fecha as outras.
fn session_tabs(app: &mut LazyApp, ui: &mut egui::Ui, project_id: &str) {
    // a aberta sempre tem aba, mesmo quando chegou por outro caminho (`ui.state.set`)
    if let Some(sel) = app.state.selected.clone() {
        app.open_tab(&sel);
    }
    let tabs = app.project_tabs(project_id);
    if tabs.is_empty() {
        return;
    }
    let tk = app.tokens;
    let time = ui.input(|i| i.time);
    let mut action = None;
    egui::Frame::new().fill(tk.p.panel).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        egui::ScrollArea::horizontal()
            .id_salt("session-tabs")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for id in &tabs {
                        let Some(th) = app.data.thread(id) else { continue };
                        let on = app.state.selected.as_deref() == Some(id.as_str());
                        let (title, cwd, status, unread) = (th.thread.title.clone(), th.thread.cwd.clone(), th.status, th.thread.unread);
                        let font = egui::FontId::proportional(tk.text_size - 0.5);
                        let mut job =
                            egui::text::LayoutJob::simple_singleline(title.clone(), font, if on { tk.p.text } else { tk.p.muted });
                        job.wrap = egui::text::TextWrapping::truncate_at_width(170.0);
                        let g = ui.painter().layout_job(job);
                        let (rect, r) = ui.allocate_exact_size(Vec2::new(g.size().x + 64.0, 32.0), Sense::click());
                        let close_rect = egui::Rect::from_center_size(egui::pos2(rect.right() - 17.0, rect.center().y), Vec2::splat(18.0));
                        let close = ui.interact(close_rect, egui::Id::new(("session-tab-close", id)), Sense::click());
                        let hovered = r.hovered() || close.hovered();
                        let p = ui.painter();
                        if on {
                            p.rect_filled(rect, 0.0, tk.p.bg);
                            p.hline(rect.x_range(), rect.top() + 1.0, Stroke::new(2.0, tk.p.accent));
                        } else if hovered {
                            p.rect_filled(rect, 0.0, tk.p.hover);
                        }
                        p.vline(rect.right() - 0.5, rect.y_range(), Stroke::new(1.0, tk.p.border));
                        p.galley(egui::pos2(rect.left() + 30.0, rect.center().y - g.size().y / 2.0), g, tk.p.text);
                        if on || hovered {
                            if close.hovered() {
                                p.rect_filled(close_rect, egui::CornerRadius::same(4), tk.p.active);
                            }
                            let c = if close.hovered() { tk.p.text } else { tk.p.muted };
                            p.text(close_rect.center(), egui::Align2::CENTER_CENTER, icon::X, egui::FontId::proportional(11.0), c);
                        }
                        let dot = egui::Rect::from_min_size(egui::pos2(rect.left() + 10.0, rect.center().y - 7.0), Vec2::splat(14.0));
                        let mut child = ui.new_child(egui::UiBuilder::new().id_salt(("session-tab-dot", id)).max_rect(dot));
                        widgets::status_dot(&mut child, &tk, status, unread, time);
                        if close.clicked() || r.middle_clicked() {
                            action = Some(TabAction::Close(id.clone()));
                        } else if r.clicked() && !on {
                            action = Some(TabAction::Open(id.clone()));
                        }
                        r.on_hover_text(format!("{title}\n{}", widgets::short_path(&cwd))).context_menu(|ui| {
                            if ui.button(format!("{}  {}", icon::X, t("sessionTabs.close"))).clicked() {
                                action = Some(TabAction::Close(id.clone()));
                                ui.close();
                            }
                            if ui.button(format!("{}  {}", icon::X_SQUARE, t("sessionTabs.closeOthers"))).clicked() {
                                action = Some(TabAction::CloseOthers(id.clone()));
                                ui.close();
                            }
                        });
                    }
                    let (rect, r) = ui.allocate_exact_size(Vec2::new(32.0, 32.0), Sense::click());
                    if r.hovered() {
                        ui.painter().rect_filled(rect.shrink(4.0), egui::CornerRadius::same(5), tk.p.hover);
                    }
                    let c = if r.hovered() { tk.p.text } else { tk.p.muted };
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, icon::PLUS, egui::FontId::proportional(13.0), c);
                    if r.on_hover_text(t("sidebar.newThreadHere")).clicked() {
                        if let Err(e) = crate::commands::new_thread(app, &json!({ "projectId": project_id }), false) {
                            app.toast_err(&e);
                        }
                    }
                });
            });
    });
    widgets::hline(ui, &tk);
    match action {
        Some(TabAction::Open(id)) => app.select_thread(&id),
        Some(TabAction::Close(id)) => app.close_tab(&id),
        Some(TabAction::CloseOthers(id)) => {
            let _ = crate::commands::run_ui(app, "ui.thread.close-others", &json!({ "id": id }));
        }
        None => {}
    }
}

fn thread_header(app: &mut LazyApp, ui: &mut egui::Ui, th: &lazyaicode_doc::ThreadView) {
    let tk = app.tokens;
    let id = th.thread.id.clone();
    let busy = matches!(th.status, Status::Running | Status::Awaiting);
    egui::Frame::new().inner_margin(egui::Margin { left: 18, right: 12, top: 9, bottom: 9 }).show(ui, |ui| {
        ui.set_min_height(36.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                match app.eph.editing_title.as_mut() {
                    Some(v) => {
                        let r = ui.add(egui::TextEdit::singleline(v).font(egui::TextStyle::Heading).desired_width(420.0));
                        r.request_focus();
                        let done = r.lost_focus();
                        let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
                        if cancel {
                            app.eph.editing_title = None;
                        } else if done {
                            let title = v.trim().to_string();
                            app.eph.editing_title = None;
                            if !title.is_empty() && title != th.thread.title {
                                if let Some(t) = app.data.thread_mut(&id) {
                                    t.thread.title = title.clone();
                                }
                                app.fire("thread.update", json!({ "id": id, "title": title }));
                            }
                        }
                    }
                    None => {
                        let r = ui.add(
                            egui::Label::new(RichText::new(&th.thread.title).size(tk.text_size + 1.0).strong().color(tk.p.text))
                                .sense(Sense::click())
                                .truncate(),
                        );
                        if r.on_hover_text(t("thread.renameHint")).double_clicked() {
                            app.eph.editing_title = Some(th.thread.title.clone());
                        }
                    }
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if let Some(p) = app.data.project(&th.thread.project_id) {
                        ui.label(widgets::muted(&tk, &p.name).size(tk.text_size - 1.0));
                    }
                    let branch = app
                        .data
                        .branch
                        .get(&id)
                        .and_then(|b| b["branch"].as_str().map(str::to_string))
                        .or_else(|| th.thread.branch.clone());
                    if let Some(b) = branch.filter(|b| !b.is_empty()) {
                        ui.label(widgets::muted(&tk, format!("{} {b}", icon::GIT_BRANCH)).size(tk.text_size - 1.0));
                    }
                    if th.thread.provider != "claude" {
                        let label = lazyaicode_doc::PROVIDERS.iter().find(|p| p.0 == th.thread.provider).map_or("", |p| p.1);
                        widgets::chip(ui, &tk, label, tk.p.blue);
                    }
                    if th.thread.isolated {
                        widgets::chip(ui, &tk, &format!("{} {}", icon::STACK, t("thread.isolatedWorkspace")), tk.p.muted)
                            .on_hover_text(&th.thread.cwd);
                    } else {
                        widgets::chip(ui, &tk, &format!("{} {}", icon::FOLDER, widgets::short_path(&th.thread.cwd)), tk.p.muted)
                            .on_hover_text(lazyaicode_i18n::tf("thread.localCheckout", &[("path", &th.thread.cwd)]));
                    }
                    if th.thread.archived {
                        widgets::chip(ui, &tk, &t("thread.threadArchived"), tk.p.amber);
                    }
                });
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_button(ui, &tk, icon::ARCHIVE, &t("thread.archiveThread")).clicked() {
                    let _ = crate::commands::run_ui(app, "ui.thread.archive", &serde_json::Value::Null);
                }
                if th.thread.isolated {
                    let r = ui
                        .add_enabled(!busy, egui::Button::new(format!("{}  {}  {}", icon::GIT_MERGE, t("thread.land"), icon::CARET_DOWN)));
                    let r = r.on_hover_text(t("thread.landTitle"));
                    egui::Popup::menu(&r).show(|ui| {
                        ui.set_min_width(300.0);
                        let opt = |ui: &mut egui::Ui, title: &str, desc: &str| {
                            ui.add(
                                egui::Button::new(RichText::new(format!("{title}\n")).strong().append_text(desc, &tk)).frame(false).wrap(),
                            )
                            .clicked()
                        };
                        if opt(ui, &t("thread.applyTitle"), &t("thread.applyDesc")) {
                            land(app, &id, "apply");
                        }
                        let merge_desc = format!(
                            "{}{}{}",
                            t("thread.mergeDescPrefix"),
                            th.thread.branch.clone().unwrap_or_default(),
                            t("thread.mergeDescSuffix")
                        );
                        if opt(ui, &t("thread.mergeTitle"), &merge_desc) {
                            land(app, &id, "merge");
                        }
                        if opt(ui, &t("thread.prTitle"), &t("thread.prDesc")) {
                            app.send_message(&id, t("thread.prPrompt"));
                        }
                    });
                }
                let fav = th.thread.bookmarked;
                if widgets::icon_button_on(
                    ui,
                    &tk,
                    if fav { icon::STAR_FOUR } else { icon::STAR },
                    &lazyaicode_i18n::tf("thread.favorite", &[("key", &"Ctrl+D")]),
                    fav,
                )
                .clicked()
                {
                    let _ = crate::commands::run_ui(app, "ui.thread.toggle-bookmark", &serde_json::Value::Null);
                }
            });
        });
    });
}

trait AppendText {
    fn append_text(self, text: &str, tk: &crate::theme::Tokens) -> egui::WidgetText;
}

impl AppendText for RichText {
    fn append_text(self, text: &str, tk: &crate::theme::Tokens) -> egui::WidgetText {
        let mut job = egui::text::LayoutJob::default();
        let style = egui::Style::default();
        self.append_to(&mut job, &style, egui::FontSelection::Default, Align::Min);
        job.append(
            text,
            0.0,
            egui::TextFormat { font_id: egui::FontId::proportional(tk.text_size - 1.0), color: tk.p.muted, ..Default::default() },
        );
        job.into()
    }
}

fn land(app: &mut LazyApp, id: &str, mode: &str) {
    app.call_ok("thread.land", json!({ "id": id, "mode": mode }), |app, v| {
        app.toast(v.as_str().unwrap_or("ok").to_string(), ToastKind::Ok);
    });
}

fn repo_header(app: &mut LazyApp, ui: &mut egui::Ui, p: &lazyaicode_doc::Project) {
    let tk = app.tokens;
    egui::Frame::new().inner_margin(egui::Margin { left: 14, right: 10, top: 8, bottom: 4 }).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}  {}", icon::GIT_BRANCH, p.name)).size(tk.text_size + 3.0).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_button(ui, &tk, icon::X, &t("repoScreen.close")).clicked() {
                    app.state.repo_project = None;
                }
                if widgets::button(ui, &tk, &format!("{}  {}", icon::NOTE_PENCIL, t("sidebar.newThreadHere"))).clicked() {
                    let _ = crate::commands::new_thread(app, &json!({ "projectId": p.id }), false);
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add(egui::Label::new(widgets::muted(&tk, widgets::short_path(&p.path))).truncate());
                });
            });
        });
    });
}

fn tabs_bar(app: &mut LazyApp, ui: &mut egui::Ui, with_chat: bool) {
    let tk = app.tokens;
    let git = app.current_project().is_some_and(|p| p.is_git);
    egui::Frame::new().inner_margin(egui::Margin { left: 12, right: 8, top: 0, bottom: 0 }).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (i, tab) in MAIN_TABS.into_iter().enumerate() {
                if (!with_chat && tab == MainTab::Chat) || (!git && tab.is_git()) {
                    continue;
                }
                let on = app.state.tab == tab;
                let ic = match tab {
                    MainTab::Chat => icon::CHAT_CIRCLE,
                    MainTab::History => icon::GIT_COMMIT,
                    MainTab::Tree => icon::TREE_STRUCTURE,
                    MainTab::Reflog => icon::CLOCK_COUNTER_CLOCKWISE,
                    MainTab::Stats => icon::CHART_BAR,
                    MainTab::Terminal => icon::TERMINAL_WINDOW,
                };
                // ícone + nome, 10 px de cada lado e o sublinhado do accent na aba aberta
                let label = format!("{ic}  {}", t(tab.label_key()));
                let g = ui.painter().layout_no_wrap(label, egui::FontId::proportional(tk.text_size), tk.p.text);
                let (rect, r) = ui.allocate_exact_size(Vec2::new(g.size().x + 20.0, 34.0), Sense::click());
                let color = if on || r.hovered() { tk.p.text } else { tk.p.muted };
                ui.painter().galley(egui::pos2(rect.left() + 10.0, rect.center().y - g.size().y / 2.0), g, color);
                if on {
                    ui.painter().hline(rect.x_range(), rect.bottom() - 1.0, Stroke::new(2.0, tk.p.accent));
                }
                if r.on_hover_text(format!("Ctrl+{}", i + 1)).clicked() {
                    app.state.tab = tab;
                    if tab == MainTab::Chat {
                        app.eph.focus_composer = true;
                    }
                }
            }
            if app.state.tab == MainTab::History && with_chat {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let review = app.state.history_view == HistoryView::Review;
                    if ui.selectable_label(review, t("thread.agentReview")).clicked() {
                        let _ = crate::commands::run_ui(app, "ui.history.review", &serde_json::Value::Null);
                    }
                    if ui.selectable_label(!review, t("mainTabs.history")).clicked() {
                        app.state.history_view = HistoryView::Graph;
                    }
                });
            }
        });
    });
    widgets::hline(ui, &tk);
}

fn content(app: &mut LazyApp, ui: &mut egui::Ui, th: Option<&lazyaicode_doc::ThreadView>) {
    let path = app.current_path().unwrap_or_default();
    match (app.state.tab, th) {
        (MainTab::Chat, Some(th)) => crate::views::thread::show(app, ui, th),
        (MainTab::History, Some(th)) if app.state.history_view == HistoryView::Review => crate::views::review::show(app, ui, th),
        (MainTab::History, _) => crate::views::git::history(app, ui, &path),
        (MainTab::Tree, _) => crate::views::git::tree(app, ui, &path),
        (MainTab::Reflog, _) => crate::views::git::reflog(app, ui, &path),
        (MainTab::Stats, _) => crate::views::git::stats(app, ui, &path),
        (MainTab::Terminal, _) => {
            egui::Frame::new().inner_margin(egui::Margin::same(6)).show(ui, |ui| crate::terminal::show(app, ui, &path));
        }
        (MainTab::Chat, None) => {
            app.state.tab = MainTab::History;
        }
    }
}

fn welcome(app: &mut LazyApp, ui: &mut egui::Ui) {
    let tk = app.tokens;
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.25);
        ui.label(RichText::new("LazyAICode").size(28.0).strong().color(tk.p.text));
        ui.add_space(4.0);
        ui.label(widgets::muted(&tk, t("app.welcome")));
        ui.add_space(18.0);
        if widgets::primary(ui, &tk, &format!("{}  {}", icon::NOTE_PENCIL, t("sidebar.newThreadShort"))).clicked() {
            if let Err(e) = crate::commands::new_thread(app, &json!({}), false) {
                app.toast_err(&e);
            }
        }
        ui.add_space(6.0);
        if app.data.projects.iter().any(|p| p.is_git)
            && widgets::button(ui, &tk, &format!("{}  {}", icon::STACK, t("sidebar.newIsolated"))).clicked()
        {
            let _ = crate::commands::new_thread(app, &json!({}), true);
        }
        ui.add_space(24.0);
        for (k, label) in [
            ("Ctrl+K", "palette.title"),
            ("Ctrl+N", "sidebar.newThreadShort"),
            ("Ctrl+B", "shortcuts.toggleSidebar"),
            ("Ctrl+,", "settings.title"),
        ] {
            ui.horizontal(|ui| {
                ui.add_space(ui.available_width() / 2.0 - 110.0);
                widgets::kbd(ui, &tk, k);
                ui.label(widgets::muted(&tk, t(label)));
            });
        }
    });
}

//! A aba Conversa: a transcrição como uma conversa (cada mensagem do
//! usuário com o avatar dele; tudo o que o agente faz até a próxima
//! mensagem sob um avatar só), o campo de digitar como a próxima mensagem e
//! o rodapé da sessão.

use egui::{Align, Layout, RichText, Vec2};
use lazyaicode_doc::transcript::Item;
use lazyaicode_doc::{Status, ThreadView};
use lazyaicode_i18n::t;

use crate::app::LazyApp;
use crate::views::widgets;

pub fn show(app: &mut LazyApp, ui: &mut egui::Ui, th: &ThreadView) {
    let tk = app.tokens;
    let id = th.thread.id.clone();
    egui::Panel::bottom(egui::Id::new(("session-footer", &id)))
        .frame(egui::Frame::new().fill(tk.p.bg).inner_margin(egui::Margin::symmetric(12, 4)))
        .show(ui, |ui| {
            crate::views::footer::show(app, ui, th);
        });
    let user = app.data.profile.as_ref().and_then(|p| p["name"].as_str()).map(str::to_string).unwrap_or_else(|| t("thread.you"));
    let width = tk.content_w.min(ui.available_width() - 32.0).max(200.0);
    let stick = std::mem::take(&mut app.eph.scroll_to_bottom);
    let scroll = egui::ScrollArea::vertical().id_salt(("chat", &id)).auto_shrink([false, false]).stick_to_bottom(true);
    // a imagem anexada aberta em tamanho real
    if let Some(url) = app.eph.preview_image.clone() {
        let mut open = true;
        egui::Window::new(t("thread.attachedImage")).open(&mut open).collapsible(false).resizable(true).show(ui.ctx(), |ui| {
            if let Some(bytes) = crate::views::transcript::data_url_bytes(&url).filter(|b| crate::views::transcript::image_ok(b)) {
                ui.add(egui::Image::from_bytes(format!("bytes://preview-{}.img", url.len()), bytes).max_size(Vec2::new(900.0, 700.0)));
            }
        });
        if !open || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.eph.preview_image = None;
        }
    }
    let top = std::mem::take(&mut app.eph.scroll_top);
    scroll.show(ui, |ui| {
        if top {
            ui.scroll_to_cursor(Some(Align::TOP));
        }
        ui.add_space(12.0);
        let items: Vec<Item> = app.data.transcripts.get(&id).map(|t| t.items.clone()).unwrap_or_default();
        let margin = ((ui.available_width() - width) / 2.0).max(8.0);
        ui.horizontal(|ui| {
            ui.add_space(margin);
            ui.vertical(|ui| {
                ui.set_width(width);
                if items.is_empty() {
                    if app.data.loading_transcript.contains(&id) {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(widgets::muted(&tk, t("thread.loading")));
                        });
                    } else {
                        hello(app, ui, th);
                    }
                }
                let mut prev_user = true;
                for it in &items {
                    if let Item::User { key, text, images } = it {
                        row(ui, &tk, Some((&user, false)), |ui| crate::views::transcript::user_message(app, ui, key, text, images));
                        prev_user = true;
                        continue;
                    }
                    let head = prev_user;
                    prev_user = false;
                    row(ui, &tk, head.then_some(("agent", true)), |ui| crate::views::transcript::item(app, ui, &id, it));
                }
                if th.status == Status::Running {
                    row(ui, &tk, None, |ui| {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(widgets::muted(&tk, t("thread.working")));
                            widgets::kbd(ui, &tk, "Esc");
                            ui.label(widgets::faint(&tk, t("thread.toInterrupt")));
                        });
                    });
                }
                ui.add_space(8.0);
                row(ui, &tk, Some((&user, false)), |ui| crate::views::composer::show(app, ui, th));
                ui.add_space(24.0);
                if stick {
                    ui.scroll_to_cursor(Some(Align::BOTTOM));
                }
            });
        });
    });
}

/// Uma linha da conversa: avatar (ou a margem dele) e o conteúdo.
fn row(ui: &mut egui::Ui, tk: &crate::theme::Tokens, avatar: Option<(&str, bool)>, body: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal_top(|ui| {
        match avatar {
            Some((name, agent)) => {
                widgets::avatar(ui, tk, name, agent);
            }
            None => {
                ui.add_space(26.0);
            }
        }
        ui.add_space(6.0);
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.set_max_width(ui.available_width());
            ui.add_space(3.0);
            body(ui);
        });
    });
    ui.add_space(6.0);
}

fn hello(app: &LazyApp, ui: &mut egui::Ui, th: &ThreadView) {
    let tk = app.tokens;
    let text = if th.thread.isolated {
        format!(
            "{}{}{}{}{}",
            t("thread.helloIsolatedA"),
            th.thread.branch.clone().unwrap_or_default(),
            t("thread.helloIsolatedB"),
            th.thread.base_ref.clone().unwrap_or_default(),
            t("thread.helloIsolatedC")
        )
    } else {
        format!("{}{}{}", t("thread.helloDirectPrefix"), th.thread.cwd, t("thread.helloDirectSuffix"))
    };
    ui.add_space(20.0);
    ui.label(RichText::new(text).color(tk.p.muted));
    ui.add_space(20.0);
}

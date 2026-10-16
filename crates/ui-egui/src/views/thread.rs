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
        // A coluna tem largura fixa e cada linha é desenhada num filho
        // recortado nela: um bloco de código ou uma linha sem quebra larga
        // demais não alarga a coluna (no egui, o pai cresce para caber o
        // filho, e todas as linhas seguintes passariam da borda).
        let avail = ui.available_width();
        let width = tk.content_w.min(avail - 48.0).max(200.0);
        let margin = ((avail - width) / 2.0).max(24.0).min((avail - width).max(0.0));
        let col = Col { left: ui.cursor().left() + margin, width };
        if items.is_empty() {
            col.row(ui, &tk, Avatar::Gap, |ui| {
                if app.data.loading_transcript.contains(&id) {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(widgets::muted(&tk, t("thread.loading")));
                    });
                } else {
                    hello(app, ui, th);
                }
            });
        }
        let mut prev_user = true;
        for (n, it) in items.iter().enumerate() {
            if let Item::User { key, text, images } = it {
                // espaço antes de cada mensagem do usuário, como no app antigo
                if n > 0 {
                    ui.add_space(12.0);
                }
                col.row(ui, &tk, Avatar::Person(&user), |ui| crate::views::transcript::user_message(app, ui, key, text, images));
                prev_user = true;
                continue;
            }
            let head = prev_user;
            prev_user = false;
            if head {
                ui.add_space(4.0);
            }
            let avatar = if head { Avatar::Agent } else { Avatar::Gap };
            col.row(ui, &tk, avatar, |ui| crate::views::transcript::item(app, ui, &id, it));
        }
        if th.status == Status::Running {
            col.row(ui, &tk, Avatar::Gap, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(widgets::muted(&tk, t("thread.working")));
                    widgets::kbd(ui, &tk, "Esc");
                    ui.label(widgets::faint(&tk, t("thread.toInterrupt")));
                });
            });
        }
        ui.add_space(8.0);
        col.row(ui, &tk, Avatar::Person(&user), |ui| crate::views::composer::show(app, ui, th));
        ui.add_space(32.0);
        if stick {
            ui.scroll_to_cursor(Some(Align::BOTTOM));
        }
    });
}

enum Avatar<'a> {
    Person(&'a str),
    Agent,
    Gap,
}

/// A coluna da conversa: onde começa e quanto mede.
#[derive(Clone, Copy)]
struct Col {
    left: f32,
    width: f32,
}

impl Col {
    /// Uma linha da conversa: avatar (ou a margem dele) e o conteúdo, num
    /// filho de largura fixa, recortado na coluna.
    fn row(self, ui: &mut egui::Ui, tk: &crate::theme::Tokens, avatar: Avatar, body: impl FnOnce(&mut egui::Ui)) {
        let top = ui.cursor().top();
        let rect = egui::Rect::from_min_size(egui::pos2(self.left, top), Vec2::new(self.width, f32::INFINITY));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Min)));
        let clip = ui.clip_rect();
        child.set_clip_rect(egui::Rect::from_x_y_ranges(self.left - 2.0..=self.left + self.width + 2.0, clip.y_range()));
        // avatar (ou o vão dele) e 12 px até o conteúdo, sem o espaçamento do egui
        child.spacing_mut().item_spacing.x = 0.0;
        match avatar {
            Avatar::Person(name) => {
                widgets::avatar(&mut child, tk, name, false);
            }
            Avatar::Agent => {
                widgets::avatar(&mut child, tk, "agent", true);
            }
            Avatar::Gap => {
                child.allocate_exact_size(Vec2::splat(26.0), egui::Sense::hover());
            }
        }
        child.add_space(12.0);
        child.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.set_max_width(ui.available_width());
            ui.add_space(3.0);
            body(ui);
        });
        let height = child.min_rect().height();
        ui.advance_cursor_after_rect(egui::Rect::from_min_size(egui::pos2(self.left, top), Vec2::new(self.width, height)));
        ui.add_space(6.0);
    }
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

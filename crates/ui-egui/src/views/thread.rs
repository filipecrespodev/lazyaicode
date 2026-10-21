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
        let rows: Vec<bool> =
            app.data.transcripts.get(&id).map(|t| t.items.iter().map(|it| matches!(it, Item::User { .. })).collect()).unwrap_or_default();
        // A coluna tem largura fixa e cada linha é desenhada num filho
        // recortado nela: um bloco de código ou uma linha sem quebra larga
        // demais não alarga a coluna (no egui, o pai cresce para caber o
        // filho, e todas as linhas seguintes passariam da borda).
        let avail = ui.available_width();
        let width = tk.content_w.min(avail - 48.0).max(200.0);
        let margin = ((avail - width) / 2.0).max(24.0).min((avail - width).max(0.0));
        let col = Col { left: ui.cursor().left() + margin, width };
        if rows.is_empty() {
            col.row(ui, &tk, "hello", Avatar::Gap, |ui| {
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
        // Só as linhas perto da área visível são desenhadas; as outras
        // reservam a altura medida da última vez. Desenhar a conversa toda
        // a cada quadro (o Markdown de cada resposta refeito) deixava o
        // quadro proporcional ao tamanho da conversa, e durante a resposta
        // do agente há um quadro por pedaço de texto.
        let clip = ui.clip_rect().expand2(Vec2::new(0.0, NEAR));
        let mut prev_user = true;
        for (n, is_user) in rows.iter().copied().enumerate() {
            let head = !is_user && prev_user;
            prev_user = is_user;
            let top = ui.cursor().top();
            let Some(it) = app.data.transcripts.get(&id).and_then(|t| t.items.get(n)) else { break };
            let s = sig(app, it, width, head);
            let known = app.eph.row_heights.get(&id).and_then(|m| m.get(it.key())).filter(|(old, _)| *old == s).map(|(_, h)| *h);
            if let Some(h) = known.filter(|h| top > clip.bottom() || top + h < clip.top()) {
                ui.add_space(h);
                continue;
            }
            let it = it.clone();
            if let Item::User { key, text, images } = &it {
                // espaço antes de cada mensagem do usuário, como no app antigo
                if n > 0 {
                    ui.add_space(12.0);
                }
                col.row(ui, &tk, key, Avatar::Person(&user), |ui| crate::views::transcript::user_message(app, ui, key, text, images));
            } else {
                if head {
                    ui.add_space(4.0);
                }
                let avatar = if head { Avatar::Agent } else { Avatar::Gap };
                col.row(ui, &tk, it.key(), avatar, |ui| crate::views::transcript::item(app, ui, &id, &it));
            }
            let h = ui.cursor().top() - top;
            app.eph.row_heights.entry(id.clone()).or_default().insert(it.key().to_string(), (s, h));
        }
        if th.status == Status::Running {
            col.row(ui, &tk, "working", Avatar::Gap, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(widgets::muted(&tk, t("thread.working")));
                    widgets::kbd(ui, &tk, "Esc");
                    ui.label(widgets::faint(&tk, t("thread.toInterrupt")));
                });
            });
        }
        ui.add_space(8.0);
        col.row(ui, &tk, "composer", Avatar::Person(&user), |ui| crate::views::composer::show(app, ui, th));
        ui.add_space(32.0);
        if stick {
            ui.scroll_to_cursor(Some(Align::BOTTOM));
        }
    });
}

/// Quanto além da área visível (px, acima e abaixo) ainda se desenha.
const NEAR: f32 = 400.0;

/// Muda quando a altura da linha pode mudar: o conteúdo cresceu, o card foi
/// aberto/fechado, a coluna mudou de largura ou a linha ganhou/perdeu o
/// avatar.
fn sig(app: &LazyApp, it: &Item, width: f32, head: bool) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (width.to_bits(), head).hash(&mut h);
    let open = |m: &std::collections::HashMap<String, bool>, k: &str| m.get(k).copied().unwrap_or(false);
    match it {
        Item::User { text, images, .. } => (0u8, text.len(), images.len()).hash(&mut h),
        Item::Text { text, is_final, .. } => (1u8, text.len(), is_final).hash(&mut h),
        Item::Thinking { key, text, is_final, .. } => (2u8, text.len(), is_final, open(&app.eph.thinking_open, key)).hash(&mut h),
        Item::Tool { key, input, partial, result, .. } => {
            let r = result.as_ref().map(|r| (r.text.len(), r.is_error));
            (3u8, input.as_object().map_or(0, |o| o.len()), partial.is_empty(), r, open(&app.eph.tool_open, key)).hash(&mut h)
        }
        Item::Permission { key, decided, .. } => (4u8, decided.is_some(), open(&app.eph.tool_open, &format!("{key}-detail"))).hash(&mut h),
        Item::Result { is_error, text, .. } => (5u8, is_error, text.as_ref().map(String::len)).hash(&mut h),
        Item::Error { text, .. } => (6u8, text.len()).hash(&mut h),
        Item::Notice { text, .. } => (7u8, text.len()).hash(&mut h),
    }
    h.finish()
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
    /// filho de largura fixa, recortado na coluna. `salt` dá a cada linha um
    /// id próprio: sem ele, todas teriam o mesmo, e o que guarda estado pelo
    /// id (as larguras das colunas de uma tabela em Markdown) se misturaria.
    fn row(self, ui: &mut egui::Ui, tk: &crate::theme::Tokens, salt: &str, avatar: Avatar, body: impl FnOnce(&mut egui::Ui)) {
        let top = ui.cursor().top();
        let rect = egui::Rect::from_min_size(egui::pos2(self.left, top), Vec2::new(self.width, f32::INFINITY));
        let mut child = ui.new_child(egui::UiBuilder::new().id_salt(salt).max_rect(rect).layout(Layout::left_to_right(Align::Min)));
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

#[cfg(test)]
mod tests {
    use super::*;
    use lazyaicode_doc::transcript::Transcript;
    use serde_json::json;

    /// O último de alguns quadros da conversa com uma resposta por texto.
    fn last_frame(replies: &[String]) -> Vec<egui::epaint::ClippedShape> {
        let mut app = LazyApp::new(Box::new(crate::MockClient::default()), Box::new(crate::NoHost), crate::Inbox::default());
        let th: ThreadView = serde_json::from_value(json!({
            "id": "t1", "project_id": "p1", "workspace_id": "w1", "title": "t", "created_at": 0, "updated_at": 0,
            "cwd": "/demo", "isolated": false, "branch": null, "base_ref": null, "session_id": null,
            "model": "default", "permission_mode": "default"
        }))
        .unwrap();
        let mut tr = Transcript::new();
        for (i, text) in replies.iter().enumerate() {
            tr.apply(&json!({"type": "user", "message": {"role": "user", "content": format!("pergunta {i}")}}));
            tr.apply(&json!({"type": "assistant", "message": {"id": format!("m{i}"), "content": [{"type": "text", "text": text}]}}));
        }
        app.data.transcripts.insert("t1".into(), tr);
        let ctx = egui::Context::default();
        let mut shapes = Vec::new();
        for f in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                time: Some(f64::from(f) * 0.05),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                egui::CentralPanel::no_frame().show(ui, |ui| show(&mut app, ui, &th));
            });
            out.textures_delta.clear();
            shapes = out.shapes;
        }
        shapes
    }

    fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
        fn walk(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        shapes.iter().for_each(|c| walk(&c.shape, &mut out));
        out
    }

    #[test]
    fn the_frame_does_not_grow_with_the_conversation() {
        // a resposta do agente chega em pedaços, um quadro por pedaço: se o
        // quadro desenhasse a conversa inteira, travaria nas conversas longas
        let reply = "## Resposta\n\nUm parágrafo com **negrito** e `código`.\n\n- um\n- dois\n\n```rust\nfn x() {}\n```".to_string();
        let (short, long) = (last_frame(&vec![reply.clone(); 40]).len(), last_frame(&vec![reply; 400]).len());
        assert!(long < short * 2, "40 turnos: {short} formas; 400 turnos: {long}");
    }

    #[test]
    fn tables_in_different_replies_do_not_share_an_id() {
        // a tabela do Markdown guarda as larguras das colunas pelo id: duas
        // respostas com o mesmo id sobrepunham as colunas uma da outra
        let a = "| coluna bem larga mesmo | b |\n|---|---|\n| 1 | 2 |".to_string();
        let b = "| x | y |\n|---|---|\n| 3 | 4 |".to_string();
        let clash: Vec<String> = texts(&last_frame(&[a, b])).into_iter().filter(|t| t.contains("use of")).collect();
        assert!(clash.is_empty(), "{clash:?}");
    }
}

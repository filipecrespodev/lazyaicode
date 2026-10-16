//! Os itens da conversa: mensagens, raciocínio, ferramentas, permissões,
//! resultados de turno.

use base64::Engine as _;
use egui::{Color32, CornerRadius, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular as icon;
use lazyaicode_doc::transcript::{tool_summary, Decision, Item};
use lazyaicode_i18n::{t, tf};
use serde_json::Value;

use crate::app::LazyApp;
use crate::views::widgets;

/// Texto de ferramenta mostrado no card (o resto fica cortado).
const PREVIEW: usize = 6000;

fn cut(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_string();
    }
    let mut i = n;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    format!("{}\n…", &s[..i])
}

/// Bytes de uma data URL (`data:image/png;base64,…`).
pub fn data_url_bytes(url: &str) -> Option<Vec<u8>> {
    let (_, b64) = url.split_once(";base64,")?;
    base64::engine::general_purpose::STANDARD.decode(b64).ok()
}

/// Maior imagem que a UI decodifica (lado e total de pixels). As imagens
/// vêm do histórico e de anexos: um PNG pequeno pode declarar 50 000²
/// pixels e pedir gigabytes ao ser aberto.
pub const MAX_IMAGE_SIDE: u32 = 8192;
pub const MAX_IMAGE_PIXELS: u64 = 40_000_000;

/// As dimensões cabem nos limites? Lê só o cabeçalho.
pub fn image_ok(bytes: &[u8]) -> bool {
    let Ok(reader) = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format() else { return false };
    match reader.into_dimensions() {
        Ok((w, h)) => w <= MAX_IMAGE_SIDE && h <= MAX_IMAGE_SIDE && u64::from(w) * u64::from(h) <= MAX_IMAGE_PIXELS,
        Err(_) => false,
    }
}

pub fn image_thumb(ui: &mut egui::Ui, uri: String, bytes: Vec<u8>, max: f32) -> egui::Response {
    if !image_ok(&bytes) {
        return ui.add(egui::Label::new(RichText::new(format!("{} {}", icon::IMAGE_BROKEN, t("images.notShown")))).sense(Sense::click()));
    }
    ui.add(egui::Image::from_bytes(uri, bytes).max_size(Vec2::splat(max)).corner_radius(CornerRadius::same(6)).sense(Sense::click()))
}

pub fn user_message(app: &mut LazyApp, ui: &mut egui::Ui, key: &str, text: &str, images: &[String]) {
    let tk = app.tokens;
    if !images.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for (n, url) in images.iter().enumerate() {
                if let Some(bytes) = data_url_bytes(url) {
                    if image_thumb(ui, format!("bytes://{key}-{n}.img"), bytes, 120.0).on_hover_text(t("thread.viewImage")).clicked() {
                        app.eph.preview_image = Some(url.clone());
                    }
                }
            }
        });
    }
    if !text.is_empty() {
        ui.add(egui::Label::new(RichText::new(text).size(tk.text_size + 1.0).color(tk.p.text)).wrap().selectable(true));
    }
}

pub fn item(app: &mut LazyApp, ui: &mut egui::Ui, thread: &str, it: &Item) {
    let tk = app.tokens;
    match it {
        Item::Text { text, .. } => {
            egui_commonmark::CommonMarkViewer::new().show(ui, &mut app.md, text);
        }
        Item::Thinking { key, text, is_final, .. } => {
            let open = app.eph.thinking_open.get(key).copied().unwrap_or(false);
            let label = if *is_final { t("thread.reasoning") } else { t("thread.thinking") };
            let r = ui.add(egui::Button::new(RichText::new(format!("{}  {label}", icon::BRAIN)).color(tk.p.muted).italics()).frame(false));
            if r.clicked() {
                app.eph.thinking_open.insert(key.clone(), !open);
            }
            if open && !text.is_empty() {
                egui::Frame::new().inner_margin(egui::Margin { left: 10, right: 4, top: 2, bottom: 4 }).stroke(Stroke::NONE).show(
                    ui,
                    |ui| {
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(2.0, 0.0), Sense::hover());
                        let _ = rect;
                        ui.add(egui::Label::new(RichText::new(text).color(tk.p.muted).italics()).wrap().selectable(true));
                    },
                );
            }
        }
        Item::Tool { key, name, input, partial, result, .. } => tool_card(app, ui, key, name, input, partial, result.as_ref()),
        Item::Permission { key, request_id, tool_name, input, reason, suggestions, decided, .. } => {
            permission_card(app, ui, thread, key, request_id, tool_name, input, reason.as_deref(), !suggestions.is_empty(), *decided)
        }
        Item::Result { duration_ms, cost_usd, is_error, text, .. } => {
            let mut s = String::new();
            if *is_error {
                s.push_str(&format!(
                    "{}  {} · ",
                    icon::WARNING,
                    text.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| t("thread.turnFailed"))
                ));
            }
            s.push_str(&widgets::fmt_duration(*duration_ms));
            if *cost_usd > 0.0 {
                s.push_str(&format!(" · US$ {cost_usd:.4}"));
            }
            ui.label(RichText::new(s).size(tk.text_size - 2.0).color(if *is_error { tk.p.red } else { tk.p.faint }));
        }
        Item::Error { text, .. } => {
            egui::Frame::new().fill(tk.del_bg).corner_radius(CornerRadius::same(tk.radius_sm)).inner_margin(egui::Margin::same(8)).show(
                ui,
                |ui| {
                    ui.horizontal_top(|ui| {
                        ui.label(RichText::new(icon::WARNING).color(tk.p.red));
                        ui.add(egui::Label::new(RichText::new(text).monospace().color(tk.del_fg)).wrap().selectable(true));
                    });
                },
            );
        }
        Item::Notice { text, .. } => {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(icon::INFO).color(tk.p.amber));
                ui.add(egui::Label::new(RichText::new(text).color(tk.p.amber)).wrap().selectable(true));
            });
        }
        Item::User { .. } => {}
    }
}

fn tool_card(
    app: &mut LazyApp,
    ui: &mut egui::Ui,
    key: &str,
    name: &str,
    input: &Value,
    partial: &str,
    result: Option<&lazyaicode_doc::transcript::ToolResult>,
) {
    let tk = app.tokens;
    let open = app.eph.tool_open.get(key).copied().unwrap_or(false);
    let err = result.is_some_and(|r| r.is_error);
    let border = if err { tk.p.red.gamma_multiply(0.5) } else { tk.p.border };
    let summary = tool_summary(name, input);
    egui::Frame::new().fill(tk.p.panel).stroke(Stroke::new(1.0, border)).corner_radius(CornerRadius::same(tk.radius)).show(ui, |ui| {
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, tk.row_h + 2.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::same(tk.radius), tk.p.hover);
        }
        let p = ui.painter_at(rect);
        let mut x = rect.left() + 10.0;
        let cy = rect.center().y;
        p.text(
            egui::pos2(x, cy),
            egui::Align2::LEFT_CENTER,
            if open { icon::CARET_DOWN } else { icon::CARET_RIGHT },
            egui::FontId::proportional(11.0),
            tk.p.muted,
        );
        x += 16.0;
        let ng = p.layout_no_wrap(name.to_string(), egui::FontId::proportional(tk.text_size), tk.p.text);
        let nw = ng.size().x;
        p.galley(egui::pos2(x, cy - ng.size().y / 2.0), ng, tk.p.text);
        x += nw + 8.0;
        let state_w = 60.0;
        let shown = if summary.is_empty() && !partial.is_empty() { "…".to_string() } else { summary.clone() };
        let mut job = egui::text::LayoutJob::single_section(
            shown,
            egui::TextFormat { font_id: egui::FontId::monospace(tk.code_size - 1.0), color: tk.p.muted, ..Default::default() },
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width((rect.right() - state_w - x).max(10.0));
        let sg = p.layout_job(job);
        p.galley(egui::pos2(x, cy - sg.size().y / 2.0), sg, tk.p.muted);
        let right = egui::pos2(rect.right() - 12.0, cy);
        match result {
            None => {
                let a = (app.now * 5.0) as f32;
                let c = right - Vec2::new(4.0, 0.0);
                let pts: Vec<egui::Pos2> = (0..10).map(|i| c + Vec2::angled(a + i as f32 * 0.17) * 5.0).collect();
                p.add(egui::Shape::line(pts, Stroke::new(1.5, tk.p.blue)));
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(60));
            }
            Some(r) if r.is_error => {
                p.text(right, egui::Align2::RIGHT_CENTER, t("tool.error"), egui::FontId::proportional(tk.text_size - 2.0), tk.p.red);
            }
            Some(_) => {
                p.text(right, egui::Align2::RIGHT_CENTER, icon::CHECK, egui::FontId::proportional(tk.text_size), tk.p.accent);
            }
        }
        if resp.on_hover_text(&summary).clicked() {
            app.eph.tool_open.insert(key.to_string(), !open);
        }
        if open {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                let is_edit = matches!(name, "Edit" | "MultiEdit") && input["old_string"].is_string();
                if is_edit {
                    for l in input["old_string"].as_str().unwrap_or("").lines() {
                        ui.add(
                            egui::Label::new(RichText::new(format!("- {l}")).monospace().color(tk.del_fg).background_color(tk.del_bg))
                                .wrap(),
                        );
                    }
                    for l in input["new_string"].as_str().unwrap_or("").lines() {
                        ui.add(
                            egui::Label::new(RichText::new(format!("+ {l}")).monospace().color(tk.add_fg).background_color(tk.add_bg))
                                .wrap(),
                        );
                    }
                } else if name == "Bash" {
                    ui.add(
                        egui::Label::new(RichText::new(format!("$ {}", input["command"].as_str().unwrap_or(""))).monospace())
                            .wrap()
                            .selectable(true),
                    );
                } else {
                    let pretty = serde_json::to_string_pretty(input).unwrap_or_default();
                    ui.add(egui::Label::new(RichText::new(cut(&pretty, PREVIEW)).monospace().color(tk.p.muted)).wrap().selectable(true));
                }
                if let Some(r) = result {
                    ui.add_space(4.0);
                    ui.label(RichText::new(t("tool.result")).size(tk.text_size - 2.0).color(tk.p.faint));
                    let text = if r.text.is_empty() { t("tool.empty") } else { cut(&r.text, PREVIEW) };
                    egui::ScrollArea::vertical().id_salt(("tool-out", key)).max_height(320.0).show(ui, |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(text).monospace().color(if r.is_error { tk.del_fg } else { tk.p.text }))
                                .wrap()
                                .selectable(true),
                        );
                    });
                }
            });
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn permission_card(
    app: &mut LazyApp,
    ui: &mut egui::Ui,
    thread: &str,
    key: &str,
    request_id: &str,
    tool: &str,
    input: &Value,
    reason: Option<&str>,
    can_always: bool,
    decided: Option<Decision>,
) {
    let tk = app.tokens;
    let pending = decided.is_none();
    let stroke = if pending { Stroke::new(1.5, tk.p.amber) } else { Stroke::new(1.0, tk.p.border) };
    egui::Frame::new()
        .fill(tk.p.panel)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(tk.radius))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::HAND_PALM).color(tk.p.amber));
                ui.label(RichText::new(tf("permission.wantsToUse", &[("tool", &tool)])).strong());
            });
            let summary = tool_summary(tool, input);
            if !summary.is_empty() {
                ui.add(egui::Label::new(RichText::new(summary).monospace().color(tk.p.muted)).wrap().selectable(true));
            }
            if let Some(r) = reason.filter(|r| !r.is_empty()) {
                ui.label(RichText::new(r).size(tk.text_size - 1.0).color(tk.p.faint));
            }
            let detail_key = format!("{key}-detail");
            let open = app.eph.tool_open.get(&detail_key).copied().unwrap_or(false);
            if ui
                .add(
                    egui::Button::new(widgets::faint(&tk, if open { t("permission.hideDetails") } else { t("permission.showDetails") }))
                        .frame(false),
                )
                .clicked()
            {
                app.eph.tool_open.insert(detail_key, !open);
            }
            if open {
                ui.add(
                    egui::Label::new(
                        RichText::new(cut(&serde_json::to_string_pretty(input).unwrap_or_default(), PREVIEW)).monospace().color(tk.p.muted),
                    )
                    .wrap()
                    .selectable(true),
                );
            }
            ui.add_space(4.0);
            match decided {
                None => {
                    ui.horizontal(|ui| {
                        if widgets::primary(ui, &tk, &t("permission.allow")).on_hover_text("Ctrl+Y").clicked() {
                            app.respond_permission(thread, request_id, Decision::Allow);
                        }
                        if can_always && widgets::button(ui, &tk, &t("permission.alwaysAllow")).on_hover_text("Ctrl+Shift+Y").clicked() {
                            app.respond_permission(thread, request_id, Decision::Always);
                        }
                        if widgets::button(ui, &tk, &t("permission.deny")).on_hover_text("Ctrl+Shift+X").clicked() {
                            app.respond_permission(thread, request_id, Decision::Deny);
                        }
                    });
                }
                Some(d) => {
                    let (text, color) = match d {
                        Decision::Allow => (t("permission.allowed"), tk.p.accent),
                        Decision::Always => (t("permission.alwaysAllowed"), tk.p.accent),
                        Decision::Deny => (t("permission.denied"), tk.p.red),
                        Decision::Cancelled => (t("permission.cancelled"), tk.p.faint),
                    };
                    ui.label(RichText::new(text).color(color).size(tk.text_size - 1.0));
                }
            }
        });
    let _ = Color32::TRANSPARENT;
}

/// A conversa em Markdown (copiar, exportar).
pub fn to_markdown(items: &[Item]) -> String {
    let mut out = String::new();
    for i in items {
        match i {
            Item::User { text, .. } => out.push_str(&format!("**Você:** {text}\n\n")),
            Item::Text { text, .. } => out.push_str(&format!("{text}\n\n")),
            Item::Tool { name, input, .. } => out.push_str(&format!("> `{name}` {}\n\n", tool_summary(name, input))),
            Item::Error { text, .. } => out.push_str(&format!("> ⚠ {text}\n\n")),
            Item::Notice { text, .. } => out.push_str(&format!("> {text}\n\n")),
            _ => {}
        }
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_images_whose_header_declares_a_huge_canvas() {
        // PNG de 1x1 válido
        let mut ok = Vec::new();
        image::RgbaImage::new(1, 1).write_to(&mut std::io::Cursor::new(&mut ok), image::ImageFormat::Png).unwrap();
        assert!(image_ok(&ok));
        // o mesmo arquivo com o IHDR dizendo 60000 x 60000
        let mut bomb = ok.clone();
        bomb[16..20].copy_from_slice(&60_000u32.to_be_bytes());
        bomb[20..24].copy_from_slice(&60_000u32.to_be_bytes());
        assert!(!image_ok(&bomb));
        assert!(!image_ok(b"nada"));
    }
}

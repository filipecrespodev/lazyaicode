//! Peças pequenas usadas por várias telas.

use egui::{Color32, CornerRadius, FontId, Response, RichText, Sense, Stroke, Ui, Vec2};
use lazyaicode_doc::Status;

use crate::theme::Tokens;

/// Botão só com ícone, sem fundo até passar o mouse.
pub fn icon_button(ui: &mut Ui, tk: &Tokens, icon: &str, tip: &str) -> Response {
    icon_button_on(ui, tk, icon, tip, false)
}

pub fn icon_button_on(ui: &mut Ui, tk: &Tokens, icon: &str, tip: &str, on: bool) -> Response {
    let color = if on { tk.p.accent } else { tk.p.muted };
    let b = egui::Button::new(RichText::new(icon).size(tk.text_size + 3.0).color(color)).frame(false).min_size(Vec2::splat(tk.row_h - 4.0));
    let r = ui.add(b);
    let r = if tip.is_empty() { r } else { r.on_hover_text(tip) };
    if r.hovered() {
        ui.painter().rect_filled(r.rect, CornerRadius::same(tk.radius_sm), tk.p.hover.gamma_multiply(0.6));
        ui.painter().text(r.rect.center(), egui::Align2::CENTER_CENTER, icon, FontId::proportional(tk.text_size + 3.0), tk.p.text);
    }
    r
}

/// Botão com o fundo do accent.
pub fn primary(ui: &mut Ui, tk: &Tokens, text: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(text).color(tk.on_accent).strong()).fill(tk.p.accent).stroke(Stroke::NONE))
}

/// Botão com borda.
pub fn button(ui: &mut Ui, tk: &Tokens, text: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(text).color(tk.p.text)).fill(tk.p.elev).stroke(Stroke::new(1.0, tk.p.border)))
}

pub fn danger(ui: &mut Ui, tk: &Tokens, text: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(text).color(Color32::WHITE).strong()).fill(tk.p.red).stroke(Stroke::NONE))
}

/// Etiqueta pequena com fundo suave.
pub fn chip(ui: &mut Ui, tk: &Tokens, text: &str, color: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(tk.text_size - 2.0), color);
    let size = galley.size() + Vec2::new(10.0, 4.0);
    let (rect, r) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(4), tk.p.elev, Stroke::new(1.0, tk.p.border), egui::StrokeKind::Inside);
    ui.painter().galley(rect.min + Vec2::new(5.0, 2.0), galley, color);
    r
}

/// Tecla de atalho (`Esc`, `Ctrl+K`).
pub fn kbd(ui: &mut Ui, tk: &Tokens, text: &str) {
    let text = pretty_shortcut(text);
    let galley = ui.painter().layout_no_wrap(text, FontId::monospace(tk.code_size - 2.0), tk.p.muted);
    let size = galley.size() + Vec2::new(8.0, 3.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(3), tk.p.panel, Stroke::new(1.0, tk.p.border), egui::StrokeKind::Inside);
    ui.painter().galley(rect.min + Vec2::new(4.0, 1.5), galley, tk.p.muted);
}

/// `Ctrl` vira `⌘` no macOS.
pub fn pretty_shortcut(s: &str) -> String {
    if cfg!(target_os = "macos") {
        s.replace("Ctrl+", "⌘").replace("Alt+", "⌥").replace("Shift+", "⇧")
    } else {
        s.replace("ArrowUp", "↑").replace("ArrowDown", "↓").replace("ArrowLeft", "←").replace("ArrowRight", "→")
    }
}

pub fn status_color(tk: &Tokens, s: Status) -> Color32 {
    match s {
        Status::Running => tk.p.blue,
        Status::Awaiting => tk.p.amber,
        Status::Error => tk.p.red,
        Status::Idle => tk.p.faint,
    }
}

/// Ponto de status (com o anel girando enquanto roda).
pub fn status_dot(ui: &mut Ui, tk: &Tokens, s: Status, unread: bool, time: f64) -> Response {
    let (rect, r) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
    let c = rect.center();
    match s {
        Status::Running => {
            let a = (time * 4.0) as f32;
            let p = ui.painter();
            p.circle_stroke(c, 4.5, Stroke::new(1.5, tk.p.blue.gamma_multiply(0.3)));
            let pts: Vec<egui::Pos2> = (0..12)
                .map(|i| {
                    let t = a + i as f32 * 0.15;
                    c + Vec2::new(t.cos(), t.sin()) * 4.5
                })
                .collect();
            p.add(egui::Shape::line(pts, Stroke::new(1.5, tk.p.blue)));
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
        }
        Status::Idle if unread => {
            ui.painter().circle_filled(c, 3.5, tk.p.accent);
        }
        Status::Idle => {}
        other => {
            ui.painter().circle_filled(c, 3.5, status_color(tk, other));
        }
    }
    r
}

/// Avatar redondo com as iniciais.
pub fn avatar(ui: &mut Ui, tk: &Tokens, name: &str, agent: bool) -> Response {
    let (rect, r) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
    let p = ui.painter();
    if agent {
        p.circle_filled(rect.center(), 13.0, tk.accent_soft);
        p.text(rect.center(), egui::Align2::CENTER_CENTER, egui_phosphor::regular::SPARKLE, FontId::proportional(15.0), tk.p.accent);
    } else {
        p.circle_filled(rect.center(), 13.0, tk.p.active);
        let initials: String = name.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect::<String>().to_uppercase();
        let initials = if initials.is_empty() { "?".to_string() } else { initials };
        p.text(rect.center(), egui::Align2::CENTER_CENTER, initials, FontId::proportional(11.0), tk.p.text);
    }
    r
}

/// Título pequeno de seção (`PROJETOS`).
pub fn section(ui: &mut Ui, tk: &Tokens, text: &str) {
    ui.label(RichText::new(text).size(tk.text_size - 1.0).color(tk.p.muted).strong());
}

pub fn muted(tk: &Tokens, text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(tk.p.muted)
}

pub fn faint(tk: &Tokens, text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(tk.p.faint)
}

/// Caminho com a pasta pessoal abreviada como `~`.
pub fn short_path(p: &str) -> String {
    for prefix in ["/home/", "/Users/"] {
        if let Some(rest) = p.strip_prefix(prefix) {
            return match rest.find('/') {
                Some(i) => format!("~{}", &rest[i..]),
                None => "~".into(),
            };
        }
    }
    if p.len() > 9 && p.get(1..9).is_some_and(|s| s.eq_ignore_ascii_case(":\\Users\\")) {
        if let Some(rest) = p.get(9..) {
            return match rest.find('\\') {
                Some(i) => format!("~{}", &rest[i..]),
                None => "~".into(),
            };
        }
    }
    p.to_string()
}

/// "agora", "5 min", "3 h", "2 d".
pub fn rel_time(ms: i64, now: i64) -> String {
    let s = (now - ms) / 1000;
    if s < 60 {
        lazyaicode_i18n::t("time.now")
    } else if s < 3600 {
        lazyaicode_i18n::tf("time.minutes", &[("n", &(s / 60))])
    } else if s < 86400 {
        lazyaicode_i18n::tf("time.hours", &[("n", &(s / 3600))])
    } else {
        lazyaicode_i18n::tf("time.days", &[("n", &(s / 86400))])
    }
}

pub fn fmt_duration(ms: u64) -> String {
    if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{}m{}s", ms / 60_000, ((ms % 60_000) as f64 / 1000.0).round())
    }
}

/// Linha separadora fina.
pub fn hline(ui: &mut Ui, tk: &Tokens) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, tk.p.border));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_home_folders() {
        assert_eq!(short_path("/home/ana/code/x"), "~/code/x");
        assert_eq!(short_path("/Users/ana"), "~");
        assert_eq!(short_path("C:\\Users\\ana\\repo"), "~\\repo");
        assert_eq!(short_path("/srv/x"), "/srv/x");
        assert_eq!(fmt_duration(1500), "1.5s");
        assert_eq!(fmt_duration(61_000), "1m1s");
    }
}

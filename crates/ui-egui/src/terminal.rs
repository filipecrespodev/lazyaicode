//! A aba Terminal: um shell no checkout da thread, desenhado a partir da
//! grade do `lazyaicode-term`. Os bytes chegam pelo evento `term-data`; as
//! teclas saem pelo comando `terminal.write`.

use base64::Engine as _;
use egui::{Color32, FontFamily, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use lazyaicode_term::{key_bytes, Key, Mods, Term};
use serde_json::{json, Value};

use crate::app::LazyApp;

pub struct TermView {
    pub id: String,
    pub cwd: String,
    pub term: Term,
    /// pedido ao motor já foi feito
    spawned: bool,
    cols: u16,
    rows: u16,
    selection: Option<((u16, u16), (u16, u16))>,
}

impl TermView {
    pub fn new(id: &str, cwd: &str) -> Self {
        Self { id: id.into(), cwd: cwd.into(), term: Term::new(24, 80), spawned: false, cols: 80, rows: 24, selection: None }
    }

    pub fn feed_b64(&mut self, b64: &str) {
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
            self.term.feed(&bytes);
        }
    }
}

/// Um terminal por pasta (a thread e a tela do repositório no mesmo
/// checkout dividem o shell).
pub fn term_id_for(path: &str) -> String {
    let mut h: u64 = 1469598103934665603;
    for b in path.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(1099511628211);
    }
    format!("term-{h:016x}")
}

pub fn current(app: &LazyApp) -> Option<&TermView> {
    app.current_path().and_then(|p| app.terms.get(&term_id_for(&p)))
}

pub fn write_current(app: &mut LazyApp, text: &str) -> Result<Value, String> {
    let id = current(app).map(|t| t.id.clone()).ok_or("terminal fechado")?;
    app.fire("terminal.write", json!({ "termId": id, "data": text }));
    Ok(Value::Null)
}

pub fn restart_current(app: &mut LazyApp) -> Result<Value, String> {
    let path = app.current_path().ok_or("sem pasta")?;
    let id = term_id_for(&path);
    app.fire("terminal.kill", json!({ "termId": id }));
    app.terms.remove(&id);
    Ok(Value::Null)
}

fn key_of(k: egui::Key) -> Option<Key> {
    use egui::Key as E;
    Some(match k {
        E::Enter => Key::Enter,
        E::Tab => Key::Tab,
        E::Backspace => Key::Backspace,
        E::Escape => Key::Escape,
        E::ArrowUp => Key::Up,
        E::ArrowDown => Key::Down,
        E::ArrowLeft => Key::Left,
        E::ArrowRight => Key::Right,
        E::Home => Key::Home,
        E::End => Key::End,
        E::PageUp => Key::PageUp,
        E::PageDown => Key::PageDown,
        E::Insert => Key::Insert,
        E::Delete => Key::Delete,
        E::F1 => Key::F(1),
        E::F2 => Key::F(2),
        E::F3 => Key::F(3),
        E::F4 => Key::F(4),
        E::F5 => Key::F(5),
        E::F6 => Key::F(6),
        E::F7 => Key::F(7),
        E::F8 => Key::F(8),
        E::F9 => Key::F(9),
        E::F10 => Key::F(10),
        E::F11 => Key::F(11),
        E::F12 => Key::F(12),
        other => {
            let name = other.name();
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Key::Char(c.to_ascii_lowercase()),
                _ => match other {
                    E::OpenBracket => Key::Char('['),
                    E::CloseBracket => Key::Char(']'),
                    E::Backslash => Key::Char('\\'),
                    E::Minus => Key::Char('-'),
                    E::Space => Key::Char(' '),
                    _ => return None,
                },
            }
        }
    })
}

fn color(c: lazyaicode_term::Color, ansi: &[[u8; 3]; 16], default: Color32) -> Color32 {
    let [r, g, b] = lazyaicode_term::rgb(c, ansi, [default.r(), default.g(), default.b()]);
    Color32::from_rgb(r, g, b)
}

/// Desenha o terminal da pasta `path` ocupando o espaço restante.
pub fn show(app: &mut LazyApp, ui: &mut egui::Ui, path: &str) {
    let id = term_id_for(path);
    let tk = app.tokens;
    let font = FontId::new(tk.terminal_size, FontFamily::Monospace);
    let (cell_w, cell_h) = ui.ctx().fonts_mut(|f| (f.glyph_width(&font, 'M'), f.row_height(&font)));
    let avail = ui.available_size();
    let cols = ((avail.x - 16.0) / cell_w.max(1.0)).floor().clamp(4.0, 1000.0) as u16;
    let rows = ((avail.y - 12.0) / cell_h.max(1.0)).floor().clamp(2.0, 500.0) as u16;

    let view = app.terms.entry(id.clone()).or_insert_with(|| TermView::new(&id, path));
    let mut calls: Vec<(&str, Value)> = Vec::new();
    if !view.spawned {
        view.spawned = true;
        view.term.resize(rows, cols);
        (view.cols, view.rows) = (cols, rows);
        calls.push(("terminal.spawn", json!({ "termId": id, "cwd": path, "cols": cols, "rows": rows })));
    } else if (cols, rows) != (view.cols, view.rows) {
        (view.cols, view.rows) = (cols, rows);
        view.term.resize(rows, cols);
        calls.push(("terminal.resize", json!({ "termId": id, "cols": cols, "rows": rows })));
    }

    let (rect, resp) = ui.allocate_exact_size(avail, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, tk.p.input);
    let origin = rect.min + Vec2::new(8.0, 6.0);
    let ansi = tk.ansi16();
    let (fg_default, bg_default) = (tk.p.text, tk.p.input);
    let cell_at = |p: Pos2| -> (u16, u16) {
        let c = ((p.x - origin.x) / cell_w).floor().clamp(0.0, f32::from(cols.saturating_sub(1))) as u16;
        let r = ((p.y - origin.y) / cell_h).floor().clamp(0.0, f32::from(rows.saturating_sub(1))) as u16;
        (r, c)
    };
    if resp.drag_started() {
        if let Some(p) = resp.interact_pointer_pos() {
            let c = cell_at(p);
            view.selection = Some((c, c));
        }
    }
    if resp.dragged() {
        if let (Some(p), Some(sel)) = (resp.interact_pointer_pos(), view.selection.as_mut()) {
            sel.1 = cell_at(p);
        }
    }
    if resp.clicked() {
        view.selection = None;
        resp.request_focus();
    }
    let (trows, _) = view.term.size();
    for r in 0..trows.min(rows) {
        let y = origin.y + f32::from(r) * cell_h;
        let mut x = origin.x;
        for cell in view.term.row(r) {
            let w = if cell.wide { cell_w * 2.0 } else { cell_w };
            let (mut fg, mut bg) = (color(cell.fg, &ansi, fg_default), color(cell.bg, &ansi, bg_default));
            if cell.inverse {
                std::mem::swap(&mut fg, &mut bg);
            }
            if cell.dim {
                fg = fg.gamma_multiply(0.6);
            }
            if bg != bg_default {
                painter.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, cell_h)), 0.0, bg);
            }
            if cell.text != " " {
                let f = if cell.bold { FontId::new(tk.terminal_size, FontFamily::Monospace) } else { font.clone() };
                painter.text(Pos2::new(x, y), egui::Align2::LEFT_TOP, &cell.text, f, fg);
            }
            if cell.underline {
                painter.line_segment([Pos2::new(x, y + cell_h - 1.0), Pos2::new(x + w, y + cell_h - 1.0)], Stroke::new(1.0, fg));
            }
            x += w;
        }
    }
    if let Some((a, b)) = view.selection {
        let (s, e) = if a <= b { (a, b) } else { (b, a) };
        for r in s.0..=e.0 {
            let c0 = if r == s.0 { s.1 } else { 0 };
            let c1 = if r == e.0 { e.1 } else { cols.saturating_sub(1) };
            let rr = Rect::from_min_size(
                Pos2::new(origin.x + f32::from(c0) * cell_w, origin.y + f32::from(r) * cell_h),
                Vec2::new(f32::from(c1.saturating_sub(c0) + 1) * cell_w, cell_h),
            );
            painter.rect_filled(rr, 0.0, tk.accent_soft.gamma_multiply(0.8));
        }
    }
    let focused = resp.has_focus();
    if let Some((r, c)) = view.term.cursor() {
        let p = Pos2::new(origin.x + f32::from(c) * cell_w, origin.y + f32::from(r) * cell_h);
        let cur = Rect::from_min_size(p, Vec2::new(cell_w, cell_h));
        if focused {
            painter.rect_filled(cur, 1.0, tk.p.text.gamma_multiply(0.7));
        } else {
            painter.rect_stroke(cur, 1.0, Stroke::new(1.0, tk.p.muted), egui::StrokeKind::Inside);
        }
    }
    if view.term.exited {
        painter.text(
            rect.right_bottom() - Vec2::new(8.0, 8.0),
            egui::Align2::RIGHT_BOTTOM,
            lazyaicode_i18n::t("terminalPane.exited"),
            FontId::proportional(12.0),
            tk.p.muted,
        );
    }
    // foco automático ao abrir a aba
    if !ui.ctx().memory(|m| m.focused().is_some()) {
        resp.request_focus();
    }

    let mut out = String::new();
    if focused {
        // o terminal recebe Tab, setas e Esc em vez de o egui mudar o foco
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(resp.id, egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true });
        });
        let app_cursor = view.term.application_cursor();
        let mut copy = None;
        ui.input(|i| {
            for ev in &i.events {
                match ev {
                    egui::Event::Text(t) => out.push_str(t),
                    egui::Event::Paste(t) => out.push_str(&String::from_utf8_lossy(&view.term.paste_bytes(t))),
                    egui::Event::Copy => {
                        copy = view.selection.map(|(a, b)| view.term.text_between(a, b));
                    }
                    egui::Event::Key { key, pressed: true, modifiers, .. } => {
                        // Ctrl+V cola (o egui já manda o Paste); Ctrl+C com seleção
                        // copia; sem seleção é o ^C de sempre
                        let has_sel = view.selection.is_some_and(|(a, b)| a != b);
                        if modifiers.ctrl && (*key == egui::Key::V || (*key == egui::Key::C && (has_sel || modifiers.shift))) {
                            continue;
                        }
                        let m = Mods { ctrl: modifiers.ctrl, alt: modifiers.alt, shift: modifiers.shift };
                        if let Some(k) = key_of(*key) {
                            let printable = matches!(k, Key::Char(_)) && !modifiers.ctrl && !modifiers.alt;
                            if !printable {
                                if let Some(bytes) = key_bytes(k, m, app_cursor) {
                                    out.push_str(&String::from_utf8_lossy(&bytes));
                                }
                            }
                        }
                    }
                    egui::Event::MouseWheel { delta, .. } if resp.hovered() => {
                        view.term.scroll((delta.y / 3.0).round() as i32);
                    }
                    _ => {}
                }
            }
        });
        if let Some(text) = copy {
            ui.ctx().copy_text(text);
        }
    } else if resp.hovered() {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if dy.abs() > 0.5 {
            view.term.scroll((dy / cell_h).round() as i32);
        }
    }
    if !out.is_empty() {
        if view.term.scrolled() > 0 {
            view.term.scroll(-(lazyaicode_term::SCROLLBACK as i32));
        }
        view.selection = None;
        calls.push(("terminal.write", json!({ "termId": id, "data": out })));
    }
    for (m, p) in calls {
        app.fire(m, p);
    }
}

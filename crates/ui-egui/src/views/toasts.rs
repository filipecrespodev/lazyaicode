//! Avisos no canto da janela.

use egui::{RichText, Stroke};

use crate::app::{LazyApp, ToastKind};

pub fn show(app: &mut LazyApp, ctx: &egui::Context) {
    let now = app.now;
    app.toasts.retain(|t| t.until > now);
    if app.toasts.is_empty() {
        return;
    }
    let tk = app.tokens;
    let screen = ctx.content_rect();
    let mut dismiss = None;
    egui::Area::new(egui::Id::new("toasts")).order(egui::Order::Tooltip).anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0)).show(
        ctx,
        |ui| {
            ui.set_max_width((screen.width() * 0.4).clamp(240.0, 460.0));
            for (i, t) in app.toasts.iter().enumerate() {
                let (c, ic) = match t.kind {
                    ToastKind::Error => (tk.p.red, egui_phosphor::regular::WARNING_CIRCLE),
                    ToastKind::Ok => (tk.p.accent, egui_phosphor::regular::CHECK_CIRCLE),
                    ToastKind::Info => (tk.p.blue, egui_phosphor::regular::INFO),
                };
                let r = egui::Frame::new()
                    .fill(tk.p.elev)
                    .stroke(Stroke::new(1.0, c.gamma_multiply(0.6)))
                    .corner_radius(egui::CornerRadius::same(tk.radius))
                    .shadow(ui.visuals().popup_shadow)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.label(RichText::new(ic).color(c));
                            ui.add(egui::Label::new(RichText::new(&t.text).color(tk.p.text)).wrap().selectable(true));
                        });
                    })
                    .response
                    .interact(egui::Sense::click());
                if r.clicked() {
                    dismiss = Some(i);
                }
                ui.add_space(6.0);
            }
        },
    );
    if let Some(i) = dismiss {
        app.toasts.remove(i);
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(500));
}

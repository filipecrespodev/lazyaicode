//! Diálogos de confirmação e os pedidos de senha do git.

use egui::RichText;
use lazyaicode_i18n::t;

use crate::app::{LazyApp, ToastKind};
use crate::views::widgets;

pub fn show(app: &mut LazyApp, ctx: &egui::Context) {
    crate::views::git::prompts(app, ctx);
    let Some((text, cmd, params)) = app.eph.confirm.clone() else { return };
    let tk = app.tokens;
    let mut done: Option<bool> = None;
    let modal = egui::Modal::new(egui::Id::new("confirm")).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.label(RichText::new(t("dialogs.confirmTitle")).strong().size(tk.text_size + 2.0));
        ui.add_space(6.0);
        ui.add(egui::Label::new(&text).wrap());
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if widgets::danger(ui, &tk, &t("dialogs.confirm")).clicked() {
                done = Some(true);
            }
            if ui.button(t("app.cancel")).clicked() {
                done = Some(false);
            }
        });
    });
    if modal.should_close() && done.is_none() {
        done = Some(false);
    }
    if let Some(yes) = done {
        app.eph.confirm = None;
        app.state.dialog = None;
        if yes {
            let selected = app.state.selected.clone();
            let deleting_selected = cmd == "thread.delete" && params["id"].as_str() == selected.as_deref();
            app.call(&cmd, params, move |app, r| match r {
                Ok(_) => {
                    if deleting_selected {
                        app.state.selected = None;
                    }
                    app.toast("✓", ToastKind::Ok);
                    crate::views::git::refresh_current(app);
                    app.refresh_threads();
                }
                Err(e) => app.toast_err(&e.message),
            });
        }
    }
}

//! A inicialização no navegador e os serviços da plataforma web.

use base64::Engine as _;
use lazyaicode_ui_egui::{Host, Inbox, LazyApp, WsClient};
use serde_json::{json, Value};
use wasm_bindgen::JsCast;

const TOKEN_KEY: &str = "lazyaicode.token";

fn window() -> Option<web_sys::Window> {
    web_sys::window()
}

fn storage() -> Option<web_sys::Storage> {
    window()?.local_storage().ok()?
}

/// `#token=…` na URL (e o tira de lá, para não ficar no histórico).
fn token_from_hash() -> Option<String> {
    let loc = window()?.location();
    let hash = loc.hash().ok()?;
    let token = hash.trim_start_matches('#').split('&').find_map(|kv| kv.strip_prefix("token="))?.to_string();
    let _ = loc.set_hash("");
    Some(token).filter(|t| !t.is_empty())
}

fn ws_url() -> String {
    let loc = window().map(|w| w.location());
    let host = loc.as_ref().and_then(|l| l.host().ok()).unwrap_or_else(|| "127.0.0.1:8080".into());
    let secure = loc.and_then(|l| l.protocol().ok()).is_some_and(|p| p == "https:");
    format!("{}://{host}/ws", if secure { "wss" } else { "ws" })
}

struct WebHost {
    inbox: Inbox,
}

impl Host for WebHost {
    fn pick_folder(&self, _title: &str, _then: &str, _params: Value) {}

    /// O servidor não lê arquivos do navegador: a imagem vai em base64.
    fn pick_images(&self, _then: &str, params: Value) {
        let inbox = self.inbox.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let Some(files) = rfd::AsyncFileDialog::new().add_filter("Imagens", &["png", "jpg", "jpeg", "gif", "webp"]).pick_files().await
            else {
                return;
            };
            for f in files {
                let bytes = f.read().await;
                let name = f.file_name();
                let mime = match name.rsplit('.').next().map(str::to_lowercase).as_deref() {
                    Some("jpg" | "jpeg") => "image/jpeg",
                    Some("gif") => "image/gif",
                    Some("webp") => "image/webp",
                    _ => "image/png",
                };
                let mut p = params.clone();
                p["mime"] = json!(mime);
                p["data"] = json!(base64::engine::general_purpose::STANDARD.encode(bytes));
                p["name"] = json!(name);
                inbox.push("ui.composer.attach-data", p);
            }
        });
    }

    fn clipboard_image(&self) -> Option<Vec<u8>> {
        None
    }

    fn notify(&self, _title: &str, _body: &str) {}

    fn native_decorations(&self) -> bool {
        true
    }

    fn is_web(&self) -> bool {
        true
    }

    fn platform(&self) -> String {
        "web".into()
    }

    fn now_ms(&self) -> i64 {
        js_sys::Date::now() as i64
    }

    fn tz_offset_ms(&self) -> i64 {
        -(js_sys::Date::new_0().get_timezone_offset() as i64) * 60_000
    }
}

/// Antes de conectar: pede o token (se não veio na URL).
enum Screen {
    Login { token: String, error: Option<String> },
    App(Box<LazyApp>),
}

struct WebApp {
    screen: Screen,
}

fn connect(ctx: &egui::Context, token: &str) -> LazyApp {
    let inbox = Inbox::default();
    let c = ctx.clone();
    let client = WsClient::connect(&ws_url(), token, move || c.request_repaint());
    lazyaicode_ui_egui::install(ctx);
    LazyApp::new(Box::new(client), Box::new(WebHost { inbox: inbox.clone() }), inbox)
}

impl eframe::App for WebApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let mut next = None;
        match &mut self.screen {
            Screen::App(app) => {
                app.pump();
                app.ui(ui);
                // token recusado: volta para a tela de login
                if app.connection_refused() {
                    if let Some(s) = storage() {
                        let _ = s.remove_item(TOKEN_KEY);
                    }
                    next = Some(Screen::Login { token: String::new(), error: Some("Token recusado pelo servidor".into()) });
                }
            }
            Screen::Login { token, error } => {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() * 0.3);
                        ui.heading("LazyAICode");
                        ui.label("Cole o token impresso pelo `lazyaicode-cli serve`:");
                        let r = ui.add(egui::TextEdit::singleline(token).password(true).desired_width(360.0));
                        if let Some(e) = error {
                            ui.colored_label(egui::Color32::from_rgb(236, 106, 106), e.as_str());
                        }
                        let go = ui.button("Conectar").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                        if go && !token.trim().is_empty() {
                            if let Some(s) = storage() {
                                let _ = s.set_item(TOKEN_KEY, token.trim());
                            }
                            next = Some(Screen::App(Box::new(connect(&ctx, token.trim()))));
                        }
                    });
                });
            }
        }
        if let Some(s) = next {
            self.screen = s;
        }
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
}

pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Warn).ok();
    wasm_bindgen_futures::spawn_local(async {
        let Some(doc) = window().and_then(|w| w.document()) else { return };
        let Some(canvas) = doc.get_element_by_id("lazyaicode").and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else {
            return;
        };
        let token = token_from_hash().or_else(|| storage().and_then(|s| s.get_item(TOKEN_KEY).ok().flatten()));
        if let (Some(t), Some(s)) = (&token, storage()) {
            let _ = s.set_item(TOKEN_KEY, t);
        }
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(move |cc| {
                    let screen = match token {
                        Some(t) => Screen::App(Box::new(connect(&cc.egui_ctx, &t))),
                        None => Screen::Login { token: String::new(), error: None },
                    };
                    Ok(Box::new(WebApp { screen }))
                }),
            )
            .await;
        if let Some(el) = doc.get_element_by_id("loading") {
            match started {
                Ok(()) => el.remove(),
                Err(e) => el.set_text_content(Some(&format!("Não foi possível iniciar: {e:?}"))),
            }
        }
    });
}

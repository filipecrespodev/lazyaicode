//! O app desktop: a janela (eframe + wgpu), o motor no mesmo processo e,
//! com `--control`, o canal de controle para scripts e agentes.
//!
//! ```sh
//! lazyaicode                                   # abre o app
//! lazyaicode --control 127.0.0.1:7878          # + canal de controle (token em <dados>/control.token)
//! lazyaicode --connect ws://host:8080/ws --token …   # UI ligada a um `lazyaicode-cli serve` remoto
//! lazyaicode --screenshot tela.png --size 1280x800   # renderiza em CPU, sem janela, e sai
//! ```

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod host;
mod local;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use base64::Engine as _;
use lazyaicode_automation::backend::{run_engine, Backend, BoxFut};
use lazyaicode_automation::{server, token};
use lazyaicode_engine::{Engine, EngineOpts};
use lazyaicode_protocol::{ErrorBody, Event};
use lazyaicode_ui_egui::{Client, Inbox, LazyApp, UiMsg, WsClient};
use serde_json::{json, Value};

struct Args {
    data_dir: Option<PathBuf>,
    ephemeral: bool,
    control: Option<SocketAddr>,
    token_file: Option<PathBuf>,
    connect: Option<String>,
    token: Option<String>,
    screenshot: Option<PathBuf>,
    size: (u32, u32),
    frames: u32,
    theme: Option<String>,
    /// comandos de tela rodados antes da captura (`--run 'ui.thread.select {"id":"…"}'`)
    run: Vec<(String, Value)>,
}

const USAGE: &str = "uso: lazyaicode [--data-dir DIR] [--ephemeral] [--control ADDR] [--token-file F] [--connect WS_URL --token T] [--screenshot OUT.png [--size WxH] [--frames N] [--theme ID] [--run 'CMD {json}']...]";

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        data_dir: None,
        ephemeral: false,
        control: None,
        token_file: None,
        connect: None,
        token: None,
        screenshot: None,
        size: (1280, 800),
        frames: 40,
        theme: None,
        run: vec![],
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = || it.next().ok_or_else(|| format!("{arg} precisa de um valor\n{USAGE}"));
        match arg.as_str() {
            "--data-dir" => a.data_dir = Some(val()?.into()),
            "--ephemeral" => a.ephemeral = true,
            "--control" => a.control = Some(val()?.parse().map_err(|e| format!("--control: {e}"))?),
            "--token-file" => a.token_file = Some(val()?.into()),
            "--connect" => a.connect = Some(val()?),
            "--token" => a.token = Some(val()?),
            "--screenshot" => a.screenshot = Some(val()?.into()),
            "--frames" => a.frames = val()?.parse().map_err(|e| format!("--frames: {e}"))?,
            "--theme" => a.theme = Some(val()?),
            "--run" => {
                let v = val()?;
                let (cmd, params) = v.split_once(' ').unwrap_or((v.as_str(), "null"));
                let params: Value = serde_json::from_str(params).map_err(|e| format!("--run {cmd}: {e}"))?;
                a.run.push((cmd.to_string(), params));
            }
            "--size" => {
                let v = val()?;
                let (w, h) = v.split_once('x').ok_or("--size: use LxA, ex. 1280x800")?;
                a.size = (w.parse().map_err(|_| "--size: largura")?, h.parse().map_err(|_| "--size: altura")?);
            }
            "--version" | "-V" => {
                println!("lazyaicode {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("argumento desconhecido: {other}\n{USAGE}")),
        }
    }
    Ok(a)
}

/// O motor para o canal de controle, com os `ui.*` atendidos pela janela.
struct DesktopBackend {
    engine: Engine,
    inbox: Inbox,
    wake: local::Wake,
}

impl Backend for DesktopBackend {
    fn call(&self, method: String, params: Value) -> BoxFut<Result<Value, ErrorBody>> {
        let (engine, inbox, wake) = (self.engine.clone(), self.inbox.clone(), self.wake.clone());
        Box::pin(async move {
            if !method.starts_with("ui.") {
                return run_engine(&engine, &method, params).await;
            }
            let (tx, rx) = std::sync::mpsc::channel();
            inbox.push_msg(UiMsg { command: method, params, reply: Some(tx) });
            if let Some(ctx) = wake.get() {
                ctx.request_repaint();
            }
            match tokio::task::spawn_blocking(move || rx.recv_timeout(Duration::from_secs(30))).await {
                Ok(Ok(r)) => r,
                _ => Err(ErrorBody { code: "timeout".into(), message: "a janela não respondeu (minimizada?)".into(), data: None }),
            }
        })
    }

    fn events(&self) -> tokio::sync::broadcast::Receiver<Event> {
        self.engine.subscribe()
    }

    fn extra_methods(&self) -> Vec<Value> {
        lazyaicode_ui_egui::commands::ui_command_infos()
            .into_iter()
            .map(|c| json!({ "method": c.id, "doc": c.doc, "params": c.params }))
            .collect()
    }
}

/// Renderiza o app em CPU (sem janela nem GPU): roda `frames` quadros num
/// contexto egui próprio e rasteriza o último.
fn cpu_frame(app: &mut LazyApp, size: (u32, u32), frames: u32, between: Duration) -> Option<Vec<u8>> {
    cpu_frame_with(app, size, frames, between, &[])
}

/// Como [`cpu_frame`], rodando os comandos de tela de `run` a partir do
/// quadro 10 (depois que a abertura carregou os dados), um a cada 10 quadros:
/// abrir um workspace carrega as threads antes de selecionar uma.
fn cpu_frame_with(app: &mut LazyApp, size: (u32, u32), frames: u32, between: Duration, run: &[(String, Value)]) -> Option<Vec<u8>> {
    let ctx = egui::Context::default();
    lazyaicode_ui_egui::install(&ctx);
    app.invalidate_theme();
    let mut tex = lazyaicode_render_cpu::Textures::new();
    let ppp = 1.0;
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size.0 as f32, size.1 as f32));
    let mut last = None;
    for i in 0..frames.max(2) {
        if i >= 10 && i % 10 == 0 {
            if let Some((cmd, params)) = run.get((i / 10 - 1) as usize) {
                app.inbox().push(cmd, params.clone());
            }
        }
        app.pump();
        let input = egui::RawInput {
            screen_rect: Some(rect),
            time: Some(f64::from(i) * between.as_secs_f64()),
            focused: true,
            ..Default::default()
        };
        let out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::no_frame().show(ui, |ui| app.ui(ui));
        });
        let mut out = out;
        tex.apply(&out.textures_delta);
        out.textures_delta.clear();
        last = Some(ctx.tessellate(out.shapes, ppp));
        if i + 1 < frames {
            std::thread::sleep(between);
        }
    }
    app.invalidate_theme();
    let bg = ctx.global_style().visuals.panel_fill;
    lazyaicode_render_cpu::render_png(&last?, &tex, [size.0, size.1], ppp, bg)
}

struct DesktopApp {
    app: LazyApp,
    engine: Option<Engine>,
    rt: tokio::runtime::Runtime,
    shot_waiters: Vec<std::sync::mpsc::Sender<Result<Value, ErrorBody>>>,
}

fn png_of(img: &egui::ColorImage) -> Option<Vec<u8>> {
    let [w, h] = img.size;
    let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    let buf = image::RgbaImage::from_raw(w as u32, h as u32, raw)?;
    let mut out = std::io::Cursor::new(Vec::new());
    buf.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.pump();
        self.app.ui(ui);
        let ctx = ui.ctx().clone();
        let (waiters, cpu) = self.app.take_screenshot_waiters();
        if !waiters.is_empty() {
            if cpu {
                let r = ctx.content_rect();
                let size = ((r.width() * ctx.pixels_per_point()) as u32, (r.height() * ctx.pixels_per_point()) as u32);
                let png = cpu_frame(&mut self.app, size, 2, Duration::ZERO);
                for w in waiters {
                    let _ = w.send(match &png {
                        Some(p) => Ok(json!({ "png": base64::engine::general_purpose::STANDARD.encode(p), "renderer": "cpu" })),
                        None => Err(ErrorBody { code: "failed".into(), message: "renderização em CPU falhou".into(), data: None }),
                    });
                }
            } else {
                self.shot_waiters.extend(waiters);
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
        }
        let shots: Vec<Arc<egui::ColorImage>> = ctx.input(|i| {
            i.raw
                .events
                .iter()
                .filter_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })
                .collect()
        });
        for img in shots {
            let png = png_of(&img);
            for w in self.shot_waiters.drain(..) {
                let _ = w.send(match &png {
                    Some(p) => Ok(json!({ "png": base64::engine::general_purpose::STANDARD.encode(p), "renderer": "wgpu" })),
                    None => Err(ErrorBody { code: "failed".into(), message: "não foi possível codificar a captura".into(), data: None }),
                });
            }
        }
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }

    fn on_exit(&mut self) {
        if let Some(e) = self.engine.take() {
            self.rt.block_on(async { tokio::time::timeout(Duration::from_secs(5), e.shutdown()).await.ok() });
        }
    }
}

fn icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(include_bytes!("../../../assets/icons/128x128.png")).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width: w, height: h })
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    lazyaicode_engine::workspace::init_platform();
    let rt =
        tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("lazyaicode-engine").build().map_err(|e| e.to_string())?;
    let wake: local::Wake = Arc::new(OnceLock::new());
    let inbox = Inbox::default();
    let decorations = cfg!(target_os = "macos");
    let host = Box::new(host::DesktopHost { inbox: inbox.clone(), decorations });

    let (client, engine): (Box<dyn Client>, Option<Engine>) = match &args.connect {
        Some(url) => {
            let token = args
                .token
                .clone()
                .or_else(|| args.token_file.as_ref().and_then(|f| token::read(f).ok()))
                .ok_or("--connect precisa de --token ou --token-file")?;
            let w = wake.clone();
            (
                Box::new(WsClient::connect(url, &token, move || {
                    if let Some(c) = w.get() {
                        c.request_repaint()
                    }
                })),
                None,
            )
        }
        None => {
            let _guard = rt.enter();
            let engine = Engine::new(EngineOpts { data_dir: args.data_dir.clone(), ephemeral: args.ephemeral });
            (Box::new(local::LocalClient::new(engine.clone(), rt.handle().clone(), wake.clone())), Some(engine))
        }
    };

    let mut app = LazyApp::new(client, host, inbox.clone());
    if let Some(t) = &args.theme {
        app.state.appearance.theme = t.clone();
    }

    if let Some(out) = &args.screenshot {
        let png =
            cpu_frame_with(&mut app, args.size, args.frames.max(20 + 10 * args.run.len() as u32), Duration::from_millis(60), &args.run)
                .ok_or("renderização em CPU falhou")?;
        std::fs::write(out, png).map_err(|e| format!("{}: {e}", out.display()))?;
        eprintln!("{} ({}x{})", out.display(), args.size.0, args.size.1);
        if let Some(e) = &engine {
            rt.block_on(e.shutdown());
        }
        return Ok(());
    }

    if let (Some(addr), Some(e)) = (args.control, &engine) {
        let t = token::generate();
        let tf = args.token_file.clone().unwrap_or_else(|| e.data_dir().join("control.token"));
        token::write(&tf, &t).map_err(|err| format!("token {}: {err}", tf.display()))?;
        eprintln!("canal de controle em {addr} (token em {})", tf.display());
        let backend = Arc::new(DesktopBackend { engine: e.clone(), inbox: inbox.clone(), wake: wake.clone() });
        rt.spawn(async move {
            if let Err(err) = server::serve_tcp(addr, backend, t).await {
                log::error!("canal de controle: {err}");
            }
        });
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("LazyAICode")
        .with_app_id("lazyaicode")
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([640.0, 420.0])
        .with_decorations(decorations)
        .with_transparent(false);
    if let Some(i) = icon() {
        viewport = viewport.with_icon(Arc::new(i));
    }
    let options = eframe::NativeOptions { viewport, renderer: eframe::Renderer::Wgpu, ..Default::default() };
    let w = wake.clone();
    eframe::run_native(
        "LazyAICode",
        options,
        Box::new(move |cc| {
            let _ = w.set(cc.egui_ctx.clone());
            lazyaicode_ui_egui::install(&cc.egui_ctx);
            Ok(Box::new(DesktopApp { app, engine, rt, shot_waiters: vec![] }))
        }),
    )
    .map_err(|e| e.to_string())
}

fn main() {
    if lazyaicode_git::answer_askpass() {
        return;
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    if let Err(e) = run() {
        eprintln!("erro: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn engine_and_screen_commands_together_pass_500_without_collisions() {
        let engine = lazyaicode_engine::commands();
        let screen = lazyaicode_ui_egui::commands::ui_command_infos();
        let mut ids = std::collections::HashSet::new();
        for c in engine.iter().chain(&screen) {
            assert!(ids.insert(c.id.clone()), "id repetido entre motor e tela: {}", c.id);
        }
        eprintln!("{} comandos ({} do motor + {} de tela)", ids.len(), engine.len(), screen.len());
        assert!(ids.len() > 500, "{} comandos", ids.len());
    }
}

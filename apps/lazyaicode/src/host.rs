//! Os serviços da plataforma para a UI: diálogos nativos (rfd), imagem da
//! área de transferência (arboard) e notificações do sistema.

use lazyaicode_ui_egui::{Host, Inbox};
use serde_json::{json, Value};

pub struct DesktopHost {
    pub inbox: Inbox,
    pub decorations: bool,
}

fn merge(mut params: Value, k: &str, v: Value) -> Value {
    if !params.is_object() {
        params = json!({});
    }
    params[k] = v;
    params
}

impl Host for DesktopHost {
    fn pick_folder(&self, title: &str, then: &str, params: Value) {
        let (inbox, title, then) = (self.inbox.clone(), title.to_string(), then.to_string());
        std::thread::spawn(move || {
            if let Some(dir) = rfd::FileDialog::new().set_title(title).pick_folder() {
                inbox.push(&then, merge(params, "path", json!(dir.to_string_lossy())));
            }
        });
    }

    fn pick_images(&self, then: &str, params: Value) {
        let (inbox, then) = (self.inbox.clone(), then.to_string());
        std::thread::spawn(move || {
            if let Some(files) = rfd::FileDialog::new().add_filter("Imagens", &["png", "jpg", "jpeg", "gif", "webp"]).pick_files() {
                let paths: Vec<String> = files.iter().map(|f| f.to_string_lossy().to_string()).collect();
                inbox.push(&then, merge(params, "paths", json!(paths)));
            }
        });
    }

    fn clipboard_image(&self) -> Option<Vec<u8>> {
        let img = arboard::Clipboard::new().ok()?.get_image().ok()?;
        let buf = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned())?;
        let mut out = std::io::Cursor::new(Vec::new());
        buf.write_to(&mut out, image::ImageFormat::Png).ok()?;
        Some(out.into_inner())
    }

    fn notify(&self, title: &str, body: &str) {
        let (title, body) = (title.to_string(), body.to_string());
        std::thread::spawn(move || {
            let _ = if cfg!(target_os = "macos") {
                let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
                lazyaicode_platform::command("osascript")
                    .args(["-e", &format!("display notification \"{}\" with title \"{}\"", esc(&body), esc(&title))])
                    .status()
            } else if cfg!(windows) {
                // sem dependência de toast: o PowerShell mostra o balão do sistema
                let script = format!(
                    "[void][System.Reflection.Assembly]::LoadWithPartialName('System.Windows.Forms');$n=New-Object System.Windows.Forms.NotifyIcon;$n.Icon=[System.Drawing.SystemIcons]::Information;$n.Visible=$true;$n.ShowBalloonTip(5000,'{}','{}',[System.Windows.Forms.ToolTipIcon]::Info);Start-Sleep 6",
                    title.replace('\'', "''"),
                    body.replace('\'', "''")
                );
                lazyaicode_platform::command("powershell").args(["-NoProfile", "-Command", &script]).status()
            } else {
                lazyaicode_platform::command("notify-send").args(["-a", "LazyAICode", &title, &body]).status()
            };
        });
    }

    fn native_decorations(&self) -> bool {
        self.decorations
    }

    fn is_web(&self) -> bool {
        false
    }

    fn platform(&self) -> String {
        if lazyaicode_platform::is_wsl() {
            "wsl".into()
        } else {
            std::env::consts::OS.into()
        }
    }

    fn now_ms(&self) -> i64 {
        chrono::Utc::now().timestamp_millis()
    }

    /// Uma fonte CJK do sistema (a embutida não tem chinês/japonês/coreano).
    fn extra_fonts(&self) -> Vec<(String, Vec<u8>, u32)> {
        const CANDIDATES: [&str; 10] = [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/local/share/fonts/noto/NotoSansCJK-Regular.ttc",
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "C:\\Windows\\Fonts\\msyh.ttc",
            "C:\\Windows\\Fonts\\simsun.ttc",
            "/mnt/c/Windows/Fonts/msyh.ttc",
        ];
        CANDIDATES.iter().find_map(|p| std::fs::read(p).ok().map(|b| ("cjk".to_string(), b, 0))).into_iter().collect()
    }

    fn tz_offset_ms(&self) -> i64 {
        i64::from(chrono::Local::now().offset().local_minus_utc()) * 1000
    }
}

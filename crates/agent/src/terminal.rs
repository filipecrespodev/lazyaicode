//! Terminais interativos (PTY) por thread, abertos no checkout da thread.

use anyhow::{anyhow, Result};
use base64::Engine;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;

struct Term {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

#[derive(Default)]
pub struct Terminals {
    terms: parking_lot::Mutex<HashMap<String, Term>>,
}

pub type DataSink = Arc<dyn Fn(&str, Option<String>) + Send + Sync>;

/// O shell do terminal, aberto em `cwd`. Pasta de uma distro WSL
/// (`\\wsl.localhost\<distro>\...`): o shell roda dentro da distro, não no
/// Windows sobre o 9P.
fn shell_command(cwd: &str) -> CommandBuilder {
    #[cfg(windows)]
    if let Some((distro, linux)) = lazyaicode_platform::wslpath::parse_unc(cwd) {
        let mut cmd = CommandBuilder::new("wsl.exe");
        cmd.args(["-d", &distro, "--cd", &linux]);
        // TERM/COLORTERM (definidas por quem chama) só atravessam pelo WSLENV.
        let prev = std::env::var("WSLENV").unwrap_or_default();
        let sep = if prev.is_empty() { "" } else { ":" };
        cmd.env("WSLENV", format!("{prev}{sep}TERM/u:COLORTERM/u"));
        return cmd;
    }
    let mut cmd = CommandBuilder::new(lazyaicode_platform::default_shell());
    cmd.cwd(cwd);
    cmd
}

impl Terminals {
    /// `on_data(id, Some(base64))` a cada leitura; `on_data(id, None)` no fim.
    pub fn spawn(&self, id: &str, cwd: &str, cols: u16, rows: u16, on_data: DataSink) -> Result<()> {
        let pty = native_pty_system();
        let pair = pty.openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
        let mut cmd = shell_command(cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let tid = id.to_string();
        std::thread::spawn(move || {
            let mut buf = [0u8; 16384];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => on_data(&tid, Some(base64::engine::general_purpose::STANDARD.encode(&buf[..n]))),
                }
            }
            on_data(&tid, None);
        });
        self.terms.lock().insert(id.to_string(), Term { master: pair.master, writer, child });
        Ok(())
    }

    pub fn write(&self, id: &str, data: &str) -> Result<()> {
        let mut terms = self.terms.lock();
        let t = terms.get_mut(id).ok_or_else(|| anyhow!("terminal fechado"))?;
        t.writer.write_all(data.as_bytes())?;
        t.writer.flush()?;
        Ok(())
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<()> {
        let terms = self.terms.lock();
        let t = terms.get(id).ok_or_else(|| anyhow!("terminal fechado"))?;
        t.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
        Ok(())
    }

    pub fn kill(&self, id: &str) {
        if let Some(mut t) = self.terms.lock().remove(id) {
            let _ = t.child.kill();
        }
    }
}

impl Terminals {
    /// Fecha todos (ao sair do app).
    pub fn kill_all(&self) {
        for (_, mut t) in self.terms.lock().drain() {
            let _ = t.child.kill();
        }
    }

    pub fn ids(&self) -> Vec<String> {
        self.terms.lock().keys().cloned().collect()
    }
}

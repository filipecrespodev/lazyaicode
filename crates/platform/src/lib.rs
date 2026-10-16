//! Diferenças entre Linux, macOS, Windows, FreeBSD e WSL num lugar só.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod wslpath;

use std::path::{Path, PathBuf};
use std::process::Command;

/// `Command` que, no Windows, não abre uma janela de console a cada `git` ou
/// `claude` (o app é GUI; sem a flag cada chamada pisca um terminal).
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Rodando dentro do WSL (o binário Linux, exibido no Windows pelo WSLg).
pub fn is_wsl() -> bool {
    if cfg!(not(target_os = "linux")) {
        return false;
    }
    std::env::var_os("WSL_DISTRO_NAME").is_some()
        || std::fs::read_to_string("/proc/sys/kernel/osrelease").map(|s| is_wsl_kernel(&s)).unwrap_or(false)
}

pub fn is_wsl_kernel(osrelease: &str) -> bool {
    let s = osrelease.to_lowercase();
    s.contains("microsoft") || s.contains("wsl")
}

/// Ajustes de ambiente antes de abrir a janela. No WSLg o Vulkan (dzn) do
/// wgpu costuma falhar ou abrir a janela preta; o backend GL é o contorno
/// conhecido. Não sobrescreve o que o usuário já definiu.
pub fn prepare_env() {
    if is_wsl() && std::env::var_os("WGPU_BACKEND").is_none() {
        std::env::set_var("WGPU_BACKEND", "gl");
    }
}

/// Executável no PATH, com o nome exato.
pub fn which(name: &str) -> Option<PathBuf> {
    in_path(name)
}

fn in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(name)).find(|p| p.is_file())
}

/// Como abrir um arquivo/pasta no programa padrão de cada sistema. No WSL o
/// "padrão" é o do Windows: `wslview` (wslu) quando existe, senão o Explorer
/// com o caminho convertido por `wslpath -w`.
pub fn open_command(path: &Path) -> Command {
    if cfg!(target_os = "macos") {
        let mut c = command("open");
        c.arg(path);
        return c;
    }
    if cfg!(windows) {
        let mut c = command("explorer");
        c.arg(path);
        return c;
    }
    if is_wsl() {
        if in_path("wslview").is_some() {
            let mut c = command("wslview");
            c.arg(path);
            return c;
        }
        let win = command("wslpath")
            .arg("-w")
            .arg(path)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        if let Some(w) = win {
            let mut c = command("explorer.exe");
            c.arg(w);
            return c;
        }
    }
    let mut c = command("xdg-open");
    c.arg(path);
    c
}

/// Shell do terminal integrado.
pub fn default_shell() -> String {
    if cfg!(windows) {
        if in_path("pwsh.exe").is_some() {
            return "pwsh.exe".into();
        }
        return "powershell.exe".into();
    }
    std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| {
        if Path::new("/bin/zsh").exists() && cfg!(target_os = "macos") {
            "/bin/zsh".into()
        } else {
            "/bin/bash".into()
        }
    })
}

/// Nomes do executável do Claude Code no PATH.
pub fn claude_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["claude.exe", "claude.cmd", "claude"]
    } else {
        &["claude"]
    }
}

/// Locais de instalação conhecidos, para quando o PATH do app gráfico não
/// inclui a pasta do instalador (comum no macOS e em lançadores Linux).
pub fn claude_install_locations(home: &Path) -> Vec<PathBuf> {
    if cfg!(windows) {
        let mut v = vec![home.join(".local/bin/claude.exe"), home.join(".claude/local/claude.exe")];
        if let Some(appdata) = std::env::var_os("APPDATA") {
            v.push(PathBuf::from(appdata).join("npm/claude.cmd"));
        }
        return v;
    }
    vec![
        home.join(".local/bin/claude"),
        home.join(".claude/local/claude"),
        home.join(".npm-global/bin/claude"),
        home.join(".bun/bin/claude"),
        home.join(".volta/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No CI do WSL a variável `EXPECT_WSL=1` garante que a detecção funciona
    /// de verdade, e não só no teste da string do kernel.
    #[test]
    fn detects_wsl_when_running_inside_it() {
        if std::env::var("EXPECT_WSL").as_deref() == Ok("1") {
            assert!(is_wsl());
            prepare_env();
            assert_eq!(std::env::var("WGPU_BACKEND").as_deref(), Ok("gl"));
        }
    }

    #[test]
    fn recognises_wsl1_and_wsl2_kernels() {
        assert!(is_wsl_kernel("5.15.167.4-microsoft-standard-WSL2"));
        assert!(is_wsl_kernel("4.4.0-19041-Microsoft"));
        assert!(!is_wsl_kernel("7.0.0-38-generic"));
    }

    #[test]
    fn opens_with_the_platform_opener() {
        let c = open_command(Path::new("/tmp"));
        let prog = c.get_program().to_string_lossy().to_string();
        if cfg!(target_os = "macos") {
            assert_eq!(prog, "open");
        } else if cfg!(windows) {
            assert_eq!(prog, "explorer");
        } else if is_wsl() {
            // no WSL o arquivo abre no Windows, nunca num xdg-open sem desktop
            assert!(prog == "wslview" || prog == "explorer.exe", "{prog}");
        } else {
            assert_eq!(prog, "xdg-open");
        }
    }
}

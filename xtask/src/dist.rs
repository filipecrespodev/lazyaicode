//! Pacotes de distribuição. Cada formato chama a ferramenta padrão dele
//! (as receitas ficam em `packaging/`); o resultado vai para
//! `packaging/out/`.

use std::process::Command;

use crate::run as sh;
use crate::{cargo, root};

fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        "freebsd" => "freebsd",
        _ => "linux",
    }
}

fn build_release() -> Result<(), String> {
    sh(cargo().args(["build", "--release", "-p", "lazyaicode", "-p", "lazyaicode-cli"]))
}

fn script(name: &str) -> Result<(), String> {
    sh(Command::new("sh").arg(root().join("packaging").join(name)).env("VERSION", version()))
}

fn tarball() -> Result<(), String> {
    build_release()?;
    let out = root().join("packaging/out");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let name = format!("lazyaicode-{}-{}-{}", version(), os(), std::env::consts::ARCH);
    let stage = out.join(&name);
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    let exe = if cfg!(windows) { ".exe" } else { "" };
    for bin in ["lazyaicode", "lazyaicode-cli"] {
        std::fs::copy(root().join(format!("target/release/{bin}{exe}")), stage.join(format!("{bin}{exe}")))
            .map_err(|e| format!("{bin}: {e}"))?;
    }
    for f in ["README.md", "LICENSE"] {
        let _ = std::fs::copy(root().join(f), stage.join(f));
    }
    if os() == "linux" || os() == "freebsd" {
        let _ = std::fs::copy(root().join("packaging/linux/lazyaicode.desktop"), stage.join("lazyaicode.desktop"));
        let _ = std::fs::copy(root().join("assets/icons/128x128.png"), stage.join("lazyaicode.png"));
    }
    if cfg!(windows) {
        sh(Command::new("powershell").args([
            "-NoProfile",
            "-Command",
            &format!("Compress-Archive -Force -Path '{}' -DestinationPath '{}.zip'", stage.display(), stage.display()),
        ]))
    } else {
        let archive = out.join(format!("{name}.tar.gz"));
        sh(Command::new("tar").arg("-czf").arg(&archive).arg("-C").arg(&out).arg(&name))
    }
}

pub fn run(format: &str) -> Result<(), String> {
    match format {
        "tarball" => tarball(),
        "deb" => {
            build_release()?;
            sh(cargo().args(["deb", "-p", "lazyaicode", "--no-build", "--output", "packaging/out/"]))
        }
        "rpm" => {
            build_release()?;
            sh(cargo().args(["generate-rpm", "-p", "apps/lazyaicode", "-o", "packaging/out/"]))
        }
        "appimage" => {
            build_release()?;
            script("linux/appimage.sh")
        }
        "flatpak" => script("linux/flatpak.sh"),
        "dmg" => {
            build_release()?;
            script("macos/dmg.sh")
        }
        "msi" => {
            build_release()?;
            sh(cargo().args(["wix", "-p", "lazyaicode", "--no-build", "--nocapture", "--output", "packaging/out/"]))
        }
        "docker" => sh(Command::new("docker").args(["build", "-t", &format!("lazyaicode-web:{}", version()), "."])),
        other => Err(format!("formato desconhecido: {other} (tarball, deb, rpm, appimage, flatpak, dmg, msi, docker)")),
    }
}

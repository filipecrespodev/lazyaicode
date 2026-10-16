//! As camadas dos crates. Um crate só depende de camadas abaixo da dele;
//! nada abaixo da UI usa egui/eframe/winit/wgpu/rfd; e a UI não liga o motor
//! (fala com ele só por comandos, pelo `Client`).

use std::collections::BTreeMap;

const LAYERS: [(&str, u8); 13] = [
    ("lazyaicode-platform", 0),
    ("lazyaicode-protocol", 0),
    ("lazyaicode-i18n", 0),
    ("lazyaicode-term", 0),
    ("lazyaicode-testkit", 0),
    ("lazyaicode-doc", 1),
    ("lazyaicode-git", 1),
    ("lazyaicode-agent", 1),
    ("lazyaicode-engine", 2),
    ("lazyaicode-automation", 3),
    ("lazyaicode-render-cpu", 4),
    ("lazyaicode-ui-egui", 4),
    // apps ficam acima de tudo (camada 9)
    ("lazyaicode", 9),
];

const UI_LIBS: [&str; 6] = ["egui", "eframe", "winit", "wgpu", "rfd", "egui_extras"];
/// A UI só conhece o contrato e o modelo, nunca o motor.
const UI_FORBIDDEN: [&str; 4] = ["lazyaicode-engine", "lazyaicode-automation", "lazyaicode-git", "lazyaicode-agent"];

fn layer(name: &str) -> Option<u8> {
    LAYERS.iter().find(|l| l.0 == name).map(|l| l.1)
}

pub fn check() -> Result<(), String> {
    let root = crate::root();
    let mut errors = Vec::new();
    let mut seen = BTreeMap::new();
    for dir in ["crates", "apps"] {
        for e in std::fs::read_dir(root.join(dir)).map_err(|e| e.to_string())?.flatten() {
            let manifest = e.path().join("Cargo.toml");
            let Ok(text) = std::fs::read_to_string(&manifest) else { continue };
            let doc: toml::Value = toml::from_str(&text).map_err(|e| format!("{}: {e}", manifest.display()))?;
            let name = doc["package"]["name"].as_str().unwrap_or("").to_string();
            let Some(mine) = (if dir == "apps" { Some(9) } else { layer(&name) }) else {
                errors.push(format!("{name}: crate novo sem camada — registre em xtask/src/layers.rs"));
                continue;
            };
            let mut deps: Vec<String> =
                doc.get("dependencies").and_then(|d| d.as_table()).map(|t| t.keys().cloned().collect()).unwrap_or_default();
            if let Some(targets) = doc.get("target").and_then(|t| t.as_table()) {
                for t in targets.values() {
                    if let Some(d) = t.get("dependencies").and_then(|d| d.as_table()) {
                        deps.extend(d.keys().cloned());
                    }
                }
            }
            for d in &deps {
                if let Some(theirs) = LAYERS.iter().find(|l| l.0 == d).map(|l| l.1) {
                    if theirs >= mine && mine < 9 {
                        errors.push(format!("{name} (L{mine}) depende de {d} (L{theirs})"));
                    }
                }
                if mine < 4 && UI_LIBS.contains(&d.as_str()) {
                    errors.push(format!("{name} (L{mine}) usa {d}: bibliotecas de UI só da camada 4 para cima"));
                }
                if name == "lazyaicode-ui-egui" && UI_FORBIDDEN.contains(&d.as_str()) {
                    errors.push(format!("a UI depende de {d}: ela fala com o motor só por comandos"));
                }
            }
            seen.insert(name, mine);
        }
    }
    if errors.is_empty() {
        eprintln!("camadas ok: {}", seen.iter().map(|(n, l)| format!("{n}=L{l}")).collect::<Vec<_>>().join(" "));
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

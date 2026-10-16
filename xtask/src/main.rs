//! `cargo xtask <tarefa>`: as checagens e o empacotamento do repositório.
//!
//! | tarefa | o que faz |
//! |---|---|
//! | `layers` | confere as camadas dos crates (nenhum depende de um de cima; nada abaixo da UI usa egui) |
//! | `wasm` | `cargo check --target wasm32-unknown-unknown` nos crates que vão para o navegador |
//! | `i18n` | toda chave de tradução usada existe nos quatro idiomas |
//! | `commands` | gera `book/src/reference/commands.md` a partir do registro |
//! | `book` | gera os comandos e roda `mdbook build book` |
//! | `web` | `trunk build --release` da versão web |
//! | `dist <formato>` | pacotes: `tarball`, `deb`, `rpm`, `appimage`, `flatpak`, `dmg`, `msi`, `docker` |
//! | `screenshots` | capturas da documentação, renderizadas em CPU |
//! | `ci` | fmt, clippy, testes, layers, wasm e i18n |

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod dist;
mod layers;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_default()
}

pub fn run(cmd: &mut Command) -> Result<(), String> {
    // a raiz é o padrão; quem pediu outro diretório (o demo das capturas,
    // o build web) fica nele
    if cmd.get_current_dir().is_none() {
        cmd.current_dir(root());
    }
    eprintln!("$ {cmd:?}");
    let st = cmd.status().map_err(|e| format!("{cmd:?}: {e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err(format!("falhou: {cmd:?} ({st})"))
    }
}

pub fn cargo() -> Command {
    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
}

/// Crates que compilam para o navegador.
const WASM_CRATES: [&str; 6] =
    ["lazyaicode-protocol", "lazyaicode-i18n", "lazyaicode-term", "lazyaicode-doc", "lazyaicode-ui-egui", "lazyaicode-web"];

fn wasm() -> Result<(), String> {
    let mut c = cargo();
    c.args(["check", "--target", "wasm32-unknown-unknown"]);
    for k in WASM_CRATES {
        c.args(["-p", k]);
    }
    run(&mut c)
}

/// Chaves `t("…")`/`tf("…")` usadas na UI que faltam em algum idioma.
fn i18n() -> Result<(), String> {
    let mut used = std::collections::BTreeSet::new();
    let mut stack = vec![root().join("crates/ui-egui/src")];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let s = std::fs::read_to_string(&p).unwrap_or_default();
                for pat in ["t(\"", "tf(\""] {
                    let mut rest = s.as_str();
                    while let Some(i) = rest.find(pat) {
                        let prev = rest.get(..i).and_then(|b| b.chars().last());
                        rest = &rest[i + pat.len()..];
                        if prev.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                            continue;
                        }
                        if let Some(end) = rest.find('"') {
                            let k = &rest[..end];
                            if k.contains('.') && !k.contains(' ') && k.chars().next().is_some_and(char::is_alphabetic) {
                                used.insert(k.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    let mut missing = Vec::new();
    for (l, _, _) in lazyaicode_i18n::LOCALES {
        let keys: std::collections::HashSet<&str> = lazyaicode_i18n::keys(l).into_iter().collect();
        for k in &used {
            if !keys.contains(k.as_str()) {
                missing.push(format!("{l}: {k}"));
            }
        }
    }
    if missing.is_empty() {
        eprintln!("i18n: {} chaves usadas, todas nos 4 idiomas", used.len());
        Ok(())
    } else {
        Err(format!("chaves faltando:\n  {}", missing.join("\n  ")))
    }
}

fn label(c: &lazyaicode_protocol::CommandInfo) -> String {
    let l = lazyaicode_i18n::lookup("en", &c.label);
    if l == c.label && c.label.contains('.') && !c.label.contains(' ') {
        String::new()
    } else {
        l
    }
}

/// A referência de comandos do book, gerada do registro.
fn commands_doc() -> Result<(), String> {
    let engine = lazyaicode_engine::commands();
    let screen = lazyaicode_ui_egui::commands::ui_command_infos();
    let mut out = format!(
        "# Referência de comandos\n\n> Gerado por `cargo xtask commands` — não edite à mão.\n\n\
         São **{}** comandos: {} do motor (rodam na CLI, no canal de controle, no MCP e na UI) e {} de tela \
         (`ui.*`, só no app aberto, pelo `ui.run` do canal de controle). Parâmetros com `?` são opcionais.\n\n",
        engine.len() + screen.len(),
        engine.len(),
        screen.len()
    );
    for (title, list) in [("Motor", &engine), ("Tela", &screen)] {
        out.push_str(&format!("## {title}\n\n"));
        let mut groups: Vec<&str> = list.iter().map(|c| c.group.as_str()).collect();
        groups.sort();
        groups.dedup();
        for g in groups {
            out.push_str(&format!("### `{g}`\n\n| Comando | Descrição | Parâmetros | Atalho |\n|---|---|---|---|\n"));
            for c in list.iter().filter(|c| c.group == g) {
                let params: Vec<String> = c.params.iter().map(|p| format!("`{}`{}", p.name, if p.required { "" } else { "?" })).collect();
                let mut desc = label(c);
                if !c.doc.is_empty() {
                    if !desc.is_empty() {
                        desc.push_str(" — ");
                    }
                    desc.push_str(&c.doc.replace('|', "\\|").replace('\n', " "));
                }
                let flags = match (c.read_only, c.destructive) {
                    (true, _) => " 🔎",
                    (_, true) => " ⚠",
                    _ => "",
                };
                out.push_str(&format!(
                    "| `{}`{flags} | {} | {} | {} |\n",
                    c.id,
                    desc,
                    params.join(", "),
                    c.shortcut.as_deref().map(|s| format!("`{s}`")).unwrap_or_default()
                ));
            }
            out.push('\n');
        }
    }
    out.push_str("🔎 só lê · ⚠ apaga ou reescreve algo difícil de desfazer\n");
    let p = root().join("book/src/reference/commands.md");
    std::fs::create_dir_all(p.parent().unwrap_or(Path::new("."))).map_err(|e| e.to_string())?;
    std::fs::write(&p, out).map_err(|e| e.to_string())?;
    eprintln!("{} ({} comandos)", p.display(), engine.len() + screen.len());
    // atalhos
    let mut kb = String::from(
        "# Atalhos de teclado\n\n> Gerado por `cargo xtask commands`. `Ctrl` é ⌘ no macOS. Troque qualquer um em \
         Configurações → Atalhos (fica em `ui.shortcuts`).\n\n| Atalho | Comando | Ação |\n|---|---|---|\n",
    );
    let mut with: Vec<&lazyaicode_protocol::CommandInfo> = screen.iter().filter(|c| c.shortcut.is_some()).collect();
    with.sort_by_key(|c| c.group.clone());
    for c in with {
        kb.push_str(&format!("| `{}` | `{}` | {} |\n", c.shortcut.as_deref().unwrap_or(""), c.id, label(c)));
    }
    std::fs::write(root().join("book/src/reference/keyboard.md"), kb).map_err(|e| e.to_string())?;
    Ok(())
}

fn book() -> Result<(), String> {
    commands_doc()?;
    run(Command::new("mdbook").args(["build", "book"]))
}

fn web() -> Result<(), String> {
    run(Command::new("trunk").args(["build", "--release"]).current_dir(root().join("apps/lazyaicode-web")))
}

/// Um workspace de demonstração neutro (sem nome nem caminho de ninguém)
/// e as capturas do book, renderizadas em CPU.
#[cfg(unix)]
fn screenshots() -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let demo = std::env::temp_dir().join("lazyaicode-demo");
    let _ = std::fs::remove_dir_all(&demo);
    let code = demo.join("code");
    let gitconfig = demo.join("gitconfig");
    std::fs::create_dir_all(&code).map_err(|e| e.to_string())?;
    std::fs::write(&gitconfig, "[user]\n\tname = Demo\n\temail = demo@example.com\n[init]\n\tdefaultBranch = main\n")
        .map_err(|e| e.to_string())?;
    let git = |dir: &Path, args: &[&str]| -> Result<(), String> {
        run(Command::new("git").args(args).current_dir(dir).env("GIT_CONFIG_GLOBAL", &gitconfig).env("GIT_CONFIG_NOSYSTEM", "1"))
    };
    let api = code.join("api");
    std::fs::create_dir_all(api.join("src")).map_err(|e| e.to_string())?;
    git(&api, &["init", "-q"])?;
    let files = [
        ("README.md", "# api\n"),
        ("src/lib.rs", "pub fn soma(a: i32, b: i32) -> i32 { a + b }\n"),
        ("src/datas.rs", "pub fn hoje() {}\n"),
    ];
    for (i, (f, body)) in files.iter().enumerate() {
        std::fs::write(api.join(f), body).map_err(|e| e.to_string())?;
        git(&api, &["add", "-A"])?;
        git(&api, &["commit", "-qm", ["chore: projeto inicial", "feat: soma", "feat: datas"].get(i).copied().unwrap_or("x")])?;
    }
    git(&api, &["tag", "v0.1.0"])?;
    git(&api, &["checkout", "-qb", "feat/parser"])?;
    std::fs::write(api.join("src/parser.rs"), "pub fn parse() {}\n").map_err(|e| e.to_string())?;
    git(&api, &["add", "-A"])?;
    git(&api, &["commit", "-qm", "feat: parser de datas"])?;
    git(&api, &["checkout", "-q", "main"])?;
    std::fs::write(api.join("README.md"), "# api\n\nServiço de exemplo.\n").map_err(|e| e.to_string())?;
    git(&api, &["commit", "-qam", "docs: README"])?;
    git(&api, &["merge", "-q", "--no-edit", "feat/parser"])?;
    std::fs::write(
        api.join("src/lib.rs"),
        "pub fn soma(a: i32, b: i32) -> i32 {\n    a + b\n}\n\npub fn sub(a: i32, b: i32) -> i32 {\n    a - b\n}\n",
    )
    .map_err(|e| e.to_string())?;
    for p in ["web", "docs"] {
        std::fs::create_dir_all(code.join(p)).map_err(|e| e.to_string())?;
    }
    git(&code.join("web"), &["init", "-q"])?;
    let bin = demo.join("claude");
    std::fs::write(&bin, concat!(
        "#!/bin/sh\nprintf '%s\\n' '{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"demo\",\"mcp_servers\":[{\"name\":\"github\",\"status\":\"connected\"}]}'\n",
        "while read -r l; do case \"$l\" in *'\"type\":\"user\"'*)\n",
        "printf '%s\\n' '{\"type\":\"assistant\",\"message\":{\"id\":\"m1\",\"content\":[{\"type\":\"thinking\",\"thinking\":\"ok\"},{\"type\":\"tool_use\",\"id\":\"t1\",\"name\":\"Read\",\"input\":{\"file_path\":\"src/lib.rs\"}}]}}'\n",
        "printf '%s\\n' '{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"t1\",\"content\":\"pub fn soma\"}]}}'\n",
        "printf '%s\\n' '{\"type\":\"assistant\",\"message\":{\"id\":\"m2\",\"content\":[{\"type\":\"tool_use\",\"id\":\"t2\",\"name\":\"Edit\",\"input\":{\"file_path\":\"src/lib.rs\",\"old_string\":\"a + b\",\"new_string\":\"a + b\"}}]}}'\n",
        "printf '%s\\n' '{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"t2\",\"content\":\"ok\"}]}}'\n",
        "printf '%s\\n' '{\"type\":\"assistant\",\"message\":{\"id\":\"m3\",\"content\":[{\"type\":\"text\",\"text\":\"## Pronto\\n\\nAdicionei `sub` em **src/lib.rs** e os testes passam:\\n\\n```rust\\npub fn sub(a: i32, b: i32) -> i32 { a - b }\\n```\\n\\nQuer que eu abra um PR?\"}]}}'\n",
        "printf '%s\\n' '{\"type\":\"result\",\"is_error\":false,\"duration_ms\":8300,\"total_cost_usd\":0.0184,\"modelUsage\":{\"opus\":{\"contextWindow\":200000}},\"usage\":{\"input_tokens\":900,\"cache_read_input_tokens\":38000}}';;\n",
        "esac; done\n"
    )).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;

    run(cargo().args(["build", "-p", "lazyaicode", "-p", "lazyaicode-cli"]))?;
    let data = demo.join("data");
    let cli = |args: &[&str]| -> Result<serde_json::Value, String> {
        let out = Command::new(root().join("target/debug/lazyaicode-cli"))
            .arg("--data-dir")
            .arg(&data)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", &gitconfig)
            .output()
            .map_err(|e| e.to_string())?;
        serde_json::from_slice(&out.stdout).map_err(|e| format!("{args:?}: {e}: {}", String::from_utf8_lossy(&out.stderr)))
    };
    let ws = cli(&["run", "workspace.create", &format!("path={}", code.display()), "name=Demo"])?;
    let wid = ws["id"].as_str().unwrap_or("").to_string();
    let projects = cli(&["run", "project.list", &format!("workspaceId={wid}")])?;
    let pid =
        projects.as_array().and_then(|a| a.iter().find(|p| p["name"] == "api")).and_then(|p| p["id"].as_str()).unwrap_or("").to_string();
    let t = cli(&["run", "thread.create", &format!("workspaceId={wid}"), &format!("projectId={pid}"), "isolated=false"])?;
    let tid = t["id"].as_str().unwrap_or("").to_string();
    for title in ["Corrigir o parser de datas", "Revisar o README"] {
        let x = cli(&["run", "thread.create", &format!("workspaceId={wid}"), &format!("projectId={pid}"), "isolated=true"])?;
        cli(&["run", "thread.rename", &format!("id={}", x["id"].as_str().unwrap_or("")), &format!("title={title}")])?;
    }

    let out = root().join("book/src/images");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let shots: Vec<(&str, Vec<String>)> = vec![
        (
            "thread.png",
            vec![
                "--run".into(),
                format!("ui.thread.select {{\"id\":\"{tid}\"}}"),
                "--run".into(),
                "ui.thread.send {\"text\":\"Adicione uma função sub em src/lib.rs\"}".into(),
            ],
        ),
        ("git-history.png", vec!["--run".into(), format!("ui.repo.open {{\"projectId\":\"{pid}\"}}")]),
        ("palette.png", vec!["--run".into(), "ui.palette.open {\"query\":\">thread\"}".into()]),
        ("settings.png", vec!["--run".into(), "ui.settings.appearance {}".into()]),
        ("light.png", vec!["--theme".into(), "light".into(), "--run".into(), format!("ui.thread.select {{\"id\":\"{tid}\"}}")]),
    ];
    for (name, extra) in shots {
        let mut c = Command::new(root().join("target/debug/lazyaicode"));
        c.arg("--data-dir").arg(&data).arg("--screenshot").arg(out.join(name)).args(["--size", "1280x800", "--frames", "70"]).args(&extra);
        c.env("GIT_CONFIG_GLOBAL", &gitconfig).env("LAZYAICODE_CLAUDE_BIN", &bin);
        run(&mut c)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn screenshots() -> Result<(), String> {
    Err("as capturas da documentação são geradas no Linux ou no macOS".into())
}

fn ci() -> Result<(), String> {
    run(cargo().args(["fmt", "--all", "--check"]))?;
    layers::check()?;
    i18n()?;
    run(cargo().args(["clippy", "--workspace", "--all-targets", "--keep-going", "--", "-D", "warnings"]))?;
    run(cargo().args(["test", "--workspace", "--no-fail-fast"]))?;
    wasm()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("layers") => layers::check(),
        Some("wasm") => wasm(),
        Some("i18n") => i18n(),
        Some("commands") => commands_doc(),
        Some("book") => book(),
        Some("web") => web(),
        Some("dist") => dist::run(args.get(1).map(String::as_str).unwrap_or("tarball")),
        Some("screenshots") => screenshots(),
        Some("ci") => ci(),
        _ => Err("uso: cargo xtask layers | wasm | i18n | commands | book | web | dist <formato> | screenshots | ci".into()),
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erro: {e}");
            ExitCode::FAILURE
        }
    }
}

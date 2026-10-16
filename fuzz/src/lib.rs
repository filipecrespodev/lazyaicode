//! Os alvos de fuzzing. Cada função recebe bytes arbitrários e exercita um
//! parser que lê dados de fora (agentes, rede, disco, git). A regra é uma
//! só: **nunca entrar em pânico**, nem estourar memória com entrada pequena.
//!
//! - `cargo +nightly fuzz run <alvo>` (libFuzzer, com sanitizers);
//! - `cargo test --manifest-path fuzz/Cargo.toml` roda o modo smoke (Rust
//!   estável): as sementes de `seeds/` e milhares de entradas aleatórias.

use serde_json::Value;

fn json(data: &[u8]) -> Option<Value> {
    serde_json::from_slice(data).ok()
}

fn text(data: &[u8]) -> String {
    String::from_utf8_lossy(data).to_string()
}

/// Linhas do stream-json do Claude Code (ou do ACP traduzido) no reducer.
pub fn transcript(data: &[u8]) {
    let mut t = lazyaicode_doc::transcript::Transcript::new();
    for line in data.split(|b| *b == b'\n') {
        if let Some(v) = json(line) {
            t.apply(&v);
        }
    }
    let _ = t.pending_permission();
    for i in &t.items {
        if let lazyaicode_doc::transcript::Item::Tool { name, input, .. } = i {
            let _ = lazyaicode_doc::transcript::tool_summary(name, input);
        }
    }
}

/// Quadros do canal de controle (linha JSON com limites de tamanho e profundidade).
pub fn frame(data: &[u8]) {
    let s = text(data);
    let _ = lazyaicode_protocol::decode_line::<lazyaicode_protocol::Request>(&s);
    let _ = lazyaicode_protocol::decode_line::<lazyaicode_protocol::Frame>(&s);
    let _ = lazyaicode_protocol::frame::json_depth(&s);
}

/// Patches unificados do git.
pub fn patch(data: &[u8]) {
    let _ = lazyaicode_doc::diff::parse_patch(&text(data));
    let _ = lazyaicode_git::thread::split_patch(&text(data));
}

/// O layout do grafo de commits com pais arbitrários (ciclos, duplicados).
pub fn graph(data: &[u8]) {
    let commits: Vec<(String, Vec<String>)> = data
        .chunks(3)
        .map(|c| {
            let sha = c.first().map_or(0, |b| b % 64).to_string();
            let parents = c.iter().skip(1).map(|b| (b % 64).to_string()).collect();
            (sha, parents)
        })
        .collect();
    let _ = lazyaicode_doc::graph::layout(&commits);
}

/// Bytes do PTY no emulador de terminal, com redimensionamentos no meio.
pub fn term(data: &[u8]) {
    let (rows, cols) = (u16::from(data.first().copied().unwrap_or(24)), u16::from(data.get(1).copied().unwrap_or(80)));
    let mut t = lazyaicode_term::Term::new(rows, cols);
    for (i, chunk) in data.chunks(97).enumerate() {
        t.feed(chunk);
        if i % 5 == 4 {
            t.resize(u16::from(chunk.first().copied().unwrap_or(10)), u16::from(chunk.last().copied().unwrap_or(10)));
        }
    }
    let (r, _) = t.size();
    for row in 0..r {
        let _ = t.row(row);
    }
    let _ = t.text_between((0, 0), (r, 500));
    let _ = t.paste_bytes(&text(data));
}

/// Atualizações de sessão ACP (Codex, Gemini) no tradutor para stream-json.
pub fn acp_translate(data: &[u8]) {
    let mut tr = lazyaicode_agent::acp_translate::Translator::default();
    tr.begin_turn();
    for line in data.split(|b| *b == b'\n') {
        if let Some(v) = json(line) {
            let _ = tr.on_update(&v);
        }
    }
    let _ = tr.end_turn(json(data).as_ref().ok_or_else(|| "x".to_string()), 1);
}

/// Um `.jsonl` de sessão do Claude Code (título, cwd, datas).
pub fn session(data: &[u8]) {
    let Ok(dir) = tempfile_dir() else { return };
    let f = dir.join("s.jsonl");
    if std::fs::write(&f, data).is_ok() {
        let _ = lazyaicode_agent::sessions::read_session(&f);
        let _ = lazyaicode_agent::transcript::load(&f);
    }
}

fn tempfile_dir() -> std::io::Result<std::path::PathBuf> {
    let d = std::env::temp_dir().join(format!("lazyaicode-fuzz-{}", std::process::id()));
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

/// Quadros com prefixo de tamanho do protocolo do agente WSL.
pub fn git_frame(data: &[u8]) {
    let mut r = data;
    for _ in 0..8 {
        match lazyaicode_git::proto::read_frame::<Value>(&mut r) {
            Ok(Some(_)) => {}
            _ => break,
        }
    }
}

/// O frontmatter YAML-ish de skills, agentes e comandos.
pub fn frontmatter(data: &[u8]) {
    let _ = lazyaicode_agent::catalog::frontmatter(&text(data));
}

/// A validação de parâmetros de comandos contra uma descrição fixa.
pub fn params(data: &[u8]) {
    use lazyaicode_protocol::{check_params, ParamInfo, ParamType};
    let spec = vec![
        ParamInfo::req("id", ParamType::String),
        ParamInfo::opt("n", ParamType::Int),
        ParamInfo::opt("mode", ParamType::Enum(vec!["a".into(), "b".into()])),
        ParamInfo::opt("list", ParamType::Strings),
    ];
    if let Some(v) = json(data) {
        let _ = check_params(&spec, &v);
    }
}

/// Caminhos UNC do WSL (`\\wsl.localhost\distro\...`).
pub fn wslpath(data: &[u8]) {
    let s = text(data);
    if let Some((d, l)) = lazyaicode_platform::wslpath::parse_unc(&s) {
        let _ = lazyaicode_platform::wslpath::to_unc(&d, &l);
    }
}

/// `slash_rank` não depende da UI: o mesmo critério da busca da paleta e do
/// menu `/` sobre nomes arbitrários (UTF-8 quebrado, `:` repetidos).
pub fn slash_rank(data: &[u8]) {
    let s = text(data);
    let names: Vec<&str> = s.split(['\n', ' ']).collect();
    let q = names.first().copied().unwrap_or("").to_lowercase();
    for n in names {
        let n = n.to_lowercase();
        let short = n.split_once(':').map_or(n.as_str(), |(_, s)| s);
        let _ = n.starts_with(&q) || short.starts_with(&q) || n.contains(&q);
    }
}

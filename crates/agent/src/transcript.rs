//! Histórico de uma thread lido do armazenamento nativo do Claude Code
//! (`<claudeDir>/projects/<cwd codificado>/<session>.jsonl`). O app não grava
//! transcrição própria: a sessão é do CLI e é a fonte de verdade.

use serde_json::Value;
use std::path::Path;

/// Mensagens `user`/`assistant` da conversa principal, no mesmo formato que
/// chega pelo stream — a UI usa um único renderizador para as duas origens.
/// Linhas lidas por mensagem no máximo (uma imagem colada em base64 cabe).
pub const MAX_LINE: usize = 64 << 20;
/// Mensagens guardadas de uma sessão (as mais recentes).
pub const MAX_MESSAGES: usize = 50_000;

pub fn load(file: &Path) -> Vec<Value> {
    use std::io::BufRead;
    let Ok(f) = std::fs::File::open(file) else { return Vec::new() };
    let mut reader = std::io::BufReader::with_capacity(256 * 1024, f);
    let mut out: std::collections::VecDeque<Value> = std::collections::VecDeque::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        // teto por linha: o resto de uma linha gigante é pulado
        match std::io::Read::take(&mut reader, MAX_LINE as u64 + 1).read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) if buf.len() > MAX_LINE && buf.last() != Some(&b'\n') => {
                // pula o resto em pedaços, sem guardar
                loop {
                    let mut skip = Vec::new();
                    match std::io::Read::take(&mut reader, 64 * 1024).read_until(b'\n', &mut skip) {
                        Ok(0) | Err(_) => break,
                        Ok(_) if skip.last() == Some(&b'\n') => break,
                        Ok(_) => {}
                    }
                }
                continue;
            }
            Ok(_) => {}
        }
        let Ok(v) = serde_json::from_slice::<Value>(&buf) else { continue };
        if let Some(m) = keep(v) {
            out.push_back(m);
            if out.len() > MAX_MESSAGES {
                out.pop_front();
            }
        }
    }
    out.into()
}

fn keep(v: Value) -> Option<Value> {
    std::iter::once(v)
        .filter(|v| matches!(v["type"].as_str(), Some("user" | "assistant")))
        .filter(|v| !v["isSidechain"].as_bool().unwrap_or(false))
        .filter(|v| !v["isMeta"].as_bool().unwrap_or(false))
        .map(|v| {
            serde_json::json!({
                "type": v["type"],
                "message": v["message"],
                "uuid": v["uuid"],
                "timestamp": v["timestamp"],
                "parent_tool_use_id": Value::Null,
            })
        })
        .next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_main_conversation_and_drops_sidechains_and_meta() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("s.jsonl");
        std::fs::write(
            &f,
            [
                r#"{"type":"summary","summary":"x"}"#,
                r#"{"type":"user","message":{"role":"user","content":"oi"},"uuid":"1"}"#,
                r#"{"type":"user","isMeta":true,"message":{"role":"user","content":"caveat"}}"#,
                r#"{"type":"assistant","isSidechain":true,"message":{"role":"assistant","content":[]}}"#,
                r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"olá"}]},"uuid":"2"}"#,
                "linha quebrada",
            ]
            .join("\n"),
        )
        .unwrap();
        let msgs = load(&f);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[1]["message"]["content"][0]["text"], "olá");
        assert!(load(&d.path().join("nao-existe.jsonl")).is_empty());
    }
}

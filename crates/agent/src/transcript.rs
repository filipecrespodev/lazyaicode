//! Histórico de uma thread lido do armazenamento nativo do Claude Code
//! (`<claudeDir>/projects/<cwd codificado>/<session>.jsonl`). O app não grava
//! transcrição própria: a sessão é do CLI e é a fonte de verdade.

use serde_json::Value;
use std::path::Path;

/// Mensagens `user`/`assistant` da conversa principal, no mesmo formato que
/// chega pelo stream — a UI usa um único renderizador para as duas origens.
pub fn load(file: &Path) -> Vec<Value> {
    let Ok(text) = std::fs::read_to_string(file) else { return Vec::new() };
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
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
        .collect()
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

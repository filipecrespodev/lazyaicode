//! Descoberta das sessões que o Claude Code já gravou no diretório ativo
//! (`<claudeDir>/projects/<cwd codificado>/<session>.jsonl`), para que as
//! conversas feitas no terminal apareçam como threads e possam continuar.
//!
//! Os arquivos chegam a dezenas de MB; cada um é lido uma vez e guardado em
//! cache por (tamanho, mtime), e só as linhas que interessam são parseadas.

use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, PartialEq)]
pub struct SessionInfo {
    pub session_id: String,
    pub cwd: String,
    pub title: String,
    pub updated_at: i64,
    pub created_at: i64,
}

#[derive(Default)]
pub struct SessionCache {
    entries: HashMap<PathBuf, (u64, i64, Option<SessionInfo>)>,
}

fn first_user_text(v: &Value) -> Option<String> {
    let c = &v["message"]["content"];
    let raw = match c {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return None,
    };
    // slash command: mostra o comando em vez das tags
    if let Some(start) = raw.find("<command-name>") {
        let rest = &raw[start + 14..];
        if let Some(end) = rest.find("</command-name>") {
            let args = raw
                .find("<command-args>")
                .and_then(|s| raw[s + 14..].find("</command-args>").map(|e| raw[s + 14..s + 14 + e].trim().to_string()))
                .unwrap_or_default();
            return Some(format!("{} {}", rest[..end].trim(), args).trim().to_string());
        }
    }
    let mut text = raw;
    for tag in ["system-reminder", "local-command-caveat", "local-command-stdout", "command-message"] {
        let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
        while let (Some(s), Some(e)) = (text.find(&open), text.find(&close)) {
            if e < s {
                break;
            }
            text.replace_range(s..e + close.len(), "");
        }
    }
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?.to_string();
    Some(line.chars().take(80).collect())
}

/// Lê um `.jsonl` de sessão. `None` para arquivos sem nenhuma mensagem do
/// usuário na conversa principal (sessões vazias ou só de subagentes).
pub fn read_session(file: &Path) -> Option<SessionInfo> {
    let meta = std::fs::metadata(file).ok()?;
    let mtime = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_millis() as i64;
    let session_id = file.file_stem()?.to_string_lossy().to_string();
    let reader = BufReader::with_capacity(256 * 1024, std::fs::File::open(file).ok()?);
    let (mut cwd, mut first, mut ai_title, mut custom_title, mut created) = (None, None, None, None, None);
    for line in reader.split(b'\n').map_while(Result::ok) {
        // filtro barato antes do parse: só tipos que interessam
        let s = String::from_utf8_lossy(&line);
        let wants_title = s.contains("\"ai-title\"") || s.contains("\"custom-title\"");
        let wants_user = first.is_none() && s.contains("\"type\":\"user\"");
        if !wants_title && !wants_user {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&s) else { continue };
        match v["type"].as_str() {
            Some("ai-title") => ai_title = v["aiTitle"].as_str().map(str::to_string),
            Some("custom-title") => custom_title = v["customTitle"].as_str().map(str::to_string),
            Some("user") if first.is_none() => {
                if v["isSidechain"].as_bool().unwrap_or(false) || v["isMeta"].as_bool().unwrap_or(false) {
                    continue;
                }
                if v["message"]["content"].as_array().is_some_and(|c| c.iter().any(|b| b["type"] == "tool_result")) {
                    continue;
                }
                if let Some(t) = first_user_text(&v) {
                    cwd = v["cwd"].as_str().map(str::to_string);
                    created = v["timestamp"]
                        .as_str()
                        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                        .map(|d| d.timestamp_millis());
                    first = Some(t);
                }
            }
            _ => {}
        }
    }
    let first = first?;
    Some(SessionInfo {
        session_id,
        cwd: cwd?,
        title: custom_title.or(ai_title).unwrap_or(first),
        updated_at: mtime,
        created_at: created.unwrap_or(mtime),
    })
}

impl SessionCache {
    /// Todas as sessões do diretório, relendo só os arquivos que mudaram.
    pub fn scan(&mut self, claude_dir: &Path) -> Vec<SessionInfo> {
        let mut seen = Vec::new();
        let Ok(projects) = std::fs::read_dir(claude_dir.join("projects")) else { return vec![] };
        for p in projects.flatten() {
            let Ok(files) = std::fs::read_dir(p.path()) else { continue };
            for f in files.flatten() {
                let path = f.path();
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let Ok(meta) = f.metadata() else { continue };
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                let fresh = matches!(self.entries.get(&path), Some((len, mt, _)) if *len == meta.len() && *mt == mtime);
                if !fresh {
                    let info = read_session(&path);
                    self.entries.insert(path.clone(), (meta.len(), mtime, info));
                }
                seen.push(path);
            }
        }
        seen.iter()
            .filter_map(|p| self.entries.get(p).and_then(|e| e.2.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, lines: &[&str]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, lines.join("\n")).unwrap();
        p
    }

    #[test]
    fn uses_the_latest_ai_title_and_the_cwd_of_the_first_real_user_message() {
        let d = tempfile::tempdir().unwrap();
        let f = write(d.path(), "abc.jsonl", &[
            r#"{"type":"permission-mode","sessionId":"abc"}"#,
            r#"{"type":"user","isMeta":true,"cwd":"/errado","message":{"role":"user","content":"meta"}}"#,
            r#"{"type":"user","cwd":"/proj","timestamp":"2026-10-01T10:00:00Z","message":{"role":"user","content":"<system-reminder>x</system-reminder>Corrigir o login\nmais"}}"#,
            r#"{"type":"ai-title","aiTitle":"Primeiro título"}"#,
            r#"{"type":"user","cwd":"/proj","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t","content":"ok"}]}}"#,
            r#"{"type":"ai-title","aiTitle":"Corrigir login"}"#,
        ]);
        let s = read_session(&f).unwrap();
        assert_eq!(s.session_id, "abc");
        assert_eq!(s.cwd, "/proj");
        assert_eq!(s.title, "Corrigir login");
        assert_eq!(s.created_at, 1_790_848_800_000);
    }

    #[test]
    fn falls_back_to_the_first_message_and_shows_slash_commands_readably() {
        let d = tempfile::tempdir().unwrap();
        let f = write(d.path(), "s.jsonl", &[
            r#"{"type":"user","cwd":"/p","message":{"role":"user","content":"<command-message>review</command-message>\n<command-name>/review</command-name>\n<command-args>123</command-args>"}}"#,
        ]);
        assert_eq!(read_session(&f).unwrap().title, "/review 123");
        let g = write(d.path(), "t.jsonl", &[r#"{"type":"user","cwd":"/p","message":{"role":"user","content":[{"type":"text","text":"Olá mundo"}]}}"#]);
        assert_eq!(read_session(&g).unwrap().title, "Olá mundo");
    }

    #[test]
    fn custom_title_wins_and_sessions_without_user_messages_are_skipped() {
        let d = tempfile::tempdir().unwrap();
        let f = write(d.path(), "c.jsonl", &[
            r#"{"type":"user","cwd":"/p","message":{"role":"user","content":"x"}}"#,
            r#"{"type":"custom-title","customTitle":"Meu nome"}"#,
            r#"{"type":"ai-title","aiTitle":"Do modelo"}"#,
        ]);
        assert_eq!(read_session(&f).unwrap().title, "Meu nome");
        let e = write(d.path(), "e.jsonl", &[r#"{"type":"summary","summary":"x"}"#]);
        assert!(read_session(&e).is_none());
    }

    #[test]
    fn scan_rereads_only_files_that_changed() {
        let d = tempfile::tempdir().unwrap();
        let proj = d.path().join("projects/-p");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::create_dir_all(proj.join("memory")).unwrap();
        write(&proj, "a.jsonl", &[r#"{"type":"user","cwd":"/p","message":{"role":"user","content":"um"}}"#]);
        let mut cache = SessionCache::default();
        assert_eq!(cache.scan(d.path())[0].title, "um");
        std::thread::sleep(std::time::Duration::from_millis(20));
        write(&proj, "a.jsonl", &[
            r#"{"type":"user","cwd":"/p","message":{"role":"user","content":"um"}}"#,
            r#"{"type":"ai-title","aiTitle":"novo"}"#,
        ]);
        assert_eq!(cache.scan(d.path())[0].title, "novo");
    }

    /// Mede a varredura no diretório Claude ativo da máquina.
    /// `cargo test --release -- --ignored scan_real --nocapture`
    #[test]
    #[ignore]
    fn scan_real_claude_dir() {
        let dir = crate::claude::resolve_active(None);
        let mut cache = SessionCache::default();
        let t = std::time::Instant::now();
        let all = cache.scan(&dir);
        let first = t.elapsed();
        let t = std::time::Instant::now();
        cache.scan(&dir);
        println!("{}: {} sessões, 1ª varredura {:?}, com cache {:?}", dir.display(), all.len(), first, t.elapsed());
        for s in all.iter().take(5) {
            println!("  {} | {} | {}", s.title, s.cwd, s.session_id);
        }
    }
}

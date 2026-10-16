//! Tudo que liga o app ao Claude Code instalado na máquina: qual diretório de
//! configuração (`~/.claude*`) está ativo, onde está o binário `claude` e qual
//! ambiente ele recebe. Não existe chave própria do app — o CLI lê as
//! credenciais (`.credentials.json`, keychain ou `ANTHROPIC_API_KEY`) do
//! diretório que `CLAUDE_CONFIG_DIR` aponta, exatamente como no terminal.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Arquivos/pastas que indicam que uma pasta `.claude*` é um diretório de
/// configuração do Claude Code e não uma pasta qualquer com o mesmo prefixo.
const MARKERS: &[&str] = &[
    "projects",
    "settings.json",
    ".credentials.json",
    "agents",
    "skills",
    "plugins",
    "history.jsonl",
];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ClaudeDirInfo {
    pub path: String,
    pub label: String,
    pub is_default: bool,
    pub has_credentials: bool,
}

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

pub fn default_claude_dir() -> PathBuf {
    home().join(".claude")
}

fn is_claude_dir(p: &Path) -> bool {
    p.is_dir() && MARKERS.iter().any(|m| p.join(m).exists())
}

/// Procura diretórios de configuração em `~/.claude*`, descendo um nível em
/// pastas que só agrupam perfis (`~/.claude-profiles/<nome>`).
pub fn discover_dirs(home: &Path) -> Vec<ClaudeDirInfo> {
    let default = home.join(".claude");
    let mut found: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(home) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.starts_with(".claude") {
                continue;
            }
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            if is_claude_dir(&p) {
                found.push(p);
            } else if let Ok(children) = std::fs::read_dir(&p) {
                for c in children.flatten() {
                    if is_claude_dir(&c.path()) {
                        found.push(c.path());
                    }
                }
            }
        }
    }
    found.sort();
    found
        .into_iter()
        .map(|p| {
            // rótulo com `/` em todo sistema (no Windows `display()` usaria `\`)
            let label = p
                .strip_prefix(home)
                .map(|r| {
                    let parts: Vec<String> = r.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
                    format!("~/{}", parts.join("/"))
                })
                .unwrap_or_else(|_| p.display().to_string());
            ClaudeDirInfo {
                has_credentials: p.join(".credentials.json").exists(),
                is_default: p == default,
                path: p.display().to_string(),
                label,
            }
        })
        .collect()
}

/// Ordem: escolha salva no app → `CLAUDE_CONFIG_DIR` herdado → `~/.claude`.
pub fn resolve_active(saved: Option<&str>) -> PathBuf {
    if let Some(s) = saved.filter(|s| !s.is_empty()) {
        return PathBuf::from(s);
    }
    if let Ok(env) = std::env::var("CLAUDE_CONFIG_DIR") {
        if !env.is_empty() {
            return PathBuf::from(env);
        }
    }
    default_claude_dir()
}

/// Como o Claude Code nomeia a pasta de sessões de um projeto:
/// todo caractere fora de `[A-Za-z0-9]` vira `-`.
pub fn encode_project_path(path: &str) -> String {
    path.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

pub fn session_file(claude_dir: &Path, cwd: &str, session_id: &str) -> PathBuf {
    let direct = claude_dir
        .join("projects")
        .join(encode_project_path(cwd))
        .join(format!("{session_id}.jsonl"));
    if direct.exists() {
        return direct;
    }
    // caminhos longos são abreviados pelo CLI: procura pelo id da sessão
    std::fs::read_dir(claude_dir.join("projects"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|d| d.path().join(format!("{session_id}.jsonl")))
        .find(|p| p.exists())
        .unwrap_or(direct)
}

/// Apps gráficos costumam herdar um PATH mínimo, sem `~/.local/bin`; por isso
/// os locais de instalação conhecidos são tentados além do PATH.
pub fn find_claude_binary() -> Option<PathBuf> {
    // TREVOCODE_CLAUDE_BIN: nome antigo, mantido por compatibilidade
    if let Ok(p) = std::env::var("LAZYAICODE_CLAUDE_BIN").or_else(|_| std::env::var("TREVOCODE_CLAUDE_BIN")) {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            for name in crate::platform::claude_names() {
                let c = dir.join(name);
                if c.is_file() {
                    return Some(c);
                }
            }
        }
    }
    crate::platform::claude_install_locations(&home()).into_iter().find(|p| p.is_file())
}

/// Ambiente de qualquer processo `claude` iniciado pelo app. Para o diretório
/// padrão a variável é removida, porque é assim que o CLI decide usar
/// `~/.claude.json` em vez de `<dir>/.claude.json`.
pub fn apply_env(cmd: &mut Command, claude_dir: &Path) {
    if claude_dir == default_claude_dir() {
        cmd.env_remove("CLAUDE_CONFIG_DIR");
    } else {
        cmd.env("CLAUDE_CONFIG_DIR", claude_dir);
    }
    cmd.env_remove("CLAUDECODE");
    cmd.env("CLAUDE_CODE_ENTRYPOINT", "sdk-rust");
}

pub fn auth_status(claude_dir: &Path) -> serde_json::Value {
    let Some(bin) = find_claude_binary() else {
        return serde_json::json!({ "loggedIn": false, "error": "claude_not_found" });
    };
    let mut cmd = crate::platform::command(bin);
    cmd.args(["auth", "status", "--json"]);
    apply_env(&mut cmd, claude_dir);
    match cmd.output() {
        Ok(out) => serde_json::from_slice(&out.stdout).unwrap_or_else(|_| {
            serde_json::json!({
                "loggedIn": std::env::var("ANTHROPIC_API_KEY").is_ok(),
                "raw": String::from_utf8_lossy(&out.stdout).trim().to_string(),
            })
        }),
        Err(e) => serde_json::json!({ "loggedIn": false, "error": e.to_string() }),
    }
}

/// Uma pergunta avulsa ao `claude -p` com a conta `claude_dir`: sem
/// ferramentas, MCP nem sessão salva, numa pasta temporária (nada do projeto
/// é lido além do que vai em `input`). Usada pela IA de commit do git.
pub async fn complete(claude_dir: &Path, model: &str, system_prompt: &str, input: &str) -> anyhow::Result<String> {
    use anyhow::{anyhow, bail};
    use tokio::io::AsyncWriteExt;

    let bin = find_claude_binary().ok_or_else(|| anyhow!("Claude Code (comando `claude`) não encontrado"))?;
    let mut std_cmd = crate::platform::command(bin);
    std_cmd
        .args(["-p", "--output-format", "json", "--no-session-persistence", "--strict-mcp-config", "--tools", ""])
        .args(["--system-prompt", system_prompt])
        .current_dir(std::env::temp_dir());
    if !model.is_empty() {
        std_cmd.args(["--model", model]);
    }
    apply_env(&mut std_cmd, claude_dir);
    let mut cmd = tokio::process::Command::from(std_cmd);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|e| anyhow!("Não foi possível iniciar o Claude Code: {e}"))?;
    let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("sem stdin"))?;
    stdin.write_all(input.as_bytes()).await?;
    drop(stdin);
    let out = tokio::time::timeout(std::time::Duration::from_secs(180), child.wait_with_output())
        .await
        .map_err(|_| anyhow!("O Claude Code demorou demais para responder"))??;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|_| {
        let err = String::from_utf8_lossy(&out.stderr);
        anyhow!("Resposta inesperada do Claude Code: {}", err.trim())
    })?;
    let text = v["result"].as_str().unwrap_or_default().to_string();
    if v["is_error"].as_bool().unwrap_or(false) || text.trim().is_empty() {
        bail!("O Claude Code respondeu com erro: {}", if text.is_empty() { v["subtype"].to_string() } else { text });
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_project_path_like_claude_code() {
        assert_eq!(
            encode_project_path("/tmp/a_b/c.d"),
            "-tmp-a-b-c-d"
        );
    }

    #[test]
    fn discovers_profiles_and_ignores_unrelated_dot_claude_folders() {
        let home = tempfile::tempdir().unwrap();
        let h = home.path();
        std::fs::create_dir_all(h.join(".claude/projects")).unwrap();
        std::fs::create_dir_all(h.join(".claude-work")).unwrap();
        std::fs::write(h.join(".claude-work/.credentials.json"), "{}").unwrap();
        std::fs::create_dir_all(h.join(".claude-profiles/pessoal/skills")).unwrap();
        std::fs::create_dir_all(h.join(".claude-cache-sem-marcador")).unwrap();

        let dirs = discover_dirs(h);
        let labels: Vec<_> = dirs.iter().map(|d| d.label.as_str()).collect();
        assert_eq!(
            labels,
            vec!["~/.claude", "~/.claude-profiles/pessoal", "~/.claude-work"]
        );
        assert!(dirs[0].is_default);
        assert!(dirs[2].has_credentials);
        assert!(!dirs[1].has_credentials);
    }

    #[test]
    fn finds_a_session_whose_folder_name_does_not_match_the_encoding() {
        let d = tempfile::tempdir().unwrap();
        let folder = d.path().join("projects/-nome-abreviado-123");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("s1.jsonl"), "").unwrap();
        assert_eq!(session_file(d.path(), "/um/caminho/longo", "s1"), folder.join("s1.jsonl"));
    }

    #[test]
    fn saved_choice_wins_over_environment() {
        assert_eq!(resolve_active(Some("/x/y")), PathBuf::from("/x/y"));
    }
}

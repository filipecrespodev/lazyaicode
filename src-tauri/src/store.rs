//! Estado persistente do app (projetos, threads, preferências) num único JSON
//! em `<data_dir>/lazyaicode/state.json`, sempre gravado de forma atômica.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: i64,
    /// Tem `.git` (recalculado a cada listagem): sem ele não há abas git
    /// nem workspace isolado.
    #[serde(default)]
    pub is_git: bool,
}

/// Uma pasta que agrupa repositórios, com a sua própria conta Claude
/// (diretório de configuração). Aparece como uma aba na barra de título.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: String,
    /// `None` = `~/.claude`.
    pub claude_dir: Option<String>,
    #[serde(default)]
    pub ignored_paths: Vec<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    /// Pasta onde o agente trabalha: um checkout isolado ou o próprio projeto.
    pub cwd: String,
    pub isolated: bool,
    pub branch: Option<String>,
    /// Ref de onde o checkout isolado partiu (ex.: `origin/main`).
    pub base_ref: Option<String>,
    pub session_id: Option<String>,
    pub model: String,
    pub permission_mode: String,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub bookmarked: bool,
    /// Arquivo → impressão digital do diff no momento em que foi marcado
    /// como visto. Um diff diferente desmarca sozinho.
    #[serde(default)]
    pub seen: HashMap<String, String>,
    /// Árvore git capturada no início do último turno (diff "Último turno").
    #[serde(default)]
    pub turn_tree: Option<String>,
    #[serde(default)]
    pub unread: bool,
    /// Diretório Claude dono da sessão: uma sessão só pode ser retomada
    /// com as credenciais e o histórico do diretório onde nasceu.
    #[serde(default)]
    pub claude_dir: Option<String>,
    /// Sessões anteriores desta thread (quando um resume gera id novo), para
    /// a importação não trazê-las de volta como threads separadas.
    #[serde(default)]
    pub past_sessions: Vec<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub add_dirs: Vec<String>,
    /// claude (Claude Code nativo) | codex (OpenAI) | gemini (Google)
    #[serde(default = "claude_provider")]
    pub provider: String,
    /// roda com `--dangerously-skip-permissions` (só Claude Code); vale em
    /// toda abertura do processo, inclusive ao retomar a sessão
    #[serde(default)]
    pub skip_permissions: bool,
}

fn claude_provider() -> String {
    "claude".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub default_model: String,
    pub default_permission_mode: String,
    pub send_with_modifier: bool,
    #[serde(default = "yes")]
    pub notify_on_finish: bool,
    #[serde(default = "yes")]
    pub sound_on_finish: bool,
    #[serde(default = "claude_provider")]
    pub default_provider: String,
    #[serde(default)]
    pub default_skip_permissions: bool,
    /// idioma da interface: en, pt, es, zh
    #[serde(default = "english")]
    pub language: String,
}

fn yes() -> bool {
    true
}

fn english() -> String {
    "en".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_model: "default".into(),
            default_permission_mode: "default".into(),
            send_with_modifier: false,
            notify_on_finish: true,
            sound_on_finish: true,
            default_provider: claude_provider(),
            default_skip_permissions: false,
            language: english(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub workspaces: Vec<Workspace>,
    #[serde(default)]
    pub active_workspace: Option<String>,
    #[serde(default)]
    pub projects: Vec<Project>,
    #[serde(default)]
    pub threads: Vec<Thread>,
}

pub struct Store {
    path: PathBuf,
    pub state: State,
}

pub fn data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("LAZYAICODE_DATA_DIR").or_else(|_| std::env::var("TREVOCODE_DATA_DIR")) {
        return PathBuf::from(p);
    }
    let base = dirs::data_dir().unwrap_or_else(|| crate::claude::home().join(".local/share"));
    let dir = base.join("lazyaicode");
    migrate_from(&base.join("trevocode"), &dir);
    dir
}

/// O app se chamava TrevoCode. Copia só o `state.json`: os checkouts isolados
/// continuam na pasta antiga, que é para onde o git de cada repositório aponta
/// (mover quebraria os worktrees).
pub fn migrate_from(old: &Path, new: &Path) {
    let (src, dst) = (old.join("state.json"), new.join("state.json"));
    if src.is_file() && !dst.exists() {
        let _ = std::fs::create_dir_all(new);
        let _ = std::fs::copy(&src, &dst);
    }
}

/// Grava num arquivo irmão e renomeia por cima: um crash no meio nunca deixa
/// o estado truncado.
pub fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

impl Store {
    pub fn load(dir: &Path) -> Self {
        let _ = std::fs::create_dir_all(dir);
        let path = dir.join("state.json");
        let state = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self { path, state }
    }

    pub fn save(&self) -> Result<()> {
        write_atomic(&self.path, &serde_json::to_vec_pretty(&self.state)?)
    }

    pub fn thread(&self, id: &str) -> Option<&Thread> {
        self.state.threads.iter().find(|t| t.id == id)
    }

    pub fn thread_mut(&mut self, id: &str) -> Option<&mut Thread> {
        self.state.threads.iter_mut().find(|t| t.id == id)
    }

    pub fn workspace(&self, id: &str) -> Option<&Workspace> {
        self.state.workspaces.iter().find(|w| w.id == id)
    }

    pub fn project(&self, id: &str) -> Option<&Project> {
        self.state.projects.iter().find(|p| p.id == id)
    }
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_the_old_state_once_and_never_overwrites_the_new_one() {
        let d = tempfile::tempdir().unwrap();
        let (old, new) = (d.path().join("trevocode"), d.path().join("lazyaicode"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("state.json"), "antigo").unwrap();
        migrate_from(&old, &new);
        assert_eq!(std::fs::read_to_string(new.join("state.json")).unwrap(), "antigo");
        std::fs::write(new.join("state.json"), "novo").unwrap();
        migrate_from(&old, &new);
        assert_eq!(std::fs::read_to_string(new.join("state.json")).unwrap(), "novo");
    }

    #[test]
    fn state_survives_a_reload_and_old_files_without_new_fields_still_load() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("state.json"),
            r#"{"projects":[{"id":"p","name":"n","path":"/x","created_at":1}]}"#,
        )
        .unwrap();
        let mut s = Store::load(dir.path());
        assert_eq!(s.state.projects.len(), 1);
        assert_eq!(s.state.settings.default_model, "default");
        s.state.settings.default_model = "opus".into();
        s.save().unwrap();
        let s2 = Store::load(dir.path());
        assert_eq!(s2.state.settings.default_model, "opus");
    }
}

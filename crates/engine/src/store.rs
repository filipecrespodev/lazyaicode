//! Estado persistente do app (projetos, threads, preferências) num único JSON
//! em `<data_dir>/state.json`, sempre gravado de forma atômica.

use anyhow::Result;
use lazyaicode_doc::State;
use std::path::{Path, PathBuf};

pub struct Store {
    path: PathBuf,
    pub state: State,
}

pub fn data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("LAZYAICODE_DATA_DIR").or_else(|_| std::env::var("TREVOCODE_DATA_DIR")) {
        return PathBuf::from(p);
    }
    let base = dirs::data_dir().unwrap_or_else(|| lazyaicode_agent::claude::home().join(".local/share"));
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
    /// Um `state.json` corrompido não derruba o app: é guardado ao lado
    /// (`state.json.corrupt-<ts>`) e o app abre vazio.
    pub fn load(dir: &Path) -> Self {
        let _ = std::fs::create_dir_all(dir);
        let path = dir.join("state.json");
        let state = match std::fs::read(&path) {
            Ok(b) => serde_json::from_slice(&b).unwrap_or_else(|_| {
                let _ = std::fs::copy(&path, path.with_extension(format!("json.corrupt-{}", now())));
                State::default()
            }),
            Err(_) => State::default(),
        };
        Self { path, state }
    }

    /// Estado em memória, sem arquivo (testes e `--ephemeral`).
    pub fn memory() -> Self {
        Self { path: PathBuf::new(), state: State::default() }
    }

    pub fn save(&self) -> Result<()> {
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }
        write_atomic(&self.path, &serde_json::to_vec_pretty(&self.state)?)
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
        std::fs::write(dir.path().join("state.json"), r#"{"projects":[{"id":"p","name":"n","path":"/x","created_at":1}]}"#).unwrap();
        let mut s = Store::load(dir.path());
        assert_eq!(s.state.projects.len(), 1);
        assert_eq!(s.state.settings.default_model, "default");
        s.state.settings.default_model = "opus".into();
        s.save().unwrap();
        let s2 = Store::load(dir.path());
        assert_eq!(s2.state.settings.default_model, "opus");
    }

    #[test]
    fn a_corrupt_state_is_kept_aside_and_the_app_opens_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("state.json"), "{ não é json").unwrap();
        let s = Store::load(dir.path());
        assert!(s.state.threads.is_empty());
        let kept =
            std::fs::read_dir(dir.path()).unwrap().filter_map(Result::ok).any(|e| e.file_name().to_string_lossy().contains("corrupt"));
        assert!(kept);
    }
}

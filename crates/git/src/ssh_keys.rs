//! Which SSH key a repo authenticates with: the one picked for the repo, else
//! for the workspace it lives in, else the default from Settings. The choices
//! live in the frontend's `layout.json` store (key `sshKeys`), read from here
//! on every connection so changes apply without restarting.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::passphrase;

const STORE_KEY: &str = "sshKeys";
/// What OpenSSH itself tries when nothing is configured.
const DEFAULT_KEY_NAMES: [&str; 2] = ["id_ed25519", "id_rsa"];

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SshKeySettings {
    /// Private key path; `None` = OpenSSH's default identities.
    pub default_key: Option<PathBuf>,
    /// Workspace or repo directory → private key path.
    pub overrides: BTreeMap<PathBuf, PathBuf>,
}

impl SshKeySettings {
    /// The most specific override containing `repo_dir` (a repo's own beats
    /// its workspace's), else the default.
    pub fn resolve(&self, repo_dir: &Path) -> Option<PathBuf> {
        self.overrides
            .iter()
            .filter(|(dir, _)| repo_dir.starts_with(dir))
            .max_by_key(|(dir, _)| dir.components().count())
            .map(|(_, key)| key.clone())
            .or_else(|| self.default_key.clone())
    }
}

fn load_settings() -> SshKeySettings {
    crate::settings::get(STORE_KEY).and_then(|value| serde_json::from_value(value).ok()).unwrap_or_default()
}

/// The key the user picked for `repo_dir` (or its workspace, or the default),
/// for tools that run `ssh` themselves (git-lfs).
pub fn configured_key(repo_dir: &Path) -> Option<PathBuf> {
    load_settings().resolve(repo_dir)
}

/// (private, public) pairs to offer for a repo at `repo_dir`, in order.
pub fn keys_for(repo_dir: Option<&Path>) -> Vec<(PathBuf, Option<PathBuf>)> {
    let configured = repo_dir.and_then(|dir| load_settings().resolve(dir));
    candidate_keys(configured, ssh_dir().as_deref())
}

/// The configured key alone, or with none configured, the OpenSSH default
/// identities that exist in `ssh_dir`. Never every key in ~/.ssh.
fn candidate_keys(configured: Option<PathBuf>, ssh_dir: Option<&Path>) -> Vec<(PathBuf, Option<PathBuf>)> {
    let with_pub = |private: PathBuf| {
        let public = private.with_extension("pub");
        let public = public.is_file().then_some(public);
        (private, public)
    };
    match configured {
        Some(key) => vec![with_pub(key)],
        None => {
            ssh_dir.into_iter().flat_map(|dir| DEFAULT_KEY_NAMES.map(|name| dir.join(name))).filter(|p| p.is_file()).map(with_pub).collect()
        }
    }
}

fn ssh_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(|home| PathBuf::from(home).join(".ssh"))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyInfo {
    pub path: String,
    pub name: String,
    pub encrypted: bool,
}

/// Private keys in `dir`: files with a `.pub` sibling.
fn list_keys_in(dir: &Path) -> Vec<SshKeyInfo> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut keys: Vec<SshKeyInfo> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_none() && p.with_extension("pub").is_file())
        .map(|p| SshKeyInfo {
            name: p.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            encrypted: passphrase::is_encrypted(&p),
            path: p.to_string_lossy().into_owned(),
        })
        .collect();
    keys.sort_by(|a, b| a.name.cmp(&b.name));
    keys
}

/// Keys found in ~/.ssh, for the pickers in Settings / workspaces / repos.
pub fn list_ssh_keys() -> Vec<SshKeyInfo> {
    ssh_dir().map(|dir| list_keys_in(&dir)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(default: Option<&str>, overrides: &[(&str, &str)]) -> SshKeySettings {
        SshKeySettings {
            default_key: default.map(PathBuf::from),
            overrides: overrides.iter().map(|(d, k)| (PathBuf::from(d), PathBuf::from(k))).collect(),
        }
    }

    #[test]
    fn resolve_prefers_repo_then_workspace_then_default() {
        let s = settings(Some("/k/default"), &[("/ws", "/k/workspace"), ("/ws/app", "/k/repo")]);
        assert_eq!(s.resolve(Path::new("/ws/app")), Some("/k/repo".into()));
        assert_eq!(s.resolve(Path::new("/ws/app/")), Some("/k/repo".into()));
        assert_eq!(s.resolve(Path::new("/ws/lib")), Some("/k/workspace".into()));
        assert_eq!(s.resolve(Path::new("/elsewhere")), Some("/k/default".into()));
    }

    #[test]
    fn resolve_matches_whole_path_components_only() {
        let s = settings(None, &[("/ws/app", "/k/repo")]);
        assert_eq!(s.resolve(Path::new("/ws/app-old")), None);
    }

    #[test]
    fn settings_deserialize_from_partial_store_value() {
        let s: SshKeySettings = serde_json::from_value(serde_json::json!({ "defaultKey": "/k/a" })).unwrap();
        assert_eq!(s, settings(Some("/k/a"), &[]));
        let s: SshKeySettings = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(s, SshKeySettings::default());
    }

    fn touch(path: &Path) {
        std::fs::write(path, "x").unwrap();
    }

    #[test]
    fn configured_key_is_the_only_candidate() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["id_ed25519", "id_ed25519.pub", "work", "work.pub"] {
            touch(&dir.path().join(name));
        }
        let keys = candidate_keys(Some(dir.path().join("work")), Some(dir.path()));
        assert_eq!(keys, vec![(dir.path().join("work"), Some(dir.path().join("work.pub")))]);
    }

    #[test]
    fn without_config_only_openssh_defaults_are_candidates() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["id_rsa", "id_rsa.pub", "id_ed25519", "other", "other.pub"] {
            touch(&dir.path().join(name));
        }
        let keys = candidate_keys(None, Some(dir.path()));
        assert_eq!(keys, vec![(dir.path().join("id_ed25519"), None), (dir.path().join("id_rsa"), Some(dir.path().join("id_rsa.pub"))),]);
    }

    #[test]
    fn lists_only_files_with_a_public_key() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["b", "b.pub", "a", "a.pub", "known_hosts", "config", "lonely"] {
            touch(&dir.path().join(name));
        }
        let names: Vec<_> = list_keys_in(dir.path()).into_iter().map(|k| k.name).collect();
        assert_eq!(names, ["a", "b"]);
    }
}

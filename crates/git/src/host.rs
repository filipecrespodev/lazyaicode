//! Filesystem scans that don't belong to an open repo: the repos of a
//! workspace folder, the folders of the WSL picker, a one-shot fetch by path.
//! Run by the app for local paths and by `lazyaicode-agent` inside WSL.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::GitCommandError;
use crate::git::remote::{self, AuthMode};
use crate::git::repo;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceRepoEntry {
    pub name: String,
    pub path: String,
    /// Tem `.git`: sem ele a pasta entra no workspace só para o chat e o terminal.
    #[serde(default)]
    pub is_repo: bool,
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.display().to_string())
}

/// Folders in the immediate children of `path` (dotfolders hidden), git or
/// not — a plain filesystem scan (no git2). The folder itself comes first when
/// it is a repo (the user picked the repo, not the folder that holds it).
pub fn scan(path: &Path) -> Result<Vec<WorkspaceRepoEntry>, GitCommandError> {
    let mut out: Vec<WorkspaceRepoEntry> = std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .map(|p| WorkspaceRepoEntry { name: name_of(&p), is_repo: p.join(".git").exists(), path: p.display().to_string() })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    if path.join(".git").exists() {
        out.insert(0, WorkspaceRepoEntry { name: name_of(path), path: path.display().to_string(), is_repo: true });
    }
    Ok(out)
}

/// One-shot fetch of every remote of the repo at `path`. Blocking.
pub fn fetch_at_path(path: &str, mode: AuthMode) -> Result<(), GitCommandError> {
    let repo = repo::open(path)?;
    remote::fetch_remotes(&repo, None, mode, |_, _, _| {})?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_repo: bool,
}

/// Subfolders of `path` for the WSL folder picker, dotfolders hidden.
pub fn list_dirs(path: &str) -> Result<Vec<DirEntry>, GitCommandError> {
    let mut dirs: Vec<DirEntry> = std::fs::read_dir(path)
        .map_err(|e| GitCommandError::Internal(e.to_string()))?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (!name.starts_with('.')).then(|| DirEntry {
                is_repo: e.path().join(".git").exists(),
                path: e.path().to_string_lossy().into_owned(),
                name,
            })
        })
        .collect();
    dirs.sort_by_key(|d| d.name.to_lowercase());
    Ok(dirs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_folders_without_dotfolders_and_marks_repos() {
        let d = tempfile::tempdir().unwrap();
        for f in ["b", "A/.git", ".hidden"] {
            std::fs::create_dir_all(d.path().join(f)).unwrap();
        }
        let dirs = list_dirs(d.path().to_str().unwrap()).unwrap();
        let got: Vec<_> = dirs.iter().map(|d| (d.name.as_str(), d.is_repo)).collect();
        assert_eq!(got, vec![("A", true), ("b", false)]);
    }
}

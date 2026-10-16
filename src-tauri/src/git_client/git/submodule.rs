//! Submodules (`git submodule status/init/update`) through libgit2.
//! Nested submodules aren't recursed into: open the submodule in its own tab.

use serde::{Deserialize, Serialize};

use super::remote::{self, AuthMode};
use crate::git_client::activity;
use crate::git_client::error::GitCommandError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubmoduleState {
    /// Not cloned/checked out yet: needs init + update.
    Uninitialized,
    /// Checked out at a different commit than the superproject records.
    OutOfDate,
    /// Right commit, but with uncommitted changes inside.
    Dirty,
    UpToDate,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleInfo {
    pub name: String,
    /// Relative to the superproject.
    pub path: String,
    pub abs_path: String,
    pub url: Option<String>,
    /// Commit the superproject records (index, else HEAD).
    pub expected_sha: Option<String>,
    /// Commit checked out inside it, if any.
    pub current_sha: Option<String>,
    /// Registered in `.git/config` (`git submodule init` done).
    pub initialized: bool,
    pub state: SubmoduleState,
}

pub fn state_of(status: git2::SubmoduleStatus) -> SubmoduleState {
    use git2::SubmoduleStatus as S;
    if status.contains(S::WD_UNINITIALIZED) {
        SubmoduleState::Uninitialized
    } else if status.contains(S::WD_MODIFIED) {
        SubmoduleState::OutOfDate
    } else if status.intersects(S::WD_INDEX_MODIFIED | S::WD_WD_MODIFIED | S::WD_UNTRACKED) {
        SubmoduleState::Dirty
    } else {
        SubmoduleState::UpToDate
    }
}

pub fn list(repo: &git2::Repository) -> Result<Vec<SubmoduleInfo>, GitCommandError> {
    let workdir = repo.workdir().unwrap_or(repo.path());
    // `SubmoduleStatus::IN_CONFIG` is about `.gitmodules`; init writes `.git/config`.
    let config = repo.config()?.snapshot()?;
    let mut out = Vec::new();
    for sm in repo.submodules()? {
        let name = sm.name().unwrap_or_default().to_string();
        let initialized = config.get_str(&format!("submodule.{name}.url")).is_ok();
        // Untracked files inside aren't scanned: that walk is the slow part
        // on big submodules and doesn't change what the user can do here.
        let status = repo.submodule_status(&name, git2::SubmoduleIgnore::Untracked)?;
        out.push(SubmoduleInfo {
            path: sm.path().display().to_string(),
            abs_path: workdir.join(sm.path()).display().to_string(),
            url: sm.url().ok().flatten().map(str::to_string),
            expected_sha: sm.index_id().or(sm.head_id()).map(|o| o.to_string()),
            current_sha: sm.workdir_id().map(|o| o.to_string()),
            initialized,
            state: state_of(status),
            name,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// `git submodule update --init [<path>...]`: clones what's missing and
/// checks out the recorded commit. Empty `paths` means every submodule.
/// Credentials as for a fetch the user started (may prompt).
pub fn update(repo: &git2::Repository, paths: &[String]) -> Result<(), GitCommandError> {
    let workdir = repo.workdir().unwrap_or(repo.path()).to_path_buf();
    for mut sm in repo.submodules()? {
        let path = sm.path().display().to_string();
        if !paths.is_empty() && !paths.contains(&path) {
            continue;
        }
        let url = sm.url().ok().flatten().unwrap_or("?").to_string();
        activity::info(
            Some(&workdir),
            format!("$ git submodule update --init {path}  → {url}"),
        );
        sm.init(false)?;
        let (fetch, locked) = remote::fetch_options(repo, AuthMode::Interactive);
        let mut opts = git2::SubmoduleUpdateOptions::new();
        opts.fetch(fetch).allow_fetch(true);
        sm.update(true, Some(&mut opts)).map_err(|e| {
            activity::error(Some(&workdir), format!("submodule {path}: {}", e.message()));
            remote::auth_error(&locked, e)
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn commit_all(repo: &git2::Repository, msg: &str) -> git2::Oid {
        let mut index = repo.index().unwrap();
        index.add_all(["*"], git2::IndexAddOption::DEFAULT, None).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<_> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents).unwrap()
    }

    /// A library repo, and a superproject that has it at `libs/lib`.
    fn superproject() -> (tempfile::TempDir, tempfile::TempDir) {
        let lib_dir = tempfile::tempdir().unwrap();
        let lib = git2::Repository::init(lib_dir.path()).unwrap();
        std::fs::write(lib_dir.path().join("lib.txt"), "v1").unwrap();
        commit_all(&lib, "lib v1");

        let sup_dir = tempfile::tempdir().unwrap();
        let sup = git2::Repository::init(sup_dir.path()).unwrap();
        std::fs::write(sup_dir.path().join("main.txt"), "main").unwrap();
        commit_all(&sup, "main");
        let mut sm = sup
            .submodule(lib_dir.path().to_str().unwrap(), Path::new("libs/lib"), true)
            .unwrap();
        sm.clone(None).unwrap();
        sm.add_finalize().unwrap();
        let mut index = sup.index().unwrap();
        let tree = sup.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let head = sup.head().unwrap().peel_to_commit().unwrap();
        sup.commit(Some("HEAD"), &sig, &sig, "add lib", &tree, &[&head]).unwrap();
        (lib_dir, sup_dir)
    }

    #[test]
    fn lists_inits_and_updates_submodules_of_a_fresh_clone() {
        let (_lib, sup) = superproject();
        let clone_dir = tempfile::tempdir().unwrap();
        let clone = git2::Repository::clone(sup.path().to_str().unwrap(), clone_dir.path()).unwrap();

        let before = list(&clone).unwrap();
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].path, "libs/lib");
        assert_eq!(before[0].state, SubmoduleState::Uninitialized);
        assert!(!before[0].initialized);
        assert!(before[0].current_sha.is_none());

        update(&clone, &[]).unwrap();
        assert!(clone_dir.path().join("libs/lib/lib.txt").exists());
        let after = list(&clone).unwrap();
        assert_eq!(after[0].state, SubmoduleState::UpToDate);
        assert!(after[0].initialized);
        assert_eq!(after[0].current_sha, after[0].expected_sha);
        assert!(Path::new(&after[0].abs_path).ends_with("libs/lib"));
    }

    #[test]
    fn reports_and_fixes_a_submodule_on_another_commit() {
        let (_lib, sup) = superproject();
        let sup_repo = git2::Repository::open(sup.path()).unwrap();
        // move the submodule's checkout ahead of what the superproject records
        let inner = git2::Repository::open(sup.path().join("libs/lib")).unwrap();
        std::fs::write(sup.path().join("libs/lib/lib.txt"), "v2").unwrap();
        commit_all(&inner, "lib v2");
        assert_eq!(list(&sup_repo).unwrap()[0].state, SubmoduleState::OutOfDate);

        update(&sup_repo, &["libs/lib".to_string()]).unwrap();
        assert_eq!(list(&sup_repo).unwrap()[0].state, SubmoduleState::UpToDate);
    }

    #[test]
    fn updating_keeps_uncommitted_edits_inside_a_submodule() {
        let (_lib, sup) = superproject();
        let sup_repo = git2::Repository::open(sup.path()).unwrap();
        let file = sup.path().join("libs/lib/lib.txt");
        std::fs::write(&file, "local edit").unwrap();
        assert_eq!(list(&sup_repo).unwrap()[0].state, SubmoduleState::Dirty);

        update(&sup_repo, &[]).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "local edit");
    }

    #[test]
    fn no_submodules_is_an_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        assert!(list(&repo).unwrap().is_empty());
        update(&repo, &[]).unwrap();
    }
}

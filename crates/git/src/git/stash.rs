use serde::{Deserialize, Serialize};

use crate::error::GitCommandError;

use super::rebase::{collect_conflicted_files, ConflictedFile};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashInfo {
    pub index: usize,
    pub message: String,
    pub sha: String,
    pub base_sha: String,
    /// When the stash was made (its commit time, unix seconds).
    pub date: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashPopResult {
    pub conflicts: Vec<ConflictedFile>,
}

pub fn save(repo: &mut git2::Repository, message: Option<&str>, include_untracked: bool) -> Result<StashInfo, GitCommandError> {
    let sig = repo.signature()?;
    let flags = if include_untracked { git2::StashFlags::INCLUDE_UNTRACKED } else { git2::StashFlags::DEFAULT };
    let oid = repo.stash_save2(&sig, message, Some(flags))?;
    Ok(StashInfo {
        index: 0,
        message: message.unwrap_or("WIP").to_string(),
        sha: oid.to_string(),
        base_sha: stash_base_sha(repo, oid),
        date: stash_date(repo, oid),
    })
}

fn stash_date(repo: &git2::Repository, stash_oid: git2::Oid) -> i64 {
    repo.find_commit(stash_oid).map(|c| c.time().seconds()).unwrap_or_default()
}

fn stash_base_sha(repo: &git2::Repository, stash_oid: git2::Oid) -> String {
    repo.find_commit(stash_oid).ok().and_then(|c| c.parent(0).ok()).map(|p| p.id().to_string()).unwrap_or_default()
}

pub fn list(repo: &mut git2::Repository) -> Result<Vec<StashInfo>, GitCommandError> {
    let mut raw = Vec::new();
    repo.stash_foreach(|index, message, oid| {
        raw.push((index, message.to_string(), *oid));
        true
    })?;

    Ok(raw
        .into_iter()
        .map(|(index, message, oid)| StashInfo {
            index,
            message,
            sha: oid.to_string(),
            base_sha: stash_base_sha(repo, oid),
            date: stash_date(repo, oid),
        })
        .collect())
}

pub fn pop(repo: &mut git2::Repository, index: usize) -> Result<StashPopResult, GitCommandError> {
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.allow_conflicts(true);
    let mut opts = git2::StashApplyOptions::new();
    opts.checkout_options(checkout);
    repo.stash_pop(index, Some(&mut opts))?;

    let conflicts = if repo.index()?.has_conflicts() { collect_conflicted_files(repo)? } else { Vec::new() };
    Ok(StashPopResult { conflicts })
}

pub fn apply(repo: &mut git2::Repository, index: usize) -> Result<StashPopResult, GitCommandError> {
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.allow_conflicts(true);
    let mut opts = git2::StashApplyOptions::new();
    opts.checkout_options(checkout);
    repo.stash_apply(index, Some(&mut opts))?;

    let conflicts = if repo.index()?.has_conflicts() { collect_conflicted_files(repo)? } else { Vec::new() };
    Ok(StashPopResult { conflicts })
}

pub fn drop(repo: &mut git2::Repository, index: usize) -> Result<(), GitCommandError> {
    repo.stash_drop(index)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn init_repo_with_commit(dir: &std::path::Path) -> git2::Repository {
        let repo = git2::Repository::init(dir).unwrap();
        std::fs::write(dir.join("a.txt"), "hello\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("a.txt")).unwrap();
        let tree_id = index.write_tree().unwrap();
        index.write().unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        {
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[]).unwrap();
        }
        repo
    }

    #[test]
    fn saves_lists_and_pops_a_stash() {
        let tmp = tempfile::tempdir().unwrap();
        let mut repo = init_repo_with_commit(tmp.path());
        std::fs::write(tmp.path().join("a.txt"), "changed\n").unwrap();

        let head_sha = repo.head().unwrap().target().unwrap().to_string();
        let info = save(&mut repo, Some("wip work"), false).unwrap();
        assert_eq!(info.message, "wip work");
        assert_eq!(info.base_sha, head_sha);
        assert!(info.date > 0);

        // working dir should be clean again after stashing
        assert_eq!(std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(), "hello\n");

        let stashes = list(&mut repo).unwrap();
        assert_eq!(stashes.len(), 1);
        assert!(stashes[0].message.contains("wip work"));
        assert_eq!(stashes[0].base_sha, head_sha);

        let result = pop(&mut repo, 0).unwrap();
        assert_eq!(result.conflicts.len(), 0);
        assert_eq!(std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(), "changed\n");
        assert_eq!(list(&mut repo).unwrap().len(), 0);
    }

    #[test]
    fn applies_and_drops_a_stash_without_removing_workdir_changes_until_dropped() {
        let tmp = tempfile::tempdir().unwrap();
        let mut repo = init_repo_with_commit(tmp.path());
        std::fs::write(tmp.path().join("a.txt"), "changed\n").unwrap();

        save(&mut repo, Some("wip work"), false).unwrap();
        let result = apply(&mut repo, 0).unwrap();
        assert_eq!(result.conflicts.len(), 0);
        assert_eq!(std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(), "changed\n");
        // apply keeps the stash around (unlike pop)
        assert_eq!(list(&mut repo).unwrap().len(), 1);

        drop(&mut repo, 0).unwrap();
        assert_eq!(list(&mut repo).unwrap().len(), 0);
    }
}

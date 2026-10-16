//! A cheap "did the repository's refs move?" token, so the UI's 5s poll can
//! skip re-reading the commit log, branches and stashes (the log re-requests
//! every loaded page) when nothing but working-tree files changed.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::SystemTime;

use crate::git_client::error::GitCommandError;

fn stamp(path: &Path) -> (bool, u128, u64) {
    match std::fs::metadata(path) {
        Ok(m) => {
            let nanos = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            (true, nanos, m.len())
        }
        Err(_) => (false, 0, 0),
    }
}

/// Every directory under `dir` (and `dir` itself): git writes a ref through a
/// lock file renamed into place, which bumps its directory's mtime — so
/// directory stamps are enough, no per-ref stat.
fn hash_dirs(dir: &Path, h: &mut DefaultHasher) {
    stamp(dir).hash(h);
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut subdirs: Vec<_> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    subdirs.sort();
    for sub in subdirs {
        hash_dirs(&sub, h);
    }
}

/// Changes whenever HEAD, a branch/tag/remote ref, the stash or an in-progress
/// operation (merge, rebase, cherry-pick, revert) changes. Not the working tree.
pub fn fingerprint(repo_path: &Path) -> Result<String, GitCommandError> {
    let repo = git2::Repository::open(repo_path)?;
    let git_dir = repo.path();
    let common = repo.commondir();
    let mut h = DefaultHasher::new();
    for name in ["HEAD", "MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "rebase-merge", "rebase-apply"] {
        stamp(&git_dir.join(name)).hash(&mut h);
    }
    stamp(&common.join("packed-refs")).hash(&mut h);
    hash_dirs(&common.join("refs"), &mut h);
    stamp(&common.join("logs/refs/stash")).hash(&mut h);
    Ok(format!("{:016x}", h.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(repo: &git2::Repository, name: &str) {
        std::fs::write(repo.workdir().unwrap().join(name), name).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(name)).unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("T", "t@example.com").unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<_> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, name, &tree, &parents).unwrap();
    }

    #[test]
    fn changes_with_refs_but_not_with_working_tree_edits() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit(&repo, "a");
        let before = fingerprint(tmp.path()).unwrap();
        assert_eq!(before, fingerprint(tmp.path()).unwrap());

        // A file edit leaves refs alone.
        std::fs::write(tmp.path().join("a"), "edited").unwrap();
        std::fs::write(tmp.path().join("untracked"), "x").unwrap();
        assert_eq!(before, fingerprint(tmp.path()).unwrap());

        // A new commit moves the branch ref.
        std::thread::sleep(std::time::Duration::from_millis(20));
        commit(&repo, "b");
        let after_commit = fingerprint(tmp.path()).unwrap();
        assert_ne!(before, after_commit);

        // A new branch adds a ref.
        std::thread::sleep(std::time::Duration::from_millis(20));
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feat/x", &head, false).unwrap();
        assert_ne!(after_commit, fingerprint(tmp.path()).unwrap());
    }
}

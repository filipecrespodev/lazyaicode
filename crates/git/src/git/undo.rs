//! Undo/redo for every local action the app performs.
//!
//! The repo actor snapshots the local state (HEAD, local branches, stashes,
//! index and working tree) before and after each mutating command. Undo puts
//! the "before" snapshot back; redo the "after" one. Remote state (pushes,
//! deleted remote branches) is never touched — it can't be taken back
//! without rewriting the server.
//!
//! Safety rule: an entry is only undone (or redone) while the repo is still
//! exactly in the state that entry left it in. If anything changed since —
//! an edit in the editor, a commit from the terminal — undo refuses instead
//! of silently throwing that work away.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::GitCommandError;

const MAX_ENTRIES: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadState {
    Branch(String),
    Detached(git2::Oid),
    /// No commit yet.
    Unborn(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashEntry {
    pub oid: git2::Oid,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub head: HeadState,
    pub branches: BTreeMap<String, git2::Oid>,
    /// Newest first, like `git stash list`.
    pub stashes: Vec<StashEntry>,
    /// `None` while the index has unresolved conflicts (no tree can be written).
    pub index_tree: Option<git2::Oid>,
    /// Tracked + untracked (not ignored) files as they are on disk.
    pub workdir_tree: Option<git2::Oid>,
    /// A rebase/merge/cherry-pick is mid-way: this state can be left (undo),
    /// but not recreated (redo), since its `.git/*` state files aren't saved.
    pub in_progress: bool,
}

pub fn capture(repo: &mut git2::Repository) -> Result<Snapshot, GitCommandError> {
    let head = match repo.head() {
        Ok(h) if h.is_branch() => HeadState::Branch(h.shorthand().unwrap_or("").to_string()),
        Ok(h) => HeadState::Detached(h.target().ok_or_else(|| GitCommandError::Internal("o HEAD não aponta para nada".into()))?),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
            let target = repo
                .find_reference("HEAD")?
                .symbolic_target()
                .ok()
                .flatten()
                .unwrap_or("refs/heads/main")
                .trim_start_matches("refs/heads/")
                .to_string();
            HeadState::Unborn(target)
        }
        Err(e) => return Err(e.into()),
    };

    let mut branches = BTreeMap::new();
    for b in repo.branches(Some(git2::BranchType::Local))? {
        let (b, _) = b?;
        if let (Some(name), Some(oid)) = (b.name()?, b.get().target()) {
            branches.insert(name.to_string(), oid);
        }
    }

    let mut stashes = Vec::new();
    repo.stash_foreach(|_, message, oid| {
        stashes.push(StashEntry { oid: *oid, message: message.to_string() });
        true
    })?;

    let (index_tree, workdir_tree) = capture_trees(repo)?;

    Ok(Snapshot { head, branches, stashes, index_tree, workdir_tree, in_progress: repo.state() != git2::RepositoryState::Clean })
}

/// Writes the index and the working tree as tree objects, without changing
/// either: the working tree is staged into the in-memory index, written, and
/// the index is then re-read from disk, discarding that in-memory staging.
fn capture_trees(repo: &git2::Repository) -> Result<(Option<git2::Oid>, Option<git2::Oid>), GitCommandError> {
    if repo.is_bare() {
        return Ok((None, None));
    }
    let mut index = repo.index()?;
    if index.has_conflicts() {
        return Ok((None, None));
    }
    let index_tree = index.write_tree()?;
    let workdir = (|| -> Result<git2::Oid, git2::Error> {
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        // Tracked files deleted from disk.
        index.update_all(["*"].iter(), None)?;
        index.write_tree()
    })();
    index.read(true)?;
    Ok((Some(index_tree), Some(workdir?)))
}

/// Puts the repository back into `snap`'s state.
pub fn restore(repo: &mut git2::Repository, snap: &Snapshot) -> Result<(), GitCommandError> {
    leave_in_progress_operation(repo)?;

    // HEAD first (detached at its current commit) so any branch, including
    // the checked-out one, can be moved or deleted below.
    if let Ok(oid) = repo.head().and_then(|h| h.peel_to_commit()).map(|c| c.id()) {
        repo.set_head_detached(oid)?;
    }

    for b in repo.branches(Some(git2::BranchType::Local))? {
        let (mut b, _) = b?;
        let name = b.name()?.unwrap_or("").to_string();
        if !snap.branches.contains_key(&name) {
            b.delete()?;
        }
    }
    for (name, oid) in &snap.branches {
        let commit = repo.find_commit(*oid)?;
        repo.branch(name, &commit, true)?;
    }

    match &snap.head {
        HeadState::Branch(name) | HeadState::Unborn(name) => repo.set_head(&format!("refs/heads/{name}"))?,
        HeadState::Detached(oid) => repo.set_head_detached(*oid)?,
    }

    restore_stashes(repo, &snap.stashes)?;

    if let (Some(workdir_tree), Some(index_tree)) = (snap.workdir_tree, snap.index_tree) {
        let tree = repo.find_tree(workdir_tree)?;
        repo.checkout_tree(tree.as_object(), Some(git2::build::CheckoutBuilder::new().force().remove_untracked(true)))?;
        let mut index = repo.index()?;
        index.read_tree(&repo.find_tree(index_tree)?)?;
        index.write()?;
    }
    Ok(())
}

/// Drops rebase/merge/cherry-pick state files; the refs and files they'd
/// restore on abort are put back by `restore` itself.
fn leave_in_progress_operation(repo: &git2::Repository) -> Result<(), GitCommandError> {
    for dir in ["rebase-merge", "rebase-apply"] {
        let path = repo.path().join(dir);
        if path.exists() {
            std::fs::remove_dir_all(&path).map_err(|e| GitCommandError::Internal(format!("falha ao limpar {dir}: {e}")))?;
        }
    }
    repo.cleanup_state()?;
    Ok(())
}

/// Rewrites `refs/stash` and its reflog (which *is* the stash list) to `stashes`.
fn restore_stashes(repo: &git2::Repository, stashes: &[StashEntry]) -> Result<(), GitCommandError> {
    if let Ok(mut r) = repo.find_reference("refs/stash") {
        r.delete()?;
    }
    let reflog_path = repo.path().join("logs/refs/stash");
    if reflog_path.exists() {
        let _ = std::fs::remove_file(&reflog_path);
    }
    if stashes.is_empty() {
        return Ok(());
    }
    repo.reference_ensure_log("refs/stash")?;
    // Oldest first, so the newest ends up as stash@{0}.
    for s in stashes.iter().rev() {
        repo.reference("refs/stash", s.oid, true, &s.message)?;
    }
    Ok(())
}

struct Entry {
    label: String,
    before: Snapshot,
    after: Snapshot,
}

#[derive(Debug, Clone, Serialize, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoStatus {
    /// Label of the action Undo would revert.
    pub undo: Option<String>,
    /// Label of the action Redo would re-apply.
    pub redo: Option<String>,
}

#[derive(Default)]
pub struct UndoHistory {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
}

impl UndoHistory {
    pub fn status(&self) -> UndoStatus {
        UndoStatus {
            undo: self.undo.last().map(|e| e.label.clone()),
            redo: self.redo.last().filter(|e| !e.after.in_progress).map(|e| e.label.clone()),
        }
    }

    /// Records a finished action. Actions taken while an operation (rebase,
    /// merge…) was already in progress — conflict resolution, staging the
    /// fix, continue/abort — belong to that operation: they extend its entry
    /// instead of adding their own, so one undo reverts the whole thing.
    pub fn record(&mut self, label: String, before: Snapshot, after: Snapshot, continuation: bool) {
        if before == after {
            return; // failed or no-op
        }
        if continuation || before.in_progress {
            if let Some(top) = self.undo.last_mut() {
                if top.after == before {
                    top.after = after;
                    self.redo.clear();
                    return;
                }
            }
        }
        self.undo.push(Entry { label, before, after });
        if self.undo.len() > MAX_ENTRIES {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self, repo: &mut git2::Repository) -> Result<String, GitCommandError> {
        let Some(entry) = self.undo.last() else {
            return Err(GitCommandError::Internal("nada para desfazer".into()));
        };
        ensure_unchanged(repo, &entry.after)?;
        restore(repo, &entry.before)?;
        let entry = self.undo.pop().unwrap();
        let label = entry.label.clone();
        self.redo.push(entry);
        Ok(label)
    }

    pub fn redo(&mut self, repo: &mut git2::Repository) -> Result<String, GitCommandError> {
        let Some(entry) = self.redo.last() else {
            return Err(GitCommandError::Internal("nada para refazer".into()));
        };
        if entry.after.in_progress {
            return Err(GitCommandError::Internal("esta ação parou no meio (conflitos) e não pode ser refeita; execute-a de novo".into()));
        }
        ensure_unchanged(repo, &entry.before)?;
        restore(repo, &entry.after)?;
        let entry = self.redo.pop().unwrap();
        let label = entry.label.clone();
        self.undo.push(entry);
        Ok(label)
    }
}

fn ensure_unchanged(repo: &mut git2::Repository, expected: &Snapshot) -> Result<(), GitCommandError> {
    let current = capture(repo)?;
    if &current == expected {
        return Ok(());
    }
    let what = if current.workdir_tree != expected.workdir_tree || current.index_tree != expected.index_tree {
        "arquivos foram alterados"
    } else {
        "branches ou stashes foram alterados"
    };
    Err(GitCommandError::Internal(format!(
        "não é possível desfazer: {what} desde essa ação (fora do LazyAICode ou em uma etapa posterior). \
         Faça commit ou stash das suas alterações, ou reverta-as, e tente de novo"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn setup() -> (tempfile::TempDir, git2::Repository) {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        {
            let mut cfg = repo.config().unwrap();
            cfg.set_str("user.name", "Test").unwrap();
            cfg.set_str("user.email", "test@example.com").unwrap();
        }
        write(&repo, "a.txt", "one\n");
        commit_all(&repo, "first");
        (tmp, repo)
    }

    fn write(repo: &git2::Repository, name: &str, content: &str) {
        std::fs::write(repo.workdir().unwrap().join(name), content).unwrap();
    }

    fn read(repo: &git2::Repository, name: &str) -> Option<String> {
        std::fs::read_to_string(repo.workdir().unwrap().join(name)).ok()
    }

    fn commit_all(repo: &git2::Repository, msg: &str) -> git2::Oid {
        let mut index = repo.index().unwrap();
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = repo.signature().unwrap();
        let parents: Vec<git2::Commit> = repo.head().ok().and_then(|h| h.peel_to_commit().ok()).into_iter().collect();
        let refs: Vec<&git2::Commit> = parents.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &refs).unwrap()
    }

    /// Runs `action` the way the actor does: snapshot, act, snapshot, record.
    fn act(h: &mut UndoHistory, repo: &mut git2::Repository, label: &str, action: impl FnOnce(&mut git2::Repository)) {
        let before = capture(repo).unwrap();
        action(repo);
        let after = capture(repo).unwrap();
        h.record(label.into(), before, after, false);
    }

    #[test]
    fn capturing_does_not_stage_anything() {
        let (_t, mut repo) = setup();
        write(&repo, "new.txt", "untracked\n");
        write(&repo, "a.txt", "modified\n");
        capture(&mut repo).unwrap();
        let statuses = repo.statuses(None).unwrap();
        assert!(statuses.iter().all(|s| !s.status().intersects(git2::Status::INDEX_NEW | git2::Status::INDEX_MODIFIED)));
    }

    #[test]
    fn undoes_and_redoes_a_commit() {
        let (_t, mut repo) = setup();
        let first = repo.head().unwrap().target().unwrap();
        let mut h = UndoHistory::default();
        write(&repo, "a.txt", "two\n");
        act(&mut h, &mut repo, "Commit", |r| {
            commit_all(r, "second");
        });
        let second = repo.head().unwrap().target().unwrap();

        assert_eq!(h.undo(&mut repo).unwrap(), "Commit");
        assert_eq!(repo.head().unwrap().target().unwrap(), first);
        // The committed change is back as an uncommitted change.
        assert_eq!(read(&repo, "a.txt").as_deref(), Some("two\n"));
        assert_eq!(h.status().redo.as_deref(), Some("Commit"));

        h.redo(&mut repo).unwrap();
        assert_eq!(repo.head().unwrap().target().unwrap(), second);
    }

    #[test]
    fn brings_back_discarded_changes_and_deleted_untracked_files() {
        let (_t, mut repo) = setup();
        let mut h = UndoHistory::default();
        write(&repo, "a.txt", "precious edit\n");
        write(&repo, "notes.txt", "untracked\n");
        act(&mut h, &mut repo, "Discard all", |r| {
            let head = r.head().unwrap().peel_to_commit().unwrap();
            r.reset(head.as_object(), git2::ResetType::Hard, None).unwrap();
            std::fs::remove_file(r.workdir().unwrap().join("notes.txt")).unwrap();
        });
        assert_eq!(read(&repo, "a.txt").as_deref(), Some("one\n"));

        h.undo(&mut repo).unwrap();
        assert_eq!(read(&repo, "a.txt").as_deref(), Some("precious edit\n"));
        assert_eq!(read(&repo, "notes.txt").as_deref(), Some("untracked\n"));
    }

    #[test]
    fn restores_a_deleted_branch_and_the_checked_out_branch() {
        let (_t, mut repo) = setup();
        {
            let head = repo.head().unwrap().peel_to_commit().unwrap();
            repo.branch("feature", &head, false).unwrap();
        }
        let main = repo.head().unwrap().shorthand().unwrap().to_string();
        let mut h = UndoHistory::default();
        act(&mut h, &mut repo, "Delete branch feature", |r| {
            r.find_branch("feature", git2::BranchType::Local).unwrap().delete().unwrap();
        });
        act(&mut h, &mut repo, "Create and checkout x", |r| {
            let c = r.head().unwrap().peel_to_commit().unwrap();
            r.branch("x", &c, false).unwrap();
            r.set_head("refs/heads/x").unwrap();
        });

        h.undo(&mut repo).unwrap();
        assert_eq!(repo.head().unwrap().shorthand().ok(), Some(main.as_str()));
        assert!(repo.find_branch("x", git2::BranchType::Local).is_err());
        h.undo(&mut repo).unwrap();
        assert!(repo.find_branch("feature", git2::BranchType::Local).is_ok());
    }

    #[test]
    fn restores_a_dropped_stash() {
        let (_t, mut repo) = setup();
        write(&repo, "a.txt", "stashed\n");
        let sig = repo.signature().unwrap();
        repo.stash_save(&sig, "keep me", None).unwrap();
        let mut h = UndoHistory::default();
        act(&mut h, &mut repo, "Drop stash", |r| r.stash_drop(0).unwrap());

        h.undo(&mut repo).unwrap();
        let mut messages = Vec::new();
        repo.stash_foreach(|_, m, _| {
            messages.push(m.to_string());
            true
        })
        .unwrap();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("keep me"));
    }

    #[test]
    fn refuses_to_undo_when_files_changed_afterwards() {
        let (_t, mut repo) = setup();
        let mut h = UndoHistory::default();
        write(&repo, "a.txt", "two\n");
        act(&mut h, &mut repo, "Commit", |r| {
            commit_all(r, "second");
        });
        write(&repo, "a.txt", "edited after the commit\n");

        assert!(h.undo(&mut repo).is_err());
        // Nothing was touched.
        assert_eq!(read(&repo, "a.txt").as_deref(), Some("edited after the commit\n"));
        assert_eq!(h.status().undo.as_deref(), Some("Commit"));
    }

    #[test]
    fn no_op_actions_are_not_recorded_and_new_actions_clear_redo() {
        let (_t, mut repo) = setup();
        let mut h = UndoHistory::default();
        act(&mut h, &mut repo, "Nothing", |_| {});
        assert!(h.status().undo.is_none());

        write(&repo, "a.txt", "two\n");
        act(&mut h, &mut repo, "Commit", |r| {
            commit_all(r, "second");
        });
        h.undo(&mut repo).unwrap();
        write(&repo, "b.txt", "x\n");
        act(&mut h, &mut repo, "Stage b", |r| {
            let mut i = r.index().unwrap();
            i.add_path(Path::new("b.txt")).unwrap();
            i.write().unwrap();
        });
        assert!(h.status().redo.is_none());
    }
}

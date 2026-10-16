use serde::{Deserialize, Serialize};

use crate::error::GitCommandError;

use super::signing;

#[derive(Debug, Serialize, Deserialize)]
pub enum RebaseState {
    Conflicts,
    Complete,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebaseStatus {
    pub state: RebaseState,
    pub current_step: u32,
    pub total_steps: u32,
    pub conflicts: Vec<ConflictedFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictedFile {
    pub path: String,
    pub ancestor_content: Option<String>,
    pub ours_content: Option<String>,
    pub theirs_content: Option<String>,
    pub is_binary: bool,
    /// Unified diff between `ours_content` and `theirs_content`, for the
    /// two-pane conflict resolution editor. Empty for binary files.
    pub patch: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", content = "content")]
pub enum ConflictResolution {
    UseOurs,
    UseTheirs,
    UseManualContent(String),
}

pub fn start<'repo>(repo: &'repo git2::Repository, onto_branch: &str) -> Result<(git2::Rebase<'repo>, RebaseStatus), GitCommandError> {
    let onto_ref = repo.resolve_reference_from_short_name(onto_branch)?;
    let onto = repo.reference_to_annotated_commit(&onto_ref)?;
    // A signing setup that can't work (no key) fails here, before anything moved.
    signing::signer_for(repo)?;
    let mut rebase = repo.rebase(None, None, Some(&onto), None)?;
    let status = advance(repo, &mut rebase, false)?;
    Ok((rebase, status))
}

const AUTOSTASH_MARKER: &str = "GK_REBASE_AUTOSTASH";

fn autostash_marker_path(repo: &git2::Repository) -> std::path::PathBuf {
    repo.path().join(AUTOSTASH_MARKER)
}

/// Stashes uncommitted changes before a rebase starts (mirroring `git rebase
/// --autostash`, which libgit2 doesn't implement natively) and remembers the
/// stash's sha in a marker file so it can be restored once the rebase is
/// done, even across the multiple separate IPC calls (start/continue/abort)
/// a single rebase spans.
pub fn autostash(repo: &mut git2::Repository) -> Result<(), GitCommandError> {
    let info = super::stash::save(repo, Some("gk-clone: rebase autostash"), false)?;
    std::fs::write(autostash_marker_path(repo), &info.sha).map_err(|e| GitCommandError::Internal(e.to_string()))?;
    Ok(())
}

/// Restores the autostash created by `autostash`, if any, once the rebase
/// that triggered it is done (completed or aborted). Looks the stash up by
/// sha rather than assuming index 0, in case other stashes were created in
/// the meantime. Returns any conflicts from re-applying it — rare, but
/// possible if the rebase itself touched the same lines.
pub fn pop_autostash_if_any(repo: &mut git2::Repository) -> Result<Vec<ConflictedFile>, GitCommandError> {
    let marker = autostash_marker_path(repo);
    let Ok(sha) = std::fs::read_to_string(&marker) else {
        return Ok(Vec::new());
    };
    let _ = std::fs::remove_file(&marker);

    let stashes = super::stash::list(repo)?;
    let Some(found) = stashes.iter().find(|s| s.sha == sha.trim()) else {
        return Ok(Vec::new());
    };
    // Like git: a stash that comes back with conflicts is kept, so the
    // changes stay recoverable after the conflict is resolved (or botched).
    let result = super::stash::apply(repo, found.index)?;
    if result.conflicts.is_empty() {
        super::stash::drop(repo, found.index)?;
    }
    Ok(result.conflicts)
}

/// Tracked changes only: every caller stashes without untracked files, and
/// libgit2 refuses to save a stash with nothing in it, so counting untracked
/// files here would make an autostash fail on an otherwise clean tree.
/// Skipping them also spares the status scan a walk of untracked dirs.
pub fn has_uncommitted_changes(repo: &git2::Repository) -> Result<bool, GitCommandError> {
    let dirty = git2::Status::WT_MODIFIED
        | git2::Status::WT_DELETED
        | git2::Status::WT_TYPECHANGE
        | git2::Status::WT_RENAMED
        | git2::Status::INDEX_NEW
        | git2::Status::INDEX_MODIFIED
        | git2::Status::INDEX_DELETED
        | git2::Status::INDEX_TYPECHANGE
        | git2::Status::INDEX_RENAMED;
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(false);
    Ok(repo.statuses(Some(&mut opts))?.iter().any(|entry| entry.status().intersects(dirty)))
}

/// Applies rebase operations one after another, committing each clean step,
/// until either a conflict blocks progress or the rebase finishes.
/// `commit_pending` should be true when resuming after conflicts on the
/// current step were just resolved (that step still needs to be committed).
pub fn advance(repo: &git2::Repository, rebase: &mut git2::Rebase, commit_pending: bool) -> Result<RebaseStatus, GitCommandError> {
    let sig = repo.signature()?;
    let signer = signing::signer_for(repo)?;
    if commit_pending {
        commit_or_drop(repo, rebase, &sig, signer.as_ref())?;
    }

    loop {
        match rebase.next() {
            None => {
                rebase.finish(Some(&sig))?;
                return Ok(RebaseStatus {
                    state: RebaseState::Complete,
                    current_step: rebase.len() as u32,
                    total_steps: rebase.len() as u32,
                    conflicts: Vec::new(),
                });
            }
            Some(op) => {
                op?;
                if repo.index()?.has_conflicts() {
                    return conflict_status(repo, rebase);
                }
                commit_or_drop(repo, rebase, &sig, signer.as_ref())?;
            }
        }
    }
}

/// Commits the current rebase step, silently dropping it when its changes
/// are already in the new base (libgit2 reports that as `Applied`) — like
/// `git rebase` skipping already-upstream commits, so no duplicate or empty
/// commit is created.
///
/// libgit2's rebase can't sign, so when commits must be signed the step's
/// commit is made again, signed (same tree, parents, author and message),
/// and the rebase's detached HEAD moved onto it: the next step builds on
/// the signed one, and finishing moves the branch there. If signing fails,
/// HEAD goes back so continuing commits (and signs) the step again.
fn commit_or_drop(
    repo: &git2::Repository,
    rebase: &mut git2::Rebase,
    sig: &git2::Signature,
    signer: Option<&signing::Signer>,
) -> Result<(), GitCommandError> {
    let before = repo.head()?.target();
    let oid = match rebase.commit(None, sig, None) {
        Err(e) if e.code() == git2::ErrorCode::Applied => return Ok(()),
        r => r?,
    };
    let Some(signer) = signer else {
        return Ok(());
    };
    let signed = (|| {
        let commit = repo.find_commit(oid)?;
        let parents: Vec<git2::Commit> = commit.parents().collect();
        let parents: Vec<&git2::Commit> = parents.iter().collect();
        let message = commit.message_raw().map_err(|_| GitCommandError::Internal("a mensagem do commit não é UTF-8 válido".into()))?;
        let (author, committer, tree) = (commit.author(), commit.committer(), commit.tree()?);
        let signed = signing::create_commit_with(repo, Some(signer), &author, &committer, message, &tree, &parents)?;
        Ok::<_, GitCommandError>(signed)
    })();
    match signed {
        Ok(signed) => Ok(repo.set_head_detached(signed)?),
        Err(e) => {
            if let Some(before) = before {
                let _ = repo.set_head_detached(before);
            }
            Err(e)
        }
    }
}

pub fn resolve_conflict(repo: &git2::Repository, path: &str, resolution: ConflictResolution) -> Result<(), GitCommandError> {
    let workdir = repo.workdir().ok_or_else(|| GitCommandError::Internal("repositório bare não suporta checkout".into()))?;
    let full_path = workdir.join(path);
    let mut index = repo.index()?;

    let conflict = index
        .conflicts()?
        .filter_map(|c| c.ok())
        .find(|c| entry_path(c) == Some(path))
        .ok_or_else(|| GitCommandError::Internal(format!("nenhum conflito em {path}")))?;

    let side = match resolution {
        ConflictResolution::UseOurs => conflict.our,
        ConflictResolution::UseTheirs => conflict.their,
        ConflictResolution::UseManualContent(content) => {
            std::fs::write(&full_path, content).map_err(|e| GitCommandError::Internal(e.to_string()))?;
            index.add_path(std::path::Path::new(path))?;
            index.write()?;
            return Ok(());
        }
    };

    match side {
        Some(entry) => {
            let blob = repo.find_blob(entry.id)?;
            std::fs::write(&full_path, blob.content()).map_err(|e| GitCommandError::Internal(e.to_string()))?;
            index.add_path(std::path::Path::new(path))?;
        }
        None => {
            let _ = std::fs::remove_file(&full_path);
            index.remove_path(std::path::Path::new(path))?;
        }
    }
    index.write()?;
    Ok(())
}

pub fn abort(rebase: &mut git2::Rebase) -> Result<(), GitCommandError> {
    rebase.abort()?;
    Ok(())
}

pub fn conflict_status(repo: &git2::Repository, rebase: &mut git2::Rebase) -> Result<RebaseStatus, GitCommandError> {
    Ok(RebaseStatus {
        state: RebaseState::Conflicts,
        current_step: rebase.operation_current().unwrap_or(0) as u32,
        total_steps: rebase.len() as u32,
        conflicts: collect_conflicted_files(repo)?,
    })
}

/// True when `.git/rebase-merge` or `.git/rebase-apply` exists on disk —
/// a rebase is in progress, whether or not libgit2's `Rebase` can open it
/// (it categorically refuses interactive rebases, i.e. `git rebase -i`).
pub fn is_rebase_in_progress(repo: &git2::Repository) -> bool {
    repo.path().join("rebase-merge").exists() || repo.path().join("rebase-apply").exists()
}

/// Reads rebase progress straight off the on-disk state files, bypassing
/// libgit2's `Rebase` entirely — used when `open_rebase` refuses a rebase
/// (interactive ones) but conflicts still need to be listed/resolved, which
/// only ever needs `repo.index()`, not a `Rebase` session.
fn external_rebase_status(repo: &git2::Repository) -> Result<Option<RebaseStatus>, GitCommandError> {
    let git_dir = repo.path();
    let (state_dir, current_file, total_file) = if git_dir.join("rebase-merge").exists() {
        (git_dir.join("rebase-merge"), "msgnum", "end")
    } else if git_dir.join("rebase-apply").exists() {
        (git_dir.join("rebase-apply"), "next", "last")
    } else {
        return Ok(None);
    };

    let read_num =
        |name: &str| -> u32 { std::fs::read_to_string(state_dir.join(name)).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0) };

    Ok(Some(RebaseStatus {
        state: RebaseState::Conflicts,
        current_step: read_num(current_file),
        total_steps: read_num(total_file),
        conflicts: collect_conflicted_files(repo)?,
    }))
}

/// The single entry point for "what's the rebase status right now" —
/// prefers libgit2's `Rebase`, falling back to the on-disk reader above for
/// rebases it can't open, and finally to a raw index-conflict check for
/// conflicts that aren't from a rebase at all (e.g. a stash apply from the
/// "sync branch to remote" flow, or a `git merge`/`git stash apply` run from
/// a terminal) — conflict resolution doesn't care what caused them. `None`
/// means nothing is in progress and nothing is conflicted.
pub fn read_status(repo: &git2::Repository) -> Result<Option<RebaseStatus>, GitCommandError> {
    if let Ok(mut session) = repo.open_rebase(None) {
        return conflict_status(repo, &mut session).map(Some);
    }
    if let Some(status) = external_rebase_status(repo)? {
        return Ok(Some(status));
    }
    let conflicts = collect_conflicted_files(repo)?;
    if conflicts.is_empty() {
        Ok(None)
    } else {
        Ok(Some(RebaseStatus { state: RebaseState::Conflicts, current_step: 0, total_steps: 0, conflicts }))
    }
}

pub fn collect_conflicted_files(repo: &git2::Repository) -> Result<Vec<ConflictedFile>, GitCommandError> {
    let index = repo.index()?;
    let mut conflicts = Vec::new();
    for conflict in index.conflicts()?.filter_map(|c| c.ok()) {
        let Some(path) = entry_path(&conflict) else {
            continue;
        };
        let is_binary = [&conflict.ancestor, &conflict.our, &conflict.their]
            .iter()
            .filter_map(|e| e.as_ref())
            .filter_map(|e| repo.find_blob(e.id).ok())
            .any(|b| b.is_binary());

        let ours_content = read_blob_text(repo, &conflict.our);
        let theirs_content = read_blob_text(repo, &conflict.their);
        let patch = if is_binary {
            String::new()
        } else {
            // Full-file context (not the default 3 lines) — the conflict
            // editor shows the whole file, not just the changed hunks.
            let mut opts = git2::DiffOptions::new();
            opts.context_lines(super::diff::FULL_CONTEXT);
            let mut p = git2::Patch::from_buffers(
                ours_content.as_deref().unwrap_or("").as_bytes(),
                Some(std::path::Path::new(path)),
                theirs_content.as_deref().unwrap_or("").as_bytes(),
                Some(std::path::Path::new(path)),
                Some(&mut opts),
            )?;
            String::from_utf8_lossy(&p.to_buf()?).into_owned()
        };

        conflicts.push(ConflictedFile {
            path: path.to_string(),
            ancestor_content: read_blob_text(repo, &conflict.ancestor),
            ours_content,
            theirs_content,
            is_binary,
            patch,
        });
    }
    Ok(conflicts)
}

fn entry_path(conflict: &git2::IndexConflict) -> Option<&str> {
    [&conflict.our, &conflict.their, &conflict.ancestor].iter().find_map(|e| e.as_ref()).and_then(|e| std::str::from_utf8(&e.path).ok())
}

fn read_blob_text(repo: &git2::Repository, entry: &Option<git2::IndexEntry>) -> Option<String> {
    let entry = entry.as_ref()?;
    let blob = repo.find_blob(entry.id).ok()?;
    Some(String::from_utf8_lossy(blob.content()).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::branch;
    use std::path::Path;

    fn commit_file(repo: &git2::Repository, content: &str, msg: &str) -> git2::Oid {
        std::fs::write(repo.workdir().unwrap().join("file.txt"), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        let tree_id = index.write_tree().unwrap();
        index.write().unwrap();

        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().ok().and_then(|h| h.target()).and_then(|oid| repo.find_commit(oid).ok());
        let parents: Vec<_> = parent.into_iter().collect();
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parent_refs).unwrap()
    }

    #[test]
    fn rebases_cleanly_when_no_conflicts() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit_file(&repo, "line1\n", "base");
        let base_branch = repo.head().unwrap().shorthand().unwrap().to_string();
        let base_commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feature", &base_commit, false).unwrap();

        std::fs::write(tmp.path().join("other.txt"), "x").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("other.txt")).unwrap();
        let tree_id = index.write_tree().unwrap();
        index.write().unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        {
            let parent = base_commit;
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "main change", &tree, &[&parent]).unwrap();
        }

        branch::checkout_branch(&repo, "feature").unwrap();
        commit_file(&repo, "line1\nfeature change\n", "feature change");

        let (_session, status) = start(&repo, &base_branch).unwrap();
        assert!(matches!(status.state, RebaseState::Complete));
    }

    /// libgit2's rebase can't sign: every rewritten commit must still come
    /// out signed (and verify with git) when the repo signs commits.
    #[cfg(unix)]
    #[test]
    fn rebased_commits_are_signed() {
        use crate::git::signing::tests::{git_verifies, keygen};
        let tmp = tempfile::tempdir().unwrap();
        let Some(key) = keygen(tmp.path(), "id", "") else { return };
        let repo = git2::Repository::init(tmp.path().join("repo")).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "T").unwrap();
        config.set_str("user.email", "t@example.com").unwrap();
        config.set_bool("core.autocrlf", false).unwrap();
        commit_file(&repo, "line1\n", "base");
        let base_branch = repo.head().unwrap().shorthand().unwrap().to_string();
        let base_commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feature", &base_commit, false).unwrap();
        std::fs::write(repo.workdir().unwrap().join("other.txt"), "x").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("other.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "main change", &tree, &[&base_commit]).unwrap();
        branch::checkout_branch(&repo, "feature").unwrap();
        commit_file(&repo, "line1\nf1\n", "feature 1");
        commit_file(&repo, "line1\nf1\nf2\n", "feature 2");

        config.set_bool("commit.gpgsign", true).unwrap();
        config.set_str("gpg.format", "ssh").unwrap();
        config.set_str("user.signingkey", &key.display().to_string()).unwrap();
        let (_session, status) = start(&repo, &base_branch).unwrap();
        assert!(matches!(status.state, RebaseState::Complete));

        let head = repo.find_reference("refs/heads/feature").unwrap().peel_to_commit().unwrap();
        assert_eq!(repo.head().unwrap().shorthand(), Ok("feature"));
        let rewritten = [head.clone(), head.parent(0).unwrap()];
        assert_eq!(rewritten[0].summary(), Ok(Some("feature 2")));
        assert_eq!(rewritten[1].summary(), Ok(Some("feature 1")));
        assert_eq!(rewritten[1].parent(0).unwrap().summary(), Ok(Some("main change")));
        for c in &rewritten {
            assert!(signing::commit_signature(&repo, &c.id().to_string()).unwrap().is_some(), "{c:?} unsigned");
            if let Some(ok) = git_verifies(repo.workdir().unwrap(), &key, &c.id().to_string()) {
                assert!(ok, "git rejected the signature of {c:?}");
            }
        }
        assert_eq!(std::fs::read_to_string(repo.workdir().unwrap().join("file.txt")).unwrap(), "line1\nf1\nf2\n");
    }

    #[test]
    fn rebase_stops_on_conflict_and_resolves_with_theirs() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit_file(&repo, "line1\n", "base");
        let base_branch = repo.head().unwrap().shorthand().unwrap().to_string();
        let base_commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feature", &base_commit, false).unwrap();

        commit_file(&repo, "line1\nfrom-main\n", "main change");

        branch::checkout_branch(&repo, "feature").unwrap();
        commit_file(&repo, "line1\nfrom-feature\n", "feature change");

        let (mut session, status) = start(&repo, &base_branch).unwrap();
        assert!(matches!(status.state, RebaseState::Conflicts));
        assert_eq!(status.conflicts.len(), 1);
        assert_eq!(status.conflicts[0].path, "file.txt");

        assert!(status.conflicts[0].patch.contains("from-feature"));
        assert!(status.conflicts[0].patch.contains("from-main"));

        resolve_conflict(&repo, "file.txt", ConflictResolution::UseTheirs).unwrap();
        let final_status = advance(&repo, &mut session, true).unwrap();
        assert!(matches!(final_status.state, RebaseState::Complete));
    }

    #[test]
    fn falls_back_to_on_disk_state_when_libgit2_refuses_an_interactive_rebase() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit_file(&repo, "line1\n", "base");
        let base_branch = repo.head().unwrap().shorthand().unwrap().to_string();
        let base_commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feature", &base_commit, false).unwrap();

        commit_file(&repo, "line1\nfrom-main\n", "main change");

        branch::checkout_branch(&repo, "feature").unwrap();
        commit_file(&repo, "line1\nfrom-feature\n", "feature change");

        {
            let (_session, status) = start(&repo, &base_branch).unwrap();
            assert!(matches!(status.state, RebaseState::Conflicts));
        }

        // Simulate a rebase libgit2 refuses to open (e.g. `git rebase -i`) by
        // dropping the same marker file real git writes for interactive mode.
        std::fs::write(repo.path().join("rebase-merge").join("interactive"), "").unwrap();
        assert!(repo.open_rebase(None).is_err());

        assert!(is_rebase_in_progress(&repo));
        let status = read_status(&repo).unwrap().unwrap();
        assert!(matches!(status.state, RebaseState::Conflicts));
        assert_eq!(status.conflicts.len(), 1);
        assert_eq!(status.conflicts[0].path, "file.txt");
    }

    #[test]
    fn read_status_reports_conflicts_that_are_not_from_a_rebase_at_all() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit_file(&repo, "line1\n", "base");
        let base_commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feature", &base_commit, false).unwrap();

        commit_file(&repo, "line1\nfrom-main\n", "main change");
        let ours = repo.head().unwrap().peel_to_commit().unwrap();

        branch::checkout_branch(&repo, "feature").unwrap();
        commit_file(&repo, "line1\nfrom-feature\n", "feature change");
        let theirs = repo.head().unwrap().peel_to_commit().unwrap();

        // A conflicted index with no rebase (or rebase-like) session behind
        // it at all — e.g. what a stash re-apply during "sync branch to
        // remote" leaves behind. `read_status` should still surface it.
        let mut index = repo.merge_commits(&ours, &theirs, None).unwrap();
        assert!(index.has_conflicts());
        repo.set_index(&mut index).unwrap();

        assert!(!is_rebase_in_progress(&repo));
        assert!(repo.open_rebase(None).is_err());

        let status = read_status(&repo).unwrap().unwrap();
        assert!(matches!(status.state, RebaseState::Conflicts));
        assert_eq!(status.conflicts.len(), 1);
        assert_eq!(status.conflicts[0].path, "file.txt");
    }

    #[test]
    fn autostashes_dirty_changes_around_a_clean_rebase() {
        let tmp = tempfile::tempdir().unwrap();
        let mut repo = git2::Repository::init(tmp.path()).unwrap();
        commit_file(&repo, "line1\n", "base");
        let base_branch = repo.head().unwrap().shorthand().unwrap().to_string();
        let base_commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feature", &base_commit, false).unwrap();

        std::fs::write(tmp.path().join("other.txt"), "x").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("other.txt")).unwrap();
        let tree_id = index.write_tree().unwrap();
        index.write().unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        {
            let parent = base_commit;
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "main change", &tree, &[&parent]).unwrap();
        }

        branch::checkout_branch(&repo, "feature").unwrap();
        commit_file(&repo, "line1\nfeature change\n", "feature change");

        // dirty, uncommitted change sitting in the workdir when the rebase starts
        std::fs::write(tmp.path().join("file.txt"), "line1\nfeature change\nuncommitted\n").unwrap();
        assert!(has_uncommitted_changes(&repo).unwrap());
        autostash(&mut repo).unwrap();
        assert!(!has_uncommitted_changes(&repo).unwrap());

        let status = {
            let (_session, status) = start(&repo, &base_branch).unwrap();
            status
        };
        assert!(matches!(status.state, RebaseState::Complete));

        let conflicts = pop_autostash_if_any(&mut repo).unwrap();
        assert_eq!(conflicts.len(), 0);
        assert_eq!(std::fs::read_to_string(tmp.path().join("file.txt")).unwrap(), "line1\nfeature change\nuncommitted\n");
        // the marker is one-shot — restoring twice must be a no-op, not double-pop
        assert_eq!(pop_autostash_if_any(&mut repo).unwrap().len(), 0);
    }
}

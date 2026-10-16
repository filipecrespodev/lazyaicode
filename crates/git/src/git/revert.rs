use serde::{Deserialize, Serialize};

use crate::error::GitCommandError;
use crate::git::{hooks, signing};

use super::rebase::{collect_conflicted_files, ConflictedFile};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertResult {
    /// The new revert commit; `None` when it stopped on conflicts.
    pub sha: Option<String>,
    pub conflicts: Vec<ConflictedFile>,
}

/// Same message `git revert` writes, so the history reads the same whichever
/// tool made the commit.
pub fn revert_message(commit: &git2::Commit, mainline_parent: Option<git2::Oid>) -> String {
    let summary = commit.summary().ok().flatten().unwrap_or_default();
    let mut message = format!("Revert \"{summary}\"\n\nThis reverts commit {}", commit.id());
    if let Some(parent) = mainline_parent {
        message.push_str(&format!(", reversing\nchanges made to {parent}"));
    }
    message.push_str(".\n");
    message
}

/// `git revert [-m <mainline>] <sha>`: applies the inverse of `sha` on top of
/// HEAD and commits it. Merge commits need `mainline` (1-based parent the
/// revert is relative to). On conflicts the repo is left in the revert state
/// (REVERT_HEAD + MERGE_MSG) for the shared conflict panel, then finished by
/// [`continue_revert`] or aborted like a merge.
pub fn revert_commit(repo: &git2::Repository, sha: &str, mainline: Option<u32>) -> Result<RevertResult, GitCommandError> {
    if repo.state() != git2::RepositoryState::Clean {
        return Err(GitCommandError::Internal("conclua ou aborte a operação em andamento antes do revert".into()));
    }
    let oid = git2::Oid::from_str(sha).map_err(|_| GitCommandError::RepoNotFound { path: sha.to_string() })?;
    let commit = repo.find_commit(oid)?;
    let parents = commit.parent_count();
    let mainline = match mainline {
        _ if parents < 2 => None,
        None => return Err(GitCommandError::Internal("este é um commit de merge: escolha em relação a qual pai fazer o revert".into())),
        Some(m) if m == 0 || m as usize > parents => return Err(GitCommandError::Internal(format!("o commit de merge não tem o pai {m}"))),
        Some(m) => Some(m),
    };
    // The revert commit is written from the index, so anything already
    // staged would silently ride along with it; and aborting a conflicted
    // revert hard-resets to HEAD, which would wipe unstaged edits too.
    // Untracked files are left alone by both, so they don't count.
    let head_tree = repo.head()?.peel_to_tree()?;
    let mut opts = git2::DiffOptions::new();
    opts.include_untracked(false);
    let dirty = repo.diff_tree_to_workdir_with_index(Some(&head_tree), Some(&mut opts))?.deltas().len() > 0;
    if dirty {
        return Err(GitCommandError::Internal("faça commit ou stash das suas alterações antes do revert".into()));
    }

    let mut opts = git2::RevertOptions::new();
    if let Some(m) = mainline {
        opts.mainline(m);
    }
    repo.revert(&commit, Some(&mut opts))?;

    let message = revert_message(&commit, mainline.map(|m| commit.parent_id(m as usize - 1)).transpose()?);
    std::fs::write(repo.path().join("MERGE_MSG"), &message).map_err(|e| GitCommandError::Internal(e.to_string()))?;

    if repo.index()?.has_conflicts() {
        return Ok(RevertResult { sha: None, conflicts: collect_conflicted_files(repo)? });
    }
    let sha = commit_revert(repo, &message, false)?;
    Ok(RevertResult { sha: Some(sha.to_string()), conflicts: Vec::new() })
}

/// Finishes a revert whose conflicts were resolved, with the message
/// `revert_commit` left in MERGE_MSG.
pub fn continue_revert(repo: &git2::Repository) -> Result<git2::Oid, GitCommandError> {
    if repo.index()?.has_conflicts() {
        return Err(GitCommandError::Internal("o revert ainda tem conflitos não resolvidos".into()));
    }
    let message = repo.message().unwrap_or_else(|_| "Revert".to_string());
    commit_revert(repo, &message, true)
}

/// Hooks like git's sequencer: prepare-commit-msg and post-commit. Finishing
/// after conflicts is a `git commit` instead (`continued`), which also runs
/// pre-commit and commit-msg. Signed when signing is configured.
fn commit_revert(repo: &git2::Repository, message: &str, continued: bool) -> Result<git2::Oid, GitCommandError> {
    if continued {
        hooks::pre_commit(repo, "pre-commit", false)?;
    }
    let tree = repo.find_tree(repo.index()?.write_tree()?)?;
    let head = repo.head()?.peel_to_commit()?;
    if tree.id() == head.tree_id() {
        // Already undone by a later commit: like `git revert`, refuse an empty commit.
        repo.cleanup_state()?;
        return Err(GitCommandError::Internal("nada para reverter: essas alterações já foram desfeitas".into()));
    }
    let sig = repo.signature()?;
    let message = hooks::prepare_message(repo, message, "message", !continued)?;
    let oid = signing::create_commit(repo, &sig, &sig, &message, &tree, &[&head])?;
    let subject = message.lines().next().unwrap_or_default();
    signing::advance_head(repo, oid, &format!("revert: {subject}"))?;
    repo.cleanup_state()?;
    hooks::run_post(repo, "post-commit", &[]);
    Ok(oid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn commit_file(repo: &git2::Repository, file: &str, content: &str, msg: &str) -> git2::Oid {
        std::fs::write(repo.workdir().unwrap().join(file), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(file)).unwrap();
        let tree_id = index.write_tree().unwrap();
        index.write().unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parents: Vec<_> = repo.head().ok().and_then(|h| h.target()).and_then(|oid| repo.find_commit(oid).ok()).into_iter().collect();
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parent_refs).unwrap()
    }

    /// Committed content, not the working copy: checkouts on a machine with
    /// core.autocrlf=true write CRLF to disk.
    fn head_blob(repo: &git2::Repository, file: &str) -> Option<String> {
        let tree = repo.head().unwrap().peel_to_tree().unwrap();
        let entry = tree.get_path(Path::new(file)).ok()?;
        let blob = repo.find_blob(entry.id()).unwrap();
        Some(String::from_utf8_lossy(blob.content()).into_owned())
    }

    /// Like `git revert`: prepare-commit-msg and post-commit run, commit-msg
    /// does not (the message isn't edited); a conflicted revert finished
    /// later is a `git commit`, so commit-msg runs then.
    #[cfg(unix)]
    #[test]
    fn runs_the_sequencer_hooks() {
        use crate::git::hooks::tests::write_hook;
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit_file(&repo, "a.txt", "one\n", "base");
        let target = commit_file(&repo, "a.txt", "one\ntwo\n", "add two");
        let hooks = repo.path().join("hooks");
        write_hook(&hooks, "prepare-commit-msg", "echo \"prepared: $2\" >> \"$1\"");
        write_hook(&hooks, "commit-msg", "echo 'checked' >> \"$1\"");
        let marker = tmp.path().join("post-commit-ran");
        write_hook(&hooks, "post-commit", &format!("touch '{}'", marker.display()));

        revert_commit(&repo, &target.to_string(), None).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        let message = head.message().unwrap();
        assert!(message.contains("prepared: message"), "{message}");
        assert!(!message.contains("checked"), "{message}");
        assert!(marker.exists());

        // Conflicted: "two" is changed again before reverting the commit that added it.
        let target = commit_file(&repo, "a.txt", "one\ntwo\n", "re-add two");
        commit_file(&repo, "a.txt", "one\nTWO\n", "change two");
        let result = revert_commit(&repo, &target.to_string(), None).unwrap();
        assert!(!result.conflicts.is_empty());
        std::fs::write(repo.workdir().unwrap().join("a.txt"), "one\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("a.txt")).unwrap();
        index.write().unwrap();
        continue_revert(&repo).unwrap();
        let message = repo.head().unwrap().peel_to_commit().unwrap().message().unwrap().to_string();
        assert!(message.contains("checked"), "{message}");
    }

    #[test]
    fn reverts_a_commit_with_gits_message() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit_file(&repo, "a.txt", "one\n", "base");
        let target = commit_file(&repo, "a.txt", "one\ntwo\n", "add two");
        commit_file(&repo, "b.txt", "other\n", "unrelated");

        let result = revert_commit(&repo, &target.to_string(), None).unwrap();
        assert!(result.conflicts.is_empty());
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(result.sha, Some(head.id().to_string()));
        assert_eq!(head.message().unwrap(), format!("Revert \"add two\"\n\nThis reverts commit {target}.\n"));
        assert_eq!(head_blob(&repo, "a.txt").as_deref(), Some("one\n"));
        assert_eq!(head_blob(&repo, "b.txt").as_deref(), Some("other\n"));
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
    }

    #[test]
    fn conflicting_revert_stops_in_revert_state_until_continued() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit_file(&repo, "a.txt", "one\n", "base");
        let target = commit_file(&repo, "a.txt", "two\n", "change to two");
        commit_file(&repo, "a.txt", "three\n", "change to three");

        let result = revert_commit(&repo, &target.to_string(), None).unwrap();
        assert_eq!(result.sha, None);
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].path, "a.txt");
        assert_eq!(repo.state(), git2::RepositoryState::Revert);

        // A second revert can't start on top of this one.
        assert!(revert_commit(&repo, &target.to_string(), None).is_err());

        super::super::rebase::resolve_conflict(&repo, "a.txt", super::super::rebase::ConflictResolution::UseManualContent("one\n".into()))
            .unwrap();
        continue_revert(&repo).unwrap();
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert!(head.message().unwrap().starts_with("Revert \"change to two\""));
        assert_eq!(head_blob(&repo, "a.txt").as_deref(), Some("one\n"));
    }

    #[test]
    fn merge_commits_need_a_mainline_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let base = commit_file(&repo, "a.txt", "one\n", "base");
        let main = repo.head().unwrap().shorthand().unwrap().to_string();
        repo.branch("feature", &repo.find_commit(base).unwrap(), false).unwrap();
        repo.set_head("refs/heads/feature").unwrap();
        commit_file(&repo, "b.txt", "feature\n", "feature work");
        repo.set_head(&format!("refs/heads/{main}")).unwrap();
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force())).unwrap();
        crate::git::merge::merge_branch(&repo, "feature").unwrap();
        let merge = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(merge.parent_count(), 2);

        assert!(revert_commit(&repo, &merge.id().to_string(), None).is_err());
        assert!(revert_commit(&repo, &merge.id().to_string(), Some(3)).is_err());

        let result = revert_commit(&repo, &merge.id().to_string(), Some(1)).unwrap();
        assert!(result.conflicts.is_empty());
        assert_eq!(head_blob(&repo, "b.txt"), None);
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert!(head.message().unwrap().ends_with(&format!(", reversing\nchanges made to {base}.\n")));
    }

    #[test]
    fn refuses_to_revert_with_staged_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit_file(&repo, "a.txt", "one\n", "base");
        let target = commit_file(&repo, "a.txt", "one\ntwo\n", "add two");
        std::fs::write(tmp.path().join("c.txt"), "staged\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("c.txt")).unwrap();
        index.write().unwrap();

        assert!(revert_commit(&repo, &target.to_string(), None).is_err());
        assert_eq!(repo.head().unwrap().target().unwrap(), target);
    }

    #[test]
    fn refuses_to_revert_over_unstaged_edits_but_ignores_untracked_files() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit_file(&repo, "a.txt", "one\n", "base");
        let target = commit_file(&repo, "b.txt", "b\n", "add b");
        // An unstaged edit an abort (hard reset to HEAD) would otherwise wipe.
        std::fs::write(tmp.path().join("a.txt"), "local edit\n").unwrap();
        assert!(revert_commit(&repo, &target.to_string(), None).is_err());
        assert_eq!(repo.head().unwrap().target().unwrap(), target);
        assert_eq!(std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(), "local edit\n");

        std::fs::write(tmp.path().join("a.txt"), "one\n").unwrap();
        std::fs::write(tmp.path().join("untracked.txt"), "keep me\n").unwrap();
        let result = revert_commit(&repo, &target.to_string(), None).unwrap();
        assert!(result.sha.is_some());
        assert_eq!(head_blob(&repo, "b.txt"), None);
        assert_eq!(std::fs::read_to_string(tmp.path().join("untracked.txt")).unwrap(), "keep me\n");
    }

    #[test]
    fn reverting_an_already_undone_change_is_refused_and_leaves_no_state() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit_file(&repo, "a.txt", "one\n", "base");
        let target = commit_file(&repo, "a.txt", "two\n", "to two");
        commit_file(&repo, "a.txt", "one\n", "back to one");
        let head = repo.head().unwrap().target().unwrap();

        assert!(revert_commit(&repo, &target.to_string(), None).is_err());
        assert_eq!(repo.head().unwrap().target().unwrap(), head);
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
    }

    /// Cross-checks against the real CLI: same tree and same message as
    /// `git revert --no-edit`.
    #[test]
    fn matches_git_cli_revert() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit_file(&repo, "a.txt", "one\n", "base");
        let target = commit_file(&repo, "a.txt", "one\ntwo\n", "add two");
        commit_file(&repo, "c.txt", "c\n", "more");
        let before = repo.head().unwrap().target().unwrap();

        let ours = revert_commit(&repo, &target.to_string(), None).unwrap().sha.unwrap();
        let ours = repo.find_commit(git2::Oid::from_str(&ours).unwrap()).unwrap();

        repo.reset(&repo.find_object(before, None).unwrap(), git2::ResetType::Hard, None).unwrap();
        let status = std::process::Command::new("git")
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.com", "revert", "--no-edit"])
            .arg(target.to_string())
            .current_dir(tmp.path())
            .output()
            .unwrap();
        assert!(status.status.success(), "{}", String::from_utf8_lossy(&status.stderr));
        let theirs = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(ours.tree_id(), theirs.tree_id());
        assert_eq!(ours.message(), theirs.message());
    }
}

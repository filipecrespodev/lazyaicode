use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::git_client::error::GitCommandError;
use crate::git_client::git::log::{to_commit_node, CommitNode};
use crate::git_client::git::{hooks, signing};

pub use super::diff::FileStatus;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub status: FileStatus,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingStatus {
    pub staged: Vec<FileEntry>,
    pub unstaged: Vec<FileEntry>,
    /// Changes when any listed file's status, size or mtime does: lets the UI
    /// re-read the (expensive, full-context) diffs only when something moved.
    pub signature: String,
}

/// Options for every working-tree scan the UI polls. libgit2's defaults
/// (`statuses(None)`) also walk *ignored* directories (node_modules, target,
/// build output...), which no caller wants and which dominates the scan time
/// on big repos — worst on Windows and on WSL's /mnt drives.
pub(crate) fn status_options() -> git2::StatusOptions {
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false);
    opts
}

pub fn working_status(repo: &git2::Repository) -> Result<WorkingStatus, GitCommandError> {
    use std::hash::{Hash, Hasher};
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let workdir = repo.workdir().map(Path::to_path_buf);

    for entry in repo.statuses(Some(&mut status_options()))?.iter() {
        let path = entry.path().unwrap_or_default().to_string();
        let s = entry.status();
        (&path, s.bits()).hash(&mut hasher);
        if let Some(meta) = workdir.as_ref().and_then(|w| std::fs::symlink_metadata(w.join(&path)).ok()) {
            let mtime = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok());
            (meta.len(), mtime.map(|d| d.as_nanos())).hash(&mut hasher);
        }
        if let Some(status) = index_status(s) {
            staged.push(FileEntry {
                path: path.clone(),
                status,
            });
        }
        if let Some(status) = workdir_status(s) {
            unstaged.push(FileEntry { path, status });
        }
    }

    Ok(WorkingStatus { staged, unstaged, signature: format!("{:016x}", hasher.finish()) })
}

fn index_status(s: git2::Status) -> Option<FileStatus> {
    if s.contains(git2::Status::INDEX_NEW) {
        Some(FileStatus::Added)
    } else if s.contains(git2::Status::INDEX_DELETED) {
        Some(FileStatus::Deleted)
    } else if s.contains(git2::Status::INDEX_RENAMED) {
        Some(FileStatus::Renamed)
    } else if s.contains(git2::Status::INDEX_TYPECHANGE) {
        Some(FileStatus::TypeChange)
    } else if s.contains(git2::Status::INDEX_MODIFIED) {
        Some(FileStatus::Modified)
    } else {
        None
    }
}

fn workdir_status(s: git2::Status) -> Option<FileStatus> {
    if s.contains(git2::Status::WT_NEW) {
        Some(FileStatus::Added)
    } else if s.contains(git2::Status::WT_DELETED) {
        Some(FileStatus::Deleted)
    } else if s.contains(git2::Status::WT_RENAMED) {
        Some(FileStatus::Renamed)
    } else if s.contains(git2::Status::WT_TYPECHANGE) {
        Some(FileStatus::TypeChange)
    } else if s.contains(git2::Status::WT_MODIFIED) {
        Some(FileStatus::Modified)
    } else {
        None
    }
}

pub fn stage_path(repo: &git2::Repository, path: &str) -> Result<(), GitCommandError> {
    let workdir = repo.workdir().ok_or_else(|| {
        GitCommandError::Internal("repositório bare não suporta staging".into())
    })?;
    let mut index = repo.index()?;
    if workdir.join(path).exists() {
        index.add_path(std::path::Path::new(path))?;
    } else {
        index.remove_path(std::path::Path::new(path))?;
    }
    index.write()?;
    Ok(())
}

pub fn stage_all(repo: &git2::Repository) -> Result<(), GitCommandError> {
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.update_all(["*"].iter(), None)?;
    index.write()?;
    Ok(())
}

pub fn unstage_path(repo: &git2::Repository, path: &str) -> Result<(), GitCommandError> {
    match repo.head() {
        Ok(head) => {
            let head = head.peel_to_commit()?;
            repo.reset_default(Some(head.as_object()), [path])?;
        }
        // Before the first commit everything staged is an addition, so
        // unstaging just drops the entry (`git rm --cached`).
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
            let mut index = repo.index()?;
            index.remove_path(std::path::Path::new(path))?;
            index.write()?;
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

/// Discards unstaged workdir changes to `path`, restoring it to match the
/// index (the same thing `git checkout -- <path>` does). A path with no
/// index entry at all is untracked, so there's nothing to restore it to —
/// it's simply deleted instead.
pub fn discard_path(repo: &git2::Repository, path: &str) -> Result<(), GitCommandError> {
    let workdir = repo.workdir().ok_or_else(|| {
        GitCommandError::Internal("repositório bare não suporta descartar alterações".into())
    })?;
    let mut index = repo.index()?;

    if index.get_path(std::path::Path::new(path), 0).is_none() {
        let full_path = workdir.join(path);
        if full_path.exists() {
            std::fs::remove_file(&full_path)
                .map_err(|e| GitCommandError::Internal(e.to_string()))?;
        }
        return Ok(());
    }

    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.path(path).force();
    repo.checkout_index(Some(&mut index), Some(&mut checkout))?;
    Ok(())
}

pub fn discard_all(repo: &git2::Repository) -> Result<(), GitCommandError> {
    for file in working_status(repo)?.unstaged {
        discard_path(repo, &file.path)?;
    }
    Ok(())
}

pub fn unstage_all(repo: &git2::Repository) -> Result<(), GitCommandError> {
    let head = repo.head()?.peel_to_commit()?;
    let status = working_status(repo)?;
    let paths: Vec<&str> = status.staged.iter().map(|f| f.path.as_str()).collect();
    if !paths.is_empty() {
        repo.reset_default(Some(head.as_object()), paths)?;
    }
    Ok(())
}

/// `git commit [--amend] [--no-verify] -m`: pre-commit, then the message
/// hooks, then the (possibly signed) commit, then post-commit.
pub fn commit(
    repo: &git2::Repository,
    summary: &str,
    body: &str,
    author_name: Option<&str>,
    author_email: Option<&str>,
    amend: bool,
    no_verify: bool,
) -> Result<String, GitCommandError> {
    let sig = match (author_name, author_email) {
        (Some(name), Some(email)) => git2::Signature::now(name, email)?,
        _ => repo.signature()?,
    };
    hooks::pre_commit(repo, "pre-commit", no_verify)?;
    let tree_id = repo.index()?.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    // An unborn HEAD (empty repo) makes this the root commit.
    let head = match repo.head() {
        Ok(head) => Some(head.peel_to_commit()?),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e.into()),
    };
    let (parents, reflog) = match (head, amend) {
        (Some(head), true) => (head.parents().collect::<Vec<_>>(), "commit (amend)"),
        (Some(head), false) => (vec![head], "commit"),
        (None, true) => return Err(GitCommandError::Internal("nada para fazer amend: ainda não há commits".into())),
        (None, false) => (Vec::new(), "commit (initial)"),
    };
    let message = hooks::prepare_message(repo, &join_message(summary, body), "message", no_verify)?;
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    let oid = signing::create_commit(repo, &sig, &sig, &message, &tree, &parent_refs)?;
    let subject = message.lines().next().unwrap_or_default();
    signing::advance_head(repo, oid, &format!("{reflog}: {subject}"))?;
    hooks::run_post(repo, "post-commit", &[]);
    Ok(oid.to_string())
}

fn join_message(summary: &str, body: &str) -> String {
    if body.trim().is_empty() {
        summary.to_string()
    } else {
        format!("{summary}\n\n{body}")
    }
}

/// Rewords a commit anywhere in the current branch's linear history, leaving
/// every commit's tree and author untouched. Every commit after it is
/// recreated on top of the reworded one with the exact same tree it already
/// had — content never changes, so this can never produce a conflict — and
/// the current branch ref (or HEAD, if detached) is moved to the new tip.
///
/// ponytail: only handles a linear chain between the target and HEAD; a merge
/// commit anywhere in that range is rejected rather than replayed, since
/// rewriting merge parents correctly needs real rebase/conflict handling.
pub fn amend_message(
    repo: &git2::Repository,
    sha: &str,
    summary: &str,
    body: &str,
    no_verify: bool,
) -> Result<CommitNode, GitCommandError> {
    let head_ref = repo.head()?;
    let update_ref_name = head_ref.name()?.to_string();

    let mut chain = vec![head_ref.peel_to_commit()?];
    while chain.last().unwrap().id().to_string() != sha {
        let cur = chain.last().unwrap();
        match cur.parent_count() {
            0 => {
                return Err(GitCommandError::Internal(
                    "o commit não faz parte do histórico da branch atual".into(),
                ));
            }
            1 => chain.push(cur.parent(0)?),
            _ => {
                return Err(GitCommandError::Internal(
                    "não é possível reescrever a mensagem antes de um commit de merge".into(),
                ));
            }
        }
    }
    chain.reverse(); // target first, HEAD last

    // Only the message changes, so no pre-commit (the tree is the same one
    // that already passed it); the message hooks still get their say.
    let new_message = hooks::prepare_message(repo, &join_message(summary, body), "message", no_verify)?;
    let signer = signing::signer_for(repo)?;

    let mut new_parent_oid = None;
    let mut reworded_node = None;
    let mut new_tip_oid = None;
    for (i, commit) in chain.iter().enumerate() {
        let message = if i == 0 {
            new_message.clone()
        } else {
            commit.message_raw().unwrap_or_default().to_string()
        };
        let parents: Vec<git2::Commit> = match new_parent_oid {
            Some(oid) => vec![repo.find_commit(oid)?],
            None => commit.parents().collect(),
        };
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        // No update_ref here: libgit2 requires the ref's current tip to equal
        // parents[0], which never holds once history is rewritten. The ref is
        // force-moved once, after the whole chain is rebuilt, below.
        let oid = signing::create_commit_with(
            repo,
            signer.as_ref(),
            &commit.author(),
            &commit.committer(),
            &message,
            &commit.tree()?,
            &parent_refs,
        )?;
        if i == 0 {
            reworded_node = Some(to_commit_node(&repo.find_commit(oid)?));
        }
        new_parent_oid = Some(oid);
        new_tip_oid = Some(oid);
    }
    let new_tip_oid = new_tip_oid.expect("chain always has at least one commit");

    if update_ref_name == "HEAD" {
        repo.set_head_detached(new_tip_oid)?;
    } else {
        repo.reference(
            &update_ref_name,
            new_tip_oid,
            true,
            "reword: rewrite history",
        )?;
    }
    hooks::run_post(repo, "post-commit", &[]);

    Ok(reworded_node.expect("chain always has at least one commit"))
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
            repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
                .unwrap();
        }
        repo
    }

    #[test]
    fn stages_unstages_and_commits() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo_with_commit(tmp.path());

        std::fs::write(tmp.path().join("a.txt"), "changed\n").unwrap();
        std::fs::write(tmp.path().join("b.txt"), "new\n").unwrap();

        let status = working_status(&repo).unwrap();
        assert_eq!(status.unstaged.len(), 2);
        assert_eq!(status.staged.len(), 0);

        stage_all(&repo).unwrap();
        let status = working_status(&repo).unwrap();
        assert_eq!(status.staged.len(), 2);
        assert_eq!(status.unstaged.len(), 0);

        unstage_path(&repo, "b.txt").unwrap();
        let status = working_status(&repo).unwrap();
        assert_eq!(status.staged.len(), 1);
        assert_eq!(status.unstaged.len(), 1);

        stage_path(&repo, "b.txt").unwrap();
        let sha = commit(&repo, "second commit", "", None, None, false, false).unwrap();
        assert!(!sha.is_empty());

        let status = working_status(&repo).unwrap();
        assert_eq!(status.staged.len(), 0);
        assert_eq!(status.unstaged.len(), 0);
    }

    #[test]
    fn status_skips_ignored_files_but_lists_untracked_ones() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo_with_commit(tmp.path());
        std::fs::write(tmp.path().join(".gitignore"), "node_modules/\n").unwrap();
        std::fs::create_dir_all(tmp.path().join("node_modules/pkg")).unwrap();
        std::fs::write(tmp.path().join("node_modules/pkg/index.js"), "x").unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/new.rs"), "y").unwrap();

        let status = working_status(&repo).unwrap();
        let paths: Vec<_> = status.unstaged.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"src/new.rs"), "{paths:?}");
        assert!(paths.contains(&".gitignore"), "{paths:?}");
        assert!(!paths.iter().any(|p| p.starts_with("node_modules")), "{paths:?}");
    }

    #[test]
    fn signature_moves_only_when_a_changed_file_does() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo_with_commit(tmp.path());
        let sig = |r: &git2::Repository| working_status(r).unwrap().signature;
        let clean = sig(&repo);
        assert_eq!(clean, sig(&repo));

        std::fs::write(tmp.path().join("a.txt"), "hello, edited\n").unwrap();
        let edited = sig(&repo);
        assert_ne!(clean, edited);
        assert_eq!(edited, sig(&repo));

        // Same status, new content (size differs): still a different signature.
        std::fs::write(tmp.path().join("a.txt"), "hello, edited again\n").unwrap();
        assert_ne!(edited, sig(&repo));
    }

    #[test]
    fn commit_uses_the_given_author_override_instead_of_repo_signature() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo_with_commit(tmp.path());

        std::fs::write(tmp.path().join("a.txt"), "changed\n").unwrap();
        stage_all(&repo).unwrap();
        let sha = commit(
            &repo,
            "profile commit",
            "",
            Some("Profile Name"),
            Some("profile@example.com"),
            false,
            false,
        )
        .unwrap();

        let oid = git2::Oid::from_str(&sha).unwrap();
        let found_commit = repo.find_commit(oid).unwrap();
        let author = found_commit.author();
        assert_eq!(author.name().unwrap(), "Profile Name");
        assert_eq!(author.email().unwrap(), "profile@example.com");
    }

    #[test]
    fn amend_replaces_head_without_adding_a_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo_with_commit(tmp.path());

        std::fs::write(tmp.path().join("a.txt"), "changed\n").unwrap();
        stage_all(&repo).unwrap();
        let sha = commit(&repo, "amended message", "", None, None, true, false).unwrap();

        let found = repo
            .find_commit(git2::Oid::from_str(&sha).unwrap())
            .unwrap();
        assert_eq!(found.summary().unwrap(), Some("amended message"));
        assert_eq!(found.parent_count(), 0);
    }

    #[test]
    fn amend_message_rejects_a_sha_not_in_current_history() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo_with_commit(tmp.path());
        let result = amend_message(
            &repo,
            "0000000000000000000000000000000000000000",
            "nope",
            "",
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn amend_message_rewords_an_older_commit_and_carries_descendants_along() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo_with_commit(tmp.path());
        let root_sha = repo
            .head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .id()
            .to_string();

        std::fs::write(tmp.path().join("b.txt"), "second\n").unwrap();
        stage_all(&repo).unwrap();
        commit(&repo, "second commit", "", None, None, false, false).unwrap();

        std::fs::write(tmp.path().join("c.txt"), "third\n").unwrap();
        stage_all(&repo).unwrap();
        let old_head_sha = commit(&repo, "third commit", "", None, None, false, false).unwrap();

        let reworded = amend_message(&repo, &root_sha, "reworded root", "", false).unwrap();
        assert_eq!(reworded.summary, "reworded root");
        assert_ne!(reworded.sha, root_sha);

        let new_head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_ne!(new_head.id().to_string(), old_head_sha);
        assert_eq!(new_head.summary().unwrap(), Some("third commit"));

        // Tree at HEAD is unchanged: all three files are still there.
        let tree = new_head.tree().unwrap();
        assert!(tree.get_name("a.txt").is_some());
        assert!(tree.get_name("b.txt").is_some());
        assert!(tree.get_name("c.txt").is_some());

        // Chain length is preserved, with the reworded commit as the new root.
        let parent = new_head.parent(0).unwrap();
        assert_eq!(parent.summary().unwrap(), Some("second commit"));
        let grandparent = parent.parent(0).unwrap();
        assert_eq!(grandparent.id().to_string(), reworded.sha);
        assert_eq!(grandparent.parent_count(), 0);
    }

    #[test]
    fn discards_a_modified_tracked_file_and_deletes_an_untracked_one() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo_with_commit(tmp.path());

        std::fs::write(tmp.path().join("a.txt"), "changed\n").unwrap();
        std::fs::write(tmp.path().join("b.txt"), "untracked\n").unwrap();

        discard_path(&repo, "a.txt").unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
            "hello\n"
        );

        discard_path(&repo, "b.txt").unwrap();
        assert!(!tmp.path().join("b.txt").exists());

        assert_eq!(working_status(&repo).unwrap().unstaged.len(), 0);
    }

    #[test]
    fn discard_all_clears_every_unstaged_change_but_leaves_staged_ones() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo_with_commit(tmp.path());

        std::fs::write(tmp.path().join("a.txt"), "changed\n").unwrap();
        std::fs::write(tmp.path().join("b.txt"), "untracked\n").unwrap();
        std::fs::write(tmp.path().join("c.txt"), "staged\n").unwrap();
        stage_path(&repo, "c.txt").unwrap();
        std::fs::write(tmp.path().join("c.txt"), "staged-then-changed\n").unwrap();

        discard_all(&repo).unwrap();

        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
            "hello\n"
        );
        assert!(!tmp.path().join("b.txt").exists());
        // c.txt was staged separately — discard_all only touches unstaged changes
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("c.txt")).unwrap(),
            "staged\n"
        );
        let status = working_status(&repo).unwrap();
        assert_eq!(status.unstaged.len(), 0);
        assert_eq!(status.staged.len(), 1);
    }

    #[cfg(unix)]
    mod hooks_and_signing {
        use super::*;
        use crate::git_client::git::hooks::tests::write_hook;
        use crate::git_client::git::signing::tests::{git_verifies, keygen};

        fn staged_change(repo: &git2::Repository) {
            std::fs::write(repo.workdir().unwrap().join("a.txt"), "changed\n").unwrap();
            stage_all(repo).unwrap();
        }

        fn head_summary(repo: &git2::Repository) -> String {
            repo.head().unwrap().peel_to_commit().unwrap().summary().unwrap().unwrap().to_string()
        }

        #[test]
        fn runs_commit_hooks_in_git_order() {
            let tmp = tempfile::tempdir().unwrap();
            let repo = init_repo_with_commit(tmp.path());
            let hooks = repo.path().join("hooks");
            let log = tmp.path().join("order.log");
            for name in ["pre-commit", "prepare-commit-msg", "commit-msg", "post-commit"] {
                write_hook(&hooks, name, &format!("echo {name} >> '{}'", log.display()));
            }
            staged_change(&repo);
            commit(&repo, "ordered", "", None, None, false, false).unwrap();
            assert_eq!(
                std::fs::read_to_string(&log).unwrap(),
                "pre-commit\nprepare-commit-msg\ncommit-msg\npost-commit\n"
            );
        }

        #[test]
        fn failing_pre_commit_aborts_and_leaves_the_index_intact() {
            let tmp = tempfile::tempdir().unwrap();
            let repo = init_repo_with_commit(tmp.path());
            write_hook(&repo.path().join("hooks"), "pre-commit", "echo 'lint failed'; exit 1");
            staged_change(&repo);
            let head_before = repo.head().unwrap().target();

            match commit(&repo, "blocked", "", None, None, false, false) {
                Err(GitCommandError::HookFailed { hook, output }) => {
                    assert_eq!(hook, "pre-commit");
                    assert_eq!(output, "lint failed");
                }
                other => panic!("expected HookFailed, got {other:?}"),
            }
            assert_eq!(repo.head().unwrap().target(), head_before);
            let status = working_status(&repo).unwrap();
            assert_eq!(status.staged.len(), 1);
            assert_eq!(status.staged[0].path, "a.txt");
        }

        #[test]
        fn no_verify_skips_pre_commit_and_commit_msg() {
            let tmp = tempfile::tempdir().unwrap();
            let repo = init_repo_with_commit(tmp.path());
            let hooks = repo.path().join("hooks");
            write_hook(&hooks, "pre-commit", "exit 1");
            write_hook(&hooks, "commit-msg", "exit 1");
            staged_change(&repo);
            commit(&repo, "skipped", "", None, None, false, true).unwrap();
            assert_eq!(head_summary(&repo), "skipped");
        }

        #[test]
        fn commit_msg_hook_can_rewrite_the_message() {
            let tmp = tempfile::tempdir().unwrap();
            let repo = init_repo_with_commit(tmp.path());
            write_hook(&repo.path().join("hooks"), "commit-msg", "printf 'feat: rewritten\\n\\nby hook\\n' > \"$1\"");
            staged_change(&repo);
            commit(&repo, "original", "", None, None, false, false).unwrap();
            let head = repo.head().unwrap().peel_to_commit().unwrap();
            assert_eq!(head.message().unwrap(), "feat: rewritten\n\nby hook");
        }

        #[test]
        fn pre_commit_restaging_is_committed() {
            // lint-staged style: the hook fixes a file and re-stages it.
            let tmp = tempfile::tempdir().unwrap();
            let repo = init_repo_with_commit(tmp.path());
            write_hook(
                &repo.path().join("hooks"),
                "pre-commit",
                "echo formatted > a.txt && git add a.txt",
            );
            staged_change(&repo);
            let sha = commit(&repo, "fmt", "", None, None, false, false).unwrap();
            let tree = repo.find_commit(git2::Oid::from_str(&sha).unwrap()).unwrap().tree().unwrap();
            let blob = repo.find_blob(tree.get_name("a.txt").unwrap().id()).unwrap();
            assert_eq!(blob.content(), b"formatted\n");
        }

        #[test]
        fn honours_core_hooks_path() {
            let tmp = tempfile::tempdir().unwrap();
            let repo = init_repo_with_commit(tmp.path());
            repo.config().unwrap().set_str("core.hooksPath", ".husky").unwrap();
            write_hook(&tmp.path().join(".husky"), "pre-commit", "exit 1");
            write_hook(&repo.path().join("hooks"), "pre-commit", "exit 0"); // ignored now
            staged_change(&repo);
            assert!(matches!(
                commit(&repo, "x", "", None, None, false, false),
                Err(GitCommandError::HookFailed { .. })
            ));
        }

        #[test]
        fn reword_runs_message_hooks() {
            let tmp = tempfile::tempdir().unwrap();
            let repo = init_repo_with_commit(tmp.path());
            let sha = repo.head().unwrap().target().unwrap().to_string();
            write_hook(&repo.path().join("hooks"), "commit-msg", "printf '\\nRefs: #1\\n' >> \"$1\"");
            let node = amend_message(&repo, &sha, "reworded", "", false).unwrap();
            assert_eq!(node.body, "Refs: #1");
            write_hook(&repo.path().join("hooks"), "commit-msg", "exit 1");
            assert!(amend_message(&repo, &node.sha, "again", "", false).is_err());
            amend_message(&repo, &node.sha, "again", "", true).unwrap();
        }

        #[test]
        fn first_commit_of_an_empty_repo_runs_hooks() {
            let tmp = tempfile::tempdir().unwrap();
            let repo = git2::Repository::init(tmp.path()).unwrap();
            write_hook(&repo.path().join("hooks"), "commit-msg", "echo 'Refs: #1' >> \"$1\"");
            std::fs::write(tmp.path().join("a.txt"), "a\n").unwrap();
            stage_all(&repo).unwrap();
            assert!(commit(&repo, "x", "", None, None, true, false).is_err(), "nothing to amend");
            let sha = commit(&repo, "root", "", None, None, false, false).unwrap();
            let head = repo.head().unwrap().peel_to_commit().unwrap();
            assert_eq!(head.id().to_string(), sha);
            assert_eq!(head.parent_count(), 0);
            assert_eq!(head.message().unwrap(), "root\nRefs: #1");
        }

        #[test]
        fn compose_runs_pre_commit_once_and_message_hooks_per_commit() {
            let tmp = tempfile::tempdir().unwrap();
            let repo = init_repo_with_commit(tmp.path());
            let hooks = repo.path().join("hooks");
            let log = tmp.path().join("hooks.log");
            write_hook(&hooks, "pre-commit", &format!("echo pre-commit >> '{}'", log.display()));
            write_hook(&hooks, "commit-msg", "echo 'Refs: #1' >> \"$1\"");
            std::fs::write(tmp.path().join("b.txt"), "b\n").unwrap();
            std::fs::write(tmp.path().join("c.txt"), "c\n").unwrap();
            stage_all(&repo).unwrap();
            let staged = crate::git_client::git::compose::staged_units(&repo).unwrap();
            let input = |summary: &str, path: &str| crate::git_client::git::compose::ComposedCommitInput {
                summary: summary.into(),
                description: String::new(),
                changes: vec![path.into()],
            };
            let plan = [input("one", "b.txt"), input("two", "c.txt")];
            crate::git_client::git::compose::apply(&repo, &staged.tree_id, &plan, None, None, false).unwrap();
            let head = repo.head().unwrap().peel_to_commit().unwrap();
            assert_eq!(head.message().unwrap(), "two\nRefs: #1");
            assert_eq!(head.parent(0).unwrap().message().unwrap(), "one\nRefs: #1");
            assert_eq!(std::fs::read_to_string(&log).unwrap(), "pre-commit\n");
        }

        #[test]
        fn compose_aborts_on_a_failing_pre_commit_unless_skipped() {
            let tmp = tempfile::tempdir().unwrap();
            let repo = init_repo_with_commit(tmp.path());
            write_hook(&repo.path().join("hooks"), "pre-commit", "exit 1");
            std::fs::write(tmp.path().join("b.txt"), "b\n").unwrap();
            stage_all(&repo).unwrap();
            let head_before = repo.head().unwrap().target();
            let staged = crate::git_client::git::compose::staged_units(&repo).unwrap();
            let plan = [crate::git_client::git::compose::ComposedCommitInput {
                summary: "one".into(),
                description: String::new(),
                changes: vec!["b.txt".into()],
            }];
            let blocked = crate::git_client::git::compose::apply(&repo, &staged.tree_id, &plan, None, None, false);
            assert!(matches!(blocked, Err(GitCommandError::HookFailed { .. })));
            assert_eq!(repo.head().unwrap().target(), head_before);
            crate::git_client::git::compose::apply(&repo, &staged.tree_id, &plan, None, None, true).unwrap();
            assert_eq!(head_summary(&repo), "one");
        }

        #[test]
        fn cherry_pick_runs_prepare_commit_msg_but_not_commit_msg() {
            let tmp = tempfile::tempdir().unwrap();
            let mut repo = init_repo_with_commit(tmp.path());
            let base = repo.head().unwrap().target().unwrap();
            staged_change(&repo);
            let picked = commit(&repo, "picked", "", None, None, false, false).unwrap();
            let base_obj = repo.find_object(base, None).unwrap();
            repo.reset(&base_obj, git2::ResetType::Hard, None).unwrap();
            drop(base_obj);
            let hooks = repo.path().join("hooks");
            write_hook(&hooks, "prepare-commit-msg", "echo 'Refs: #2' >> \"$1\"");
            write_hook(&hooks, "commit-msg", "exit 1");
            crate::git_client::git::cherry_pick::cherry_pick(&mut repo, &picked, true).unwrap();
            let head = repo.head().unwrap().peel_to_commit().unwrap();
            assert_eq!(head.message().unwrap(), "picked\nRefs: #2");
            assert_eq!(head.parent_id(0).unwrap(), base);
            assert_eq!(repo.state(), git2::RepositoryState::Clean);
        }

        #[test]
        fn signed_commit_and_amend_verify_with_git() {
            let tmp = tempfile::tempdir().unwrap();
            let Some(key) = keygen(tmp.path(), "id", "") else { return };
            let repo_dir = tmp.path().join("repo");
            let repo = init_repo_with_commit(&repo_dir);
            let mut config = repo.config().unwrap();
            config.set_bool("commit.gpgsign", true).unwrap();
            config.set_str("gpg.format", "ssh").unwrap();
            config.set_str("user.signingkey", &key.display().to_string()).unwrap();
            config.set_str("user.name", "T").unwrap();
            config.set_str("user.email", "t@example.com").unwrap();

            staged_change(&repo);
            let sha = commit(&repo, "signed", "", None, None, false, false).unwrap();
            assert_eq!(repo.head().unwrap().target().unwrap().to_string(), sha);
            let amended = commit(&repo, "signed amend", "", None, None, true, false).unwrap();
            let head = repo.head().unwrap().peel_to_commit().unwrap();
            assert_eq!(head.id().to_string(), amended);
            assert_eq!(head.parent(0).unwrap().summary().unwrap(), Some("initial"));
            for sha in [&sha, &amended] {
                assert!(repo.extract_signature(&git2::Oid::from_str(sha).unwrap(), None).is_ok());
                if let Some(ok) = git_verifies(&repo_dir, &key, sha) {
                    assert!(ok, "git verify-commit rejected {sha}");
                }
            }
        }
    }
}

//! Linked worktrees (`git worktree list/add/remove/prune`), all through libgit2.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::git_client::error::GitCommandError;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    /// Name under `.git/worktrees`; `None` for the main worktree.
    pub name: Option<String>,
    pub path: String,
    /// `None` when detached (or unreadable).
    pub branch: Option<String>,
    pub head_sha: Option<String>,
    pub is_main: bool,
    /// The worktree this repo was opened from.
    pub is_current: bool,
    pub locked: bool,
    /// Its directory is gone: `prune` would drop it.
    pub prunable: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddWorktreeArgs {
    pub path: String,
    pub branch: String,
    /// Create `branch` (at `start_point`, or HEAD) instead of checking out an existing one.
    pub new_branch: bool,
    pub start_point: Option<String>,
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Branch and HEAD of a checkout, read without failing the whole listing.
fn head_of(repo: &git2::Repository) -> (Option<String>, Option<String>) {
    let Ok(head) = repo.head() else {
        return (None, None);
    };
    let branch = head.is_branch().then(|| head.shorthand().ok().map(str::to_string)).flatten();
    (branch, head.target().map(|o| o.to_string()))
}

/// Working directory of the main worktree, also when `repo` is a linked one.
fn main_workdir(repo: &git2::Repository) -> Option<PathBuf> {
    if repo.is_worktree() {
        // commondir is the main checkout's `.git`
        repo.commondir().parent().map(Path::to_path_buf)
    } else {
        repo.workdir().map(Path::to_path_buf)
    }
}

pub fn list(repo: &git2::Repository) -> Result<Vec<WorktreeInfo>, GitCommandError> {
    let current = repo.workdir().map(Path::to_path_buf);
    let is_current = |p: &Path| current.as_deref().is_some_and(|c| same_dir(c, p));
    let mut out = Vec::new();

    if let Some(main) = main_workdir(repo) {
        let (branch, head_sha) = if is_current(&main) {
            head_of(repo)
        } else {
            git2::Repository::open(&main).map(|r| head_of(&r)).unwrap_or((None, None))
        };
        out.push(WorktreeInfo {
            name: None,
            path: main.display().to_string(),
            branch,
            head_sha,
            is_main: true,
            is_current: is_current(&main),
            locked: false,
            prunable: false,
        });
    }

    for name in repo.worktrees()?.iter().flatten().flatten() {
        let wt = repo.find_worktree(name)?;
        let path = wt.path().to_path_buf();
        let prunable = wt.validate().is_err();
        let (branch, head_sha) = if prunable {
            (None, None)
        } else {
            git2::Repository::open_from_worktree(&wt).map(|r| head_of(&r)).unwrap_or((None, None))
        };
        out.push(WorktreeInfo {
            name: Some(name.to_string()),
            is_current: is_current(&path),
            path: path.display().to_string(),
            branch,
            head_sha,
            is_main: false,
            locked: matches!(wt.is_locked(), Ok(git2::WorktreeLockStatus::Locked(_))),
            prunable,
        });
    }
    Ok(out)
}

/// `git worktree add [-b <branch>] <path> [<branch|start>]`.
pub fn add(repo: &git2::Repository, args: &AddWorktreeArgs) -> Result<(), GitCommandError> {
    let path = Path::new(&args.path);
    // libgit2 would resolve a relative path against the app's own cwd.
    if !path.is_absolute() {
        return Err(GitCommandError::Internal(format!("{} não é um caminho absoluto", args.path)));
    }
    if path.exists() && path.read_dir().map_or(true, |mut d| d.next().is_some()) {
        return Err(GitCommandError::Internal(format!("{} já existe e não está vazio", args.path)));
    }
    let branch = if args.new_branch {
        let start = match &args.start_point {
            Some(s) => repo.revparse_single(s)?.peel_to_commit()?,
            None => repo.head()?.peel_to_commit()?,
        };
        repo.branch(&args.branch, &start, false)?
    } else {
        repo.find_branch(&args.branch, git2::BranchType::Local)?
    };

    let mut opts = git2::WorktreeAddOptions::new();
    opts.reference(Some(branch.get()));
    // libgit2 refuses a branch already checked out in another worktree, like git.
    if let Err(e) = repo.worktree(&unique_name(repo, path), path, Some(&opts)) {
        // Don't leave behind the branch made just for this worktree.
        if args.new_branch {
            let mut branch = branch;
            let _ = branch.delete();
        }
        return Err(e.into());
    }
    Ok(())
}

/// Admin dir name: the folder's name, suffixed when taken (like git does).
fn unique_name(repo: &git2::Repository, path: &Path) -> String {
    let base = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "worktree".into());
    let admin = repo.commondir().join("worktrees");
    let mut name = base.clone();
    let mut n = 1;
    while admin.join(&name).exists() {
        name = format!("{base}{n}");
        n += 1;
    }
    name
}

/// `git worktree remove [--force]`: deletes its directory. Without `force`
/// it refuses a locked worktree or one with uncommitted changes.
pub fn remove(repo: &git2::Repository, name: &str, force: bool) -> Result<(), GitCommandError> {
    let wt = repo.find_worktree(name)?;
    // Deleting the checkout this repo was opened from would pull the folder
    // out from under the open tab.
    if repo.workdir().is_some_and(|w| same_dir(w, wt.path())) {
        return Err(GitCommandError::Internal(format!(
            "o worktree {name} é o que está aberto aqui — remova-o a partir de outro worktree"
        )));
    }
    if !force {
        if let Ok(git2::WorktreeLockStatus::Locked(_)) = wt.is_locked() {
            return Err(GitCommandError::Internal(format!("o worktree {name} está bloqueado")));
        }
        if let Ok(checkout) = git2::Repository::open_from_worktree(&wt) {
            let mut opts = git2::StatusOptions::new();
            opts.include_untracked(true).include_ignored(false);
            if !checkout.statuses(Some(&mut opts))?.is_empty() {
                return Err(GitCommandError::Internal(format!(
                    "o worktree {name} tem alterações não commitadas"
                )));
            }
        }
    }
    let mut opts = git2::WorktreePruneOptions::new();
    opts.valid(true).locked(force).working_tree(true);
    wt.prune(Some(&mut opts))?;
    Ok(())
}

/// `git worktree prune`: forgets worktrees whose directory is gone. Returns how many.
pub fn prune(repo: &git2::Repository) -> Result<u32, GitCommandError> {
    let mut pruned = 0;
    for name in repo.worktrees()?.iter().flatten().flatten() {
        let wt = repo.find_worktree(name)?;
        if wt.is_prunable(None)? {
            wt.prune(None)?;
            pruned += 1;
        }
    }
    Ok(pruned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_with_commit() -> (tempfile::TempDir, git2::Repository) {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        {
            std::fs::write(dir.path().join("a.txt"), "a").unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("a.txt")).unwrap();
            index.write().unwrap();
            let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
            let sig = git2::Signature::now("Test", "test@example.com").unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();
        }
        (dir, repo)
    }

    fn add_args(path: &Path, branch: &str, new_branch: bool) -> AddWorktreeArgs {
        AddWorktreeArgs {
            path: path.display().to_string(),
            branch: branch.into(),
            new_branch,
            start_point: None,
        }
    }

    #[test]
    fn adds_lists_and_removes_worktrees() {
        let (dir, repo) = repo_with_commit();
        let outside = tempfile::tempdir().unwrap();
        let wt_path = outside.path().join("feature-wt");

        let only_main = list(&repo).unwrap();
        assert_eq!(only_main.len(), 1);
        assert!(only_main[0].is_main && only_main[0].is_current);

        add(&repo, &add_args(&wt_path, "feature", true)).unwrap();
        assert!(wt_path.join("a.txt").exists(), "the new worktree is checked out");

        let all = list(&repo).unwrap();
        assert_eq!(all.len(), 2);
        let linked = &all[1];
        assert_eq!(linked.name.as_deref(), Some("feature-wt"));
        assert_eq!(linked.branch.as_deref(), Some("feature"));
        assert!(!linked.is_main && !linked.is_current && !linked.prunable);
        assert_eq!(linked.head_sha, all[0].head_sha);

        // the same branch can't be checked out twice
        let again = add(&repo, &add_args(&outside.path().join("other"), "feature", false));
        assert!(again.is_err());

        // dirty worktrees need force
        std::fs::write(wt_path.join("new.txt"), "x").unwrap();
        assert!(remove(&repo, "feature-wt", false).is_err());
        remove(&repo, "feature-wt", true).unwrap();
        assert!(!wt_path.exists());
        assert_eq!(list(&repo).unwrap().len(), 1);
        // the branch stays
        assert!(repo.find_branch("feature", git2::BranchType::Local).is_ok());
        drop(dir);
    }

    #[test]
    fn a_failed_add_leaves_no_new_branch_behind() {
        let (_dir, repo) = repo_with_commit();
        let outside = tempfile::tempdir().unwrap();
        let taken = outside.path().join("taken");
        std::fs::create_dir(&taken).unwrap();
        std::fs::write(taken.join("x"), "x").unwrap();
        assert!(add(&repo, &add_args(&taken, "fresh", true)).is_err());
        assert!(add(&repo, &add_args(Path::new("relative/wt"), "fresh", true)).is_err());
        // a path libgit2 itself can't create (a file where a folder should be)
        let blocked = taken.join("x").join("wt");
        assert!(add(&repo, &add_args(&blocked, "fresh", true)).is_err());
        assert!(repo.find_branch("fresh", git2::BranchType::Local).is_err());
    }

    #[test]
    fn refuses_to_remove_the_worktree_it_was_opened_from() {
        let (_dir, repo) = repo_with_commit();
        let outside = tempfile::tempdir().unwrap();
        let wt_path = outside.path().join("self");
        add(&repo, &add_args(&wt_path, "self", true)).unwrap();
        let linked = git2::Repository::open(&wt_path).unwrap();
        assert!(remove(&linked, "self", true).is_err());
        assert!(wt_path.exists());
        remove(&repo, "self", false).unwrap();
        assert!(!wt_path.exists());
    }

    #[test]
    fn lists_from_inside_a_linked_worktree() {
        let (dir, repo) = repo_with_commit();
        let outside = tempfile::tempdir().unwrap();
        let wt_path = outside.path().join("linked");
        add(&repo, &add_args(&wt_path, "linked", true)).unwrap();

        let linked_repo = git2::Repository::open(&wt_path).unwrap();
        let all = list(&linked_repo).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all[0].is_main && !all[0].is_current);
        assert!(same_dir(Path::new(&all[0].path), dir.path()));
        assert!(all[1].is_current);
    }

    #[test]
    fn prunes_worktrees_whose_directory_is_gone() {
        let (_dir, repo) = repo_with_commit();
        let outside = tempfile::tempdir().unwrap();
        let wt_path = outside.path().join("gone");
        add(&repo, &add_args(&wt_path, "gone", true)).unwrap();
        std::fs::remove_dir_all(&wt_path).unwrap();

        assert!(list(&repo).unwrap()[1].prunable);
        assert_eq!(prune(&repo).unwrap(), 1);
        assert_eq!(list(&repo).unwrap().len(), 1);
    }

    #[test]
    fn checks_out_an_existing_branch_with_a_unique_admin_name() {
        let (_dir, repo) = repo_with_commit();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("one", &head, false).unwrap();
        repo.branch("two", &head, false).unwrap();
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        // same folder name in two places
        add(&repo, &add_args(&a.path().join("wt"), "one", false)).unwrap();
        add(&repo, &add_args(&b.path().join("wt"), "two", false)).unwrap();
        let names: Vec<_> = list(&repo).unwrap().into_iter().filter_map(|w| w.name).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"wt".to_string()) && names.contains(&"wt1".to_string()));
    }
}

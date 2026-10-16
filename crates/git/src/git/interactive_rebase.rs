//! Interactive rebase (`git rebase -i`), which libgit2 can't start: the app
//! prepares the todo list itself and runs the real `git` binary with a
//! sequence editor that just copies that list over git's own.
//!
//! Rewording doesn't go through `reword` (that would need an editor answering
//! a different message per commit): a reworded commit is a plain `pick`
//! followed by `exec git commit --amend -F <file>`, so the message lands even
//! when the pick stops on a conflict first — the `exec` runs after `--continue`.
//! Squash keeps git's combined message (GIT_EDITOR is `true`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::external_git;
use super::log::{to_commit_node, CommitNode};
use super::rebase::{self, RebaseState, RebaseStatus};
use crate::error::GitCommandError;

/// Past this, the modal would be unusable and the walk wasteful.
const MAX_COMMITS: usize = 500;
/// Holds the todo and reword messages; `exec` lines read them until the
/// rebase is over, so it lives in the git dir (survives app restarts).
const STATE_DIR: &str = "githydra-rebase-i";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TodoAction {
    Pick,
    Reword,
    Squash,
    Fixup,
    Drop,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub sha: String,
    pub action: TodoAction,
    /// New message for `reword`.
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebasePlan {
    /// Commit the rebase replays onto; `None` rewrites from the root commit.
    pub upstream: Option<String>,
    /// Oldest first, like git's todo list.
    pub commits: Vec<CommitNode>,
}

/// Commits `git rebase -i <from>^` would list: `from` itself and every
/// non-merge commit after it up to HEAD, oldest first.
pub fn plan(repo: &git2::Repository, from_sha: &str) -> Result<RebasePlan, GitCommandError> {
    let from = repo.find_commit(git2::Oid::from_str(from_sha)?)?;
    let head = repo.head()?.peel_to_commit()?;
    if head.id() != from.id() && !repo.graph_descendant_of(head.id(), from.id())? {
        return Err(GitCommandError::Internal("esse commit não faz parte da branch em checkout".into()));
    }
    let upstream = from.parent_id(0).ok();
    let commits = commits_to_rebase(repo, head.id(), upstream)?.iter().map(to_commit_node).collect();
    Ok(RebasePlan { upstream: upstream.map(|o| o.to_string()), commits })
}

/// Non-merge commits in `upstream..head` (everything for `None`), oldest first.
fn commits_to_rebase(
    repo: &git2::Repository,
    head: git2::Oid,
    upstream: Option<git2::Oid>,
) -> Result<Vec<git2::Commit<'_>>, GitCommandError> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)?;
    walk.push(head)?;
    if let Some(up) = upstream {
        walk.hide(up)?;
    }
    let mut commits = Vec::new();
    for oid in walk {
        let commit = repo.find_commit(oid?)?;
        if commit.parent_count() > 1 {
            continue; // like git without --rebase-merges
        }
        if commits.len() == MAX_COMMITS {
            return Err(GitCommandError::Internal(format!("mais de {MAX_COMMITS} commits para o rebase — escolha um commit mais recente")));
        }
        commits.push(commit);
    }
    Ok(commits)
}

/// The list must name exactly the commits git would rewrite now: a line
/// missing from the todo is a dropped commit, so a list made before HEAD
/// moved (a commit, pull or reset since the modal opened) would silently
/// lose the new commits or resurrect removed ones.
fn check_matches_branch(repo: &git2::Repository, upstream: Option<&str>, items: &[TodoItem]) -> Result<(), GitCommandError> {
    let head = repo.head()?.peel_to_commit()?.id();
    let upstream = upstream.map(git2::Oid::from_str).transpose()?;
    let mut expected: Vec<String> = commits_to_rebase(repo, head, upstream)?.iter().map(|c| c.id().to_string()).collect();
    let mut given: Vec<String> = items.iter().map(|i| i.sha.clone()).collect();
    expected.sort_unstable();
    given.sort_unstable();
    if expected != given {
        return Err(GitCommandError::Internal("a branch mudou desde que esta lista foi montada — reabra o rebase interativo".into()));
    }
    Ok(())
}

/// Same rules the modal shows before running; checked again here since a
/// bad list would leave git stopped half-way.
pub fn validate(items: &[TodoItem]) -> Result<(), String> {
    let mut kept = items.iter().filter(|i| i.action != TodoAction::Drop);
    match kept.next() {
        None => return Err("todos os commits foram descartados — não sobraria nada para o rebase".into()),
        Some(first) if matches!(first.action, TodoAction::Squash | TodoAction::Fixup) => {
            return Err("o primeiro commit não pode receber squash nem fixup: não há nada antes dele para combinar".into())
        }
        _ => {}
    }
    for item in items {
        if item.action == TodoAction::Reword && item.message.as_deref().map_or(true, |m| m.trim().is_empty()) {
            return Err(format!("o commit {} precisa de uma mensagem para o reword", short(&item.sha)));
        }
    }
    Ok(())
}

/// Text of the todo list. `message_file(i)` names the file holding item
/// `i`'s new message (written by the caller).
pub fn todo_text(items: &[TodoItem], summaries: &dyn Fn(&str) -> String, message_file: &dyn Fn(usize) -> PathBuf) -> String {
    let mut out = String::new();
    // A reword's amend waits until its fixups/squashes are melded in.
    let mut pending_amend: Option<usize> = None;
    for (i, item) in items.iter().enumerate() {
        let verb = match item.action {
            TodoAction::Pick | TodoAction::Reword => "pick",
            TodoAction::Squash => "squash",
            TodoAction::Fixup => "fixup",
            TodoAction::Drop => "drop",
        };
        if matches!(item.action, TodoAction::Pick | TodoAction::Reword) {
            if let Some(j) = pending_amend.take() {
                out.push_str(&amend_line(&message_file(j)));
            }
        }
        out.push_str(&format!("{verb} {} {}\n", item.sha, summaries(&item.sha)));
        if item.action == TodoAction::Reword {
            pending_amend = Some(i);
        }
    }
    if let Some(j) = pending_amend {
        out.push_str(&amend_line(&message_file(j)));
    }
    out
}

fn amend_line(message_file: &Path) -> String {
    format!("exec git commit --amend --allow-empty --no-verify --cleanup=whitespace -F {}\n", sh_quote(message_file))
}

/// Single-quotes a path for the `sh` git runs editors and `exec` lines with
/// (Git for Windows ships one too). Forward slashes keep it valid there.
pub fn sh_quote(path: &Path) -> String {
    let s = path.display().to_string();
    let s = if cfg!(windows) { s.replace('\\', "/") } else { s };
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

fn io(e: std::io::Error) -> GitCommandError {
    GitCommandError::Internal(e.to_string())
}

/// Starts the rebase. Stops on conflicts (or a failed `exec`) are reported
/// like any other rebase in progress, for the conflict panel to take over.
pub fn start(repo: &git2::Repository, upstream: Option<&str>, items: &[TodoItem]) -> Result<RebaseStatus, GitCommandError> {
    let env = super::signing::git_cli_env(repo)?;
    start_with_env(repo, upstream, items, &env)
}

fn start_with_env(
    repo: &git2::Repository,
    upstream: Option<&str>,
    items: &[TodoItem],
    env: &[(String, String)],
) -> Result<RebaseStatus, GitCommandError> {
    if rebase::is_rebase_in_progress(repo) {
        return Err(GitCommandError::Internal("já há um rebase em andamento neste repositório".into()));
    }
    validate(items).map_err(GitCommandError::Internal)?;
    check_matches_branch(repo, upstream, items)?;
    let workdir = repo.workdir().ok_or_else(|| GitCommandError::Internal("repositório bare não tem diretório de trabalho".into()))?;

    let dir = repo.path().join(STATE_DIR);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(io)?;
    let message_file = |i: usize| dir.join(format!("message-{i}"));
    for (i, item) in items.iter().enumerate() {
        if let (TodoAction::Reword, Some(message)) = (item.action, &item.message) {
            std::fs::write(message_file(i), message).map_err(io)?;
        }
    }
    let summaries = |sha: &str| {
        git2::Oid::from_str(sha)
            .and_then(|oid| repo.find_commit(oid))
            .map(|c| c.summary().ok().flatten().unwrap_or("").to_string())
            .unwrap_or_default()
    };
    let todo = dir.join("git-rebase-todo");
    std::fs::write(&todo, todo_text(items, &summaries, &message_file)).map_err(io)?;

    let result = external_git::rebase_interactive(workdir, upstream, &todo, env);
    after_git(repo);
    if rebase::is_rebase_in_progress(repo) {
        return Ok(rebase::read_status(repo)?.unwrap_or(stopped()));
    }
    result?;
    Ok(RebaseStatus {
        state: RebaseState::Complete,
        current_step: items.len() as u32,
        total_steps: items.len() as u32,
        conflicts: Vec::new(),
    })
}

/// A stop with nothing conflicted (e.g. a failed `exec`): still "in
/// progress" so the panel offers Continue/Abort.
fn stopped() -> RebaseStatus {
    RebaseStatus { state: RebaseState::Conflicts, current_step: 0, total_steps: 0, conflicts: Vec::new() }
}

/// Call after running the git CLI on `repo`: reloads the index it may have
/// rewritten (libgit2 keeps the one it read, so conflicts would go unseen)
/// and drops the todo/message files once no rebase is running anymore.
pub fn after_git(repo: &git2::Repository) {
    if let Ok(mut index) = repo.index() {
        let _ = index.read(false);
    }
    if !rebase::is_rebase_in_progress(repo) {
        let _ = std::fs::remove_dir_all(repo.path().join(STATE_DIR));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn item(sha: &str, action: TodoAction) -> TodoItem {
        TodoItem { sha: sha.into(), action, message: None }
    }

    #[test]
    fn validation_rules() {
        assert!(validate(&[item("a", TodoAction::Pick), item("b", TodoAction::Squash)]).is_ok());
        assert!(validate(&[item("a", TodoAction::Fixup), item("b", TodoAction::Pick)]).is_err());
        // a dropped first commit doesn't count as "before" the squash
        assert!(validate(&[item("a", TodoAction::Drop), item("b", TodoAction::Squash)]).is_err());
        assert!(validate(&[item("a", TodoAction::Drop)]).is_err());
        assert!(validate(&[item("a", TodoAction::Reword)]).is_err());
        let reword = TodoItem { message: Some("new".into()), ..item("a", TodoAction::Reword) };
        assert!(validate(&[reword]).is_ok());
    }

    #[test]
    fn reword_amends_after_its_fixups() {
        let items = [
            TodoItem { message: Some("m".into()), ..item("a", TodoAction::Reword) },
            item("b", TodoAction::Fixup),
            item("c", TodoAction::Pick),
            item("d", TodoAction::Drop),
        ];
        let text = todo_text(&items, &|s| format!("sum-{s}"), &|i| PathBuf::from(format!("/m/{i}")));
        let amend = "exec git commit --amend --allow-empty --no-verify --cleanup=whitespace -F '/m/0'";
        assert_eq!(text, format!("pick a sum-a\nfixup b sum-b\n{amend}\npick c sum-c\ndrop d sum-d\n"));
    }

    #[test]
    fn quotes_paths_for_sh() {
        assert_eq!(sh_quote(Path::new("/tmp/it's here")), r"'/tmp/it'\''s here'");
    }

    // --- end to end against the real git binary ---

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git").current_dir(dir).args(args).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn commit(repo: &git2::Repository, file: &str, content: &str, msg: &str) -> String {
        std::fs::write(repo.workdir().unwrap().join(file), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(file)).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<_> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents).unwrap().to_string()
    }

    fn setup() -> (tempfile::TempDir, git2::Repository) {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        config.set_bool("commit.gpgsign", false).unwrap();
        (dir, repo)
    }

    fn messages(dir: &Path) -> Vec<String> {
        git(dir, &["log", "--format=%s", "--reverse"]).lines().map(String::from).collect()
    }

    fn blob_at_head(repo: &git2::Repository, file: &str) -> Option<String> {
        let tree = repo.head().unwrap().peel_to_tree().unwrap();
        let entry = tree.get_path(Path::new(file)).ok()?;
        let blob = repo.find_blob(entry.id()).unwrap();
        Some(String::from_utf8_lossy(blob.content()).into_owned())
    }

    #[test]
    fn plans_from_a_commit_to_head_oldest_first() {
        let (_dir, repo) = setup();
        commit(&repo, "a", "a", "base");
        let b = commit(&repo, "b", "b", "second");
        commit(&repo, "c", "c", "third");
        let plan = plan(&repo, &b).unwrap();
        let summaries: Vec<_> = plan.commits.iter().map(|c| c.summary.as_str()).collect();
        assert_eq!(summaries, ["second", "third"]);
        let base = repo.find_commit(git2::Oid::from_str(&b).unwrap()).unwrap().parent_id(0).unwrap();
        assert_eq!(plan.upstream, Some(base.to_string()));
    }

    #[test]
    fn reorders_squashes_drops_and_rewords() {
        let (dir, repo) = setup();
        commit(&repo, "base.txt", "base\n", "base");
        let a = commit(&repo, "a.txt", "a\n", "add a");
        let b = commit(&repo, "b.txt", "b\n", "add b");
        let c = commit(&repo, "c.txt", "c\n", "add c");
        let d = commit(&repo, "a.txt", "a\nmore\n", "tweak a");
        let plan = plan(&repo, &a).unwrap();
        assert_eq!(plan.commits.len(), 4);

        let items = vec![
            // c first, reworded; a with its tweak squashed in; b dropped
            TodoItem { message: Some("add c (reworded)\n\nwith a body".into()), ..item(&c, TodoAction::Reword) },
            item(&a, TodoAction::Pick),
            item(&d, TodoAction::Fixup),
            item(&b, TodoAction::Drop),
        ];
        let status = start(&repo, plan.upstream.as_deref(), &items).unwrap();
        assert!(matches!(status.state, RebaseState::Complete), "{status:?}");

        assert_eq!(messages(dir.path()), ["base", "add c (reworded)", "add a"]);
        assert_eq!(git(dir.path(), &["log", "-1", "--skip=1", "--format=%b"]), "with a body");
        assert_eq!(blob_at_head(&repo, "a.txt").as_deref(), Some("a\nmore\n"));
        assert_eq!(blob_at_head(&repo, "b.txt"), None);
        assert!(!repo.path().join(STATE_DIR).exists());
    }

    /// What the graph's "Squash N commits" sends: the oldest reworded with the
    /// user's message, the rest fixed up, newer commits picked untouched.
    #[test]
    fn squashes_adjacent_commits_into_one_with_the_given_message() {
        let (dir, repo) = setup();
        commit(&repo, "base.txt", "base\n", "base");
        let a = commit(&repo, "a.txt", "a\n", "add a");
        let b = commit(&repo, "b.txt", "b\n", "add b");
        let c = commit(&repo, "c.txt", "c\n", "add c");
        let d = commit(&repo, "d.txt", "d\n", "add d");
        let plan = plan(&repo, &b).unwrap(); // squash b + c; d is newer
        let items = vec![
            TodoItem { message: Some("add b and c".into()), ..item(&b, TodoAction::Reword) },
            item(&c, TodoAction::Fixup),
            item(&d, TodoAction::Pick),
        ];
        let status = start(&repo, plan.upstream.as_deref(), &items).unwrap();
        assert!(matches!(status.state, RebaseState::Complete), "{status:?}");
        assert_eq!(messages(dir.path()), ["base", "add a", "add b and c", "add d"]);
        assert_eq!(blob_at_head(&repo, "b.txt").as_deref(), Some("b\n"));
        assert_eq!(blob_at_head(&repo, "c.txt").as_deref(), Some("c\n"));
        let _ = a;
    }

    /// The app's "Sign commits" setting, not git config, asks for signing:
    /// the commits git makes (picks and the reword's amend) still get signed.
    #[cfg(unix)]
    #[test]
    fn signs_rewritten_commits_per_the_app_setting() {
        use crate::git::signing::{
            self,
            tests::{git_verifies, keygen},
        };
        let (dir, repo) = setup();
        let Some(key) = keygen(dir.path(), "id", "") else { return };
        commit(&repo, "base.txt", "base\n", "base");
        let a = commit(&repo, "a.txt", "a\n", "add a");
        let b = commit(&repo, "b.txt", "b\n", "add b");
        let plan = plan(&repo, &a).unwrap();
        let items = [item(&b, TodoAction::Pick), TodoItem { message: Some("add a, reworded".into()), ..item(&a, TodoAction::Reword) }];
        let signer = signing::Signer {
            method: signing::Method::Ssh(signing::SshKey::File(key.clone())),
            program: "ssh-keygen".into(),
            askpass: std::path::PathBuf::from("/bin/false"),
            log_dir: None,
        };
        let env = signing::cli_env(&signer, true, None);
        let status = start_with_env(&repo, plan.upstream.as_deref(), &items, &env).unwrap();
        assert!(matches!(status.state, RebaseState::Complete), "{status:?}");
        assert_eq!(messages(dir.path()), ["base", "add b", "add a, reworded"]);

        let head = repo.head().unwrap().peel_to_commit().unwrap();
        for c in [head.clone(), head.parent(0).unwrap()] {
            let sha = c.id().to_string();
            assert!(signing::commit_signature(&repo, &sha).unwrap().is_some(), "{:?} unsigned", c.summary());
            if let Some(ok) = git_verifies(dir.path(), &key, &sha) {
                assert!(ok, "git rejected the signature of {:?}", c.summary());
            }
        }
        // The base wasn't rewritten, so it stays as it was.
        assert!(signing::commit_signature(&repo, &head.parent(0).unwrap().parent_id(0).unwrap().to_string()).unwrap().is_none());
    }

    #[test]
    fn squash_keeps_both_messages() {
        let (dir, repo) = setup();
        commit(&repo, "base.txt", "base\n", "base");
        let a = commit(&repo, "a.txt", "a\n", "first part");
        let b = commit(&repo, "b.txt", "b\n", "second part");
        let plan = plan(&repo, &a).unwrap();
        let items = [item(&a, TodoAction::Pick), item(&b, TodoAction::Squash)];
        start(&repo, plan.upstream.as_deref(), &items).unwrap();
        let full = git(dir.path(), &["log", "-1", "--format=%B"]);
        assert!(full.contains("first part") && full.contains("second part"), "{full}");
        assert_eq!(messages(dir.path()).len(), 2);
    }

    #[test]
    fn rewrites_from_the_root_commit() {
        let (dir, repo) = setup();
        let root = commit(&repo, "a.txt", "a\n", "root");
        let b = commit(&repo, "b.txt", "b\n", "second");
        let plan = plan(&repo, &root).unwrap();
        assert_eq!(plan.upstream, None);
        let items = [item(&b, TodoAction::Pick), item(&root, TodoAction::Pick)];
        start(&repo, None, &items).unwrap();
        assert_eq!(messages(dir.path()), ["second", "root"]);
    }

    #[test]
    fn a_conflict_stops_for_the_panel_and_the_reword_lands_after_continue() {
        let (dir, repo) = setup();
        commit(&repo, "f.txt", "base\n", "base");
        let a = commit(&repo, "f.txt", "one\n", "change one");
        let b = commit(&repo, "f.txt", "two\n", "change two");
        let plan = plan(&repo, &a).unwrap();
        // swapping two edits of the same line conflicts
        let items = [TodoItem { message: Some("two, reworded".into()), ..item(&b, TodoAction::Reword) }, item(&a, TodoAction::Pick)];
        let status = start(&repo, plan.upstream.as_deref(), &items).unwrap();
        assert!(matches!(status.state, RebaseState::Conflicts));
        assert_eq!(status.conflicts.len(), 1);
        assert!(repo.path().join(STATE_DIR).exists());

        rebase::resolve_conflict(&repo, "f.txt", rebase::ConflictResolution::UseTheirs).unwrap();
        let _ = external_git::rebase_continue(dir.path(), &[]);
        after_git(&repo);
        // the second pick conflicts too (base->one on top of two)
        let status = rebase::read_status(&repo).unwrap().unwrap();
        assert_eq!(status.conflicts.len(), 1);
        rebase::resolve_conflict(&repo, "f.txt", rebase::ConflictResolution::UseTheirs).unwrap();
        external_git::rebase_continue(dir.path(), &[]).unwrap();
        after_git(&repo);

        assert!(!rebase::is_rebase_in_progress(&repo));
        assert_eq!(messages(dir.path()), ["base", "two, reworded", "change one"]);
        assert!(!repo.path().join(STATE_DIR).exists());
    }

    #[test]
    fn aborting_a_stopped_rebase_restores_the_branch() {
        let (dir, repo) = setup();
        commit(&repo, "f.txt", "base\n", "base");
        let a = commit(&repo, "f.txt", "one\n", "change one");
        let b = commit(&repo, "f.txt", "two\n", "change two");
        let head = git(dir.path(), &["rev-parse", "HEAD"]);
        let plan = plan(&repo, &a).unwrap();
        let items = [item(&b, TodoAction::Pick), item(&a, TodoAction::Pick)];
        let status = start(&repo, plan.upstream.as_deref(), &items).unwrap();
        assert!(matches!(status.state, RebaseState::Conflicts));

        external_git::rebase_abort(dir.path()).unwrap();
        after_git(&repo);
        assert!(!rebase::is_rebase_in_progress(&repo));
        assert_eq!(git(dir.path(), &["rev-parse", "HEAD"]), head);
        assert!(repo.index().unwrap().conflicts().unwrap().next().is_none());
        assert!(!repo.path().join(STATE_DIR).exists());
    }

    #[test]
    fn refuses_a_list_made_before_head_moved() {
        let (dir, repo) = setup();
        commit(&repo, "a.txt", "a\n", "base");
        let a = commit(&repo, "b.txt", "b\n", "planned");
        let plan = plan(&repo, &a).unwrap();
        // committed while the modal was open: running the old list would drop it
        commit(&repo, "c.txt", "c\n", "new since");
        let head = git(dir.path(), &["rev-parse", "HEAD"]);
        let items = [item(&a, TodoAction::Pick)];
        assert!(start(&repo, plan.upstream.as_deref(), &items).is_err());
        assert_eq!(git(dir.path(), &["rev-parse", "HEAD"]), head);
        assert!(!repo.path().join(STATE_DIR).exists());
    }

    #[test]
    fn stashes_and_restores_uncommitted_changes() {
        let (dir, repo) = setup();
        commit(&repo, "base.txt", "base\n", "base");
        let a = commit(&repo, "a.txt", "a\n", "one");
        let b = commit(&repo, "b.txt", "b\n", "two");
        std::fs::write(dir.path().join("base.txt"), "edited\n").unwrap();
        let plan = plan(&repo, &a).unwrap();
        let items = [item(&b, TodoAction::Pick), item(&a, TodoAction::Pick)];
        let status = start(&repo, plan.upstream.as_deref(), &items).unwrap();
        assert!(matches!(status.state, RebaseState::Complete), "{status:?}");
        assert_eq!(messages(dir.path()), ["base", "two", "one"]);
        let text = std::fs::read_to_string(dir.path().join("base.txt")).unwrap();
        assert_eq!(text.replace("\r\n", "\n"), "edited\n");
    }

    #[test]
    fn rebases_a_detached_head() {
        let (dir, repo) = setup();
        commit(&repo, "base.txt", "base\n", "base");
        let a = commit(&repo, "a.txt", "a\n", "one");
        let b = commit(&repo, "b.txt", "b\n", "two");
        git(dir.path(), &["checkout", "--detach", "HEAD"]);
        let plan = plan(&repo, &a).unwrap();
        let items = [item(&a, TodoAction::Pick), item(&b, TodoAction::Fixup)];
        start(&repo, plan.upstream.as_deref(), &items).unwrap();
        assert!(repo.head_detached().unwrap());
        assert_eq!(messages(dir.path()), ["base", "one"]);
    }

    #[test]
    fn refuses_an_invalid_list_without_touching_the_repo() {
        let (dir, repo) = setup();
        commit(&repo, "a.txt", "a\n", "base");
        let a = commit(&repo, "b.txt", "b\n", "only");
        let head = git(dir.path(), &["rev-parse", "HEAD"]);
        assert!(start(&repo, None, &[item(&a, TodoAction::Squash)]).is_err());
        assert_eq!(git(dir.path(), &["rev-parse", "HEAD"]), head);
        assert!(!rebase::is_rebase_in_progress(&repo));
    }
}

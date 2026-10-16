//! `git pull` for the checked-out branch: fetch its upstream, then
//! fast-forward, merge or rebase onto it. Conflicts are left exactly the way
//! "Merge into current branch" and "Rebase onto" leave theirs, so the existing
//! conflict panel (continue/abort) finishes the job — including restoring the
//! autostash, which shares rebase's marker file.

use serde::{Deserialize, Serialize};

use crate::activity;
use crate::error::GitCommandError;

use super::rebase::{self, ConflictedFile, RebaseState};
use super::remote::{self, AuthMode};
use super::{hooks, merge};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PullStrategy {
    FfOnly,
    Merge,
    Rebase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PullOutcome {
    UpToDate,
    FastForwarded,
    Merged,
    Rebased,
    /// Stopped on conflicts: the repo is mid-merge or mid-rebase.
    Conflicts,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullResult {
    /// Short name of what was pulled, e.g. `origin/main`.
    pub upstream: String,
    pub strategy: PullStrategy,
    pub outcome: PullOutcome,
    /// Upstream commits that weren't on the branch yet.
    pub commits: usize,
    /// Paths only: the UI loads the full conflict status (contents, patches)
    /// for the conflict panel itself, so it isn't serialized twice.
    pub conflicts: Vec<String>,
    /// The pull itself went fine but re-applying the autostash conflicted;
    /// the UI opens the conflict panel for these.
    pub autostash_conflicts: Vec<String>,
}

fn paths(conflicts: Vec<ConflictedFile>) -> Vec<String> {
    conflicts.into_iter().map(|c| c.path).collect()
}

/// Where the checked-out branch stands against its upstream, for the Pull
/// button. Counts come from the last fetch; nothing here touches the network.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamStatus {
    pub upstream: String,
    pub ahead: usize,
    pub behind: usize,
    /// What a plain Pull does, per `pull.rebase` / `branch.<name>.rebase` / `pull.ff`.
    pub strategy: PullStrategy,
}

/// The checked-out branch's `branch.<name>.remote` / `.merge`.
struct Upstream {
    branch: String,
    head_ref: String,
    remote: String,
    /// Local ref holding the upstream's tip: `refs/remotes/<remote>/<b>`, or
    /// `refs/heads/<b>` when the remote is `.` (a local branch).
    tracking_ref: String,
    /// The branch's name on the remote (`branch.<name>.merge` without `refs/heads/`).
    merge_branch: String,
}

impl Upstream {
    fn display(&self) -> &str {
        self.tracking_ref
            .strip_prefix("refs/remotes/")
            .or_else(|| self.tracking_ref.strip_prefix("refs/heads/"))
            .unwrap_or(&self.tracking_ref)
    }
}

fn upstream_of_head(repo: &git2::Repository) -> Result<Upstream, GitCommandError> {
    // HEAD's symbolic target rather than `repo.head()`, which fails on an
    // unborn branch (fresh `git init` + upstream) — a pull is how that gets
    // its first commit.
    let head = repo.find_reference("HEAD")?;
    let Some(head_ref) = head.symbolic_target().ok().flatten().filter(|r| r.starts_with("refs/heads/")) else {
        return Err(GitCommandError::Internal("HEAD destacado (detached): faça checkout de uma branch para o pull".into()));
    };
    let head_ref = head_ref.to_string();
    let branch = head_ref["refs/heads/".len()..].to_string();
    let no_upstream = || GitCommandError::NoUpstream { branch: branch.clone() };

    let remote = repo.branch_upstream_remote(&head_ref).ok().and_then(|b| b.as_str().ok().map(String::from)).ok_or_else(no_upstream)?;
    let tracking_ref = repo.branch_upstream_name(&head_ref).ok().and_then(|b| b.as_str().ok().map(String::from)).ok_or_else(no_upstream)?;
    let merge_branch = repo
        .config()?
        .get_string(&format!("branch.{branch}.merge"))
        .map(|m| m.trim_start_matches("refs/heads/").to_string())
        .map_err(|_| no_upstream())?;

    Ok(Upstream { branch, head_ref, remote, tracking_ref, merge_branch })
}

/// What a plain `git pull` would do on `branch`, plus whether `pull.ff=false`
/// asks for a merge commit even when a fast-forward is possible.
/// `branch.<name>.rebase` wins over `pull.rebase`, and rebasing over `pull.ff`.
pub fn configured_strategy(config: &git2::Config, branch: &str) -> (PullStrategy, bool) {
    let get = |key: &str| config.get_string(key).ok().map(|v| v.trim().to_ascii_lowercase());
    let rebase = get(&format!("branch.{branch}.rebase")).or_else(|| get("pull.rebase"));
    if rebase.as_deref().is_some_and(rebase_enabled) {
        return (PullStrategy::Rebase, false);
    }
    match get("pull.ff").as_deref() {
        Some("only") => (PullStrategy::FfOnly, false),
        Some("false" | "no" | "off" | "0") => (PullStrategy::Merge, true),
        _ => (PullStrategy::Merge, false),
    }
}

/// `pull.rebase` takes a bool or a rebase flavour (`merges`, `interactive`);
/// any flavour still means "rebase", which is all we do.
fn rebase_enabled(value: &str) -> bool {
    matches!(value, "true" | "yes" | "on" | "1" | "merges" | "m" | "interactive" | "i")
}

/// Ahead/behind of the checked-out branch vs. its upstream as last fetched.
/// `None` when there's nothing to pull from (detached HEAD, no upstream).
pub fn upstream_status(repo: &git2::Repository) -> Result<Option<UpstreamStatus>, GitCommandError> {
    let Ok(up) = upstream_of_head(repo) else {
        return Ok(None);
    };
    let (strategy, _) = configured_strategy(&repo.config()?, &up.branch);
    let (ahead, behind) = match (repo.refname_to_id(&up.head_ref), repo.refname_to_id(&up.tracking_ref)) {
        (Ok(ours), Ok(theirs)) => repo.graph_ahead_behind(ours, theirs)?,
        // Upstream not fetched yet.
        _ => (0, 0),
    };
    Ok(Some(UpstreamStatus { upstream: up.display().to_string(), ahead, behind, strategy }))
}

/// Pulls the checked-out branch. `choice` overrides the configured strategy
/// for this one pull. Uncommitted changes are stashed first and restored once
/// the pull is through — after continue/abort when it stops on conflicts.
pub fn pull(
    repo: &mut git2::Repository,
    choice: Option<PullStrategy>,
    on_progress: impl FnMut(usize, usize, usize) + 'static,
) -> Result<PullResult, GitCommandError> {
    if repo.state() != git2::RepositoryState::Clean || rebase::is_rebase_in_progress(repo) {
        return Err(GitCommandError::Internal("conclua ou aborte o merge/rebase em andamento antes do pull".into()));
    }
    let up = upstream_of_head(repo)?;
    let (configured, config_no_ff) = configured_strategy(&repo.config()?, &up.branch);
    let strategy = choice.unwrap_or(configured);
    let no_ff = strategy == PullStrategy::Merge && config_no_ff;

    let flag = match strategy {
        PullStrategy::FfOnly => "--ff-only",
        PullStrategy::Merge if no_ff => "--no-rebase --no-ff",
        PullStrategy::Merge => "--no-rebase",
        PullStrategy::Rebase => "--rebase",
    };
    let dir = repo.workdir().unwrap_or(repo.path()).to_path_buf();
    activity::info(Some(&dir), format!("$ git pull {flag} {} {}", up.remote, up.merge_branch));

    if up.remote != "." {
        remote::fetch(repo, &up.remote, AuthMode::Interactive, on_progress)?;
    }

    let their_oid = repo
        .refname_to_id(&up.tracking_ref)
        .map_err(|_| GitCommandError::Internal(format!("{} não existe: nada para o pull", up.display())))?;
    let Ok(head_oid) = repo.refname_to_id(&up.head_ref) else {
        return pull_into_unborn(repo, &up, strategy, their_oid);
    };
    let (_, commits) = repo.graph_ahead_behind(head_oid, their_oid)?;
    let analysis = {
        let theirs = repo.find_annotated_commit(their_oid)?;
        repo.merge_analysis(&[&theirs])?.0
    };

    let result = |outcome, conflicts| PullResult {
        upstream: up.display().to_string(),
        strategy,
        outcome,
        commits,
        conflicts,
        autostash_conflicts: Vec::new(),
    };
    if analysis.is_up_to_date() {
        return Ok(result(PullOutcome::UpToDate, Vec::new()));
    }
    let fast_forward = analysis.is_fast_forward() && !no_ff;
    if strategy == PullStrategy::FfOnly && !fast_forward {
        return Err(GitCommandError::PullNotFastForward { upstream: up.display().to_string() });
    }

    let stashed = rebase::has_uncommitted_changes(repo)?;
    if stashed {
        rebase::autostash(repo)?;
    }
    let integrated = if fast_forward {
        fast_forward_to(repo, &up.head_ref, their_oid).map(|_| {
            // git pull runs post-merge after a fast-forward too.
            hooks::run_post(repo, "post-merge", &["0"]);
            (PullOutcome::FastForwarded, Vec::new())
        })
    } else if strategy == PullStrategy::Rebase {
        rebase::start(repo, &up.tracking_ref).map(|(_session, status)| match status.state {
            RebaseState::Complete => (PullOutcome::Rebased, Vec::new()),
            RebaseState::Conflicts => (PullOutcome::Conflicts, paths(status.conflicts)),
        })
    } else {
        merge_upstream(repo, &up)
    };

    match integrated {
        Ok((outcome, conflicts)) => {
            let mut out = result(outcome, conflicts);
            // On conflicts the stash waits for continue/abort, like rebase's.
            if outcome != PullOutcome::Conflicts {
                out.autostash_conflicts = paths(rebase::pop_autostash_if_any(repo)?);
            }
            Ok(out)
        }
        Err(e) => {
            // Failed without conflicts (e.g. an untracked file in the way of
            // the checkout). libgit2 cleans up when the first checkout fails,
            // but a rebase failing on a later step stays in progress with no
            // conflict for the panel to show — roll it back, then put the
            // stashed changes back so nothing is left hidden in a stash.
            roll_back(repo);
            if stashed {
                let _ = rebase::pop_autostash_if_any(repo);
            }
            Err(e)
        }
    }
}

/// Undoes a merge/rebase this pull started (the repo was clean before it).
/// HEAD hasn't moved for a merge and every tracked change was autostashed,
/// so resetting to HEAD loses nothing; a rebase abort restores the branch.
fn roll_back(repo: &git2::Repository) {
    if rebase::is_rebase_in_progress(repo) {
        if let Ok(mut session) = repo.open_rebase(None) {
            let _ = session.abort();
        }
    } else if repo.state() != git2::RepositoryState::Clean {
        let _ = merge::abort_merge(repo);
    }
}

/// First pull into a branch with no commits yet (`git init` + upstream):
/// the branch simply starts at the upstream's tip, as with `git pull`.
fn pull_into_unborn(
    repo: &git2::Repository,
    up: &Upstream,
    strategy: PullStrategy,
    their_oid: git2::Oid,
) -> Result<PullResult, GitCommandError> {
    // Staged files can't be autostashed without a HEAD commit, and checking
    // the upstream out over them could clobber them.
    if !repo.index()?.is_empty() {
        return Err(GitCommandError::Internal(
            "a branch ainda não tem commits, mas há arquivos em staging: faça commit ou remova-os do staging antes do pull".into(),
        ));
    }
    let commits = {
        let mut walk = repo.revwalk()?;
        walk.push(their_oid)?;
        walk.count()
    };
    fast_forward_to(repo, &up.head_ref, their_oid)?;
    Ok(PullResult {
        upstream: up.display().to_string(),
        strategy,
        outcome: PullOutcome::FastForwarded,
        commits,
        conflicts: Vec::new(),
        autostash_conflicts: Vec::new(),
    })
}

/// Moves (or, on an unborn branch, creates) the branch at `target` after
/// checking its tree out; a safe checkout refuses (changing nothing) if that
/// would clobber a file.
fn fast_forward_to(repo: &git2::Repository, head_ref: &str, target: git2::Oid) -> Result<(), GitCommandError> {
    let commit = repo.find_commit(target)?;
    repo.checkout_tree(commit.as_object(), Some(git2::build::CheckoutBuilder::new().safe()))?;
    repo.reference(head_ref, target, true, &format!("pull: Fast-forward to {target}"))?;
    Ok(())
}

/// Merge commit named the way `git pull` names it.
fn merge_upstream(repo: &git2::Repository, up: &Upstream) -> Result<(PullOutcome, Vec<String>), GitCommandError> {
    let message = if up.remote == "." {
        format!("Merge branch '{}'", up.merge_branch)
    } else {
        let url = repo.find_remote(&up.remote)?.url().unwrap_or(&up.remote).to_string();
        format!("Merge branch '{}' of {url}", up.merge_branch)
    };
    let their_ref = repo.find_reference(&up.tracking_ref)?;
    let merged = merge::merge_reference(repo, &their_ref, &message)?;
    Ok(if merged.conflicts.is_empty() { (PullOutcome::Merged, Vec::new()) } else { (PullOutcome::Conflicts, paths(merged.conflicts)) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn configure(repo: &git2::Repository) {
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        // Pin what a machine's global config could change under the tests.
        config.set_bool("core.autocrlf", false).unwrap();
        config.set_bool("pull.rebase", false).unwrap();
        config.set_str("pull.ff", "true").unwrap();
    }

    fn commit_file(repo: &git2::Repository, file: &str, content: &str, msg: &str) -> git2::Oid {
        std::fs::write(repo.workdir().unwrap().join(file), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(file)).unwrap();
        let tree_id = index.write_tree().unwrap();
        index.write().unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents).unwrap()
    }

    fn head_oid(repo: &git2::Repository) -> git2::Oid {
        repo.head().unwrap().target().unwrap()
    }

    fn read(repo: &git2::Repository, file: &str) -> String {
        std::fs::read_to_string(repo.workdir().unwrap().join(file)).unwrap()
    }

    /// A bare "server" plus two clones of it: `local` (the one pulling, its
    /// branch tracking origin) and `other` (pushing commits to pull).
    struct Setup {
        _dirs: Vec<tempfile::TempDir>,
        local: git2::Repository,
        other: git2::Repository,
        branch: String,
    }

    fn setup() -> Setup {
        let bare = tempfile::tempdir().unwrap();
        git2::Repository::init_bare(bare.path()).unwrap();
        let bare_url = bare.path().to_str().unwrap().to_string();

        let seed_dir = tempfile::tempdir().unwrap();
        let seed = git2::Repository::init(seed_dir.path()).unwrap();
        configure(&seed);
        commit_file(&seed, "a.txt", "base", "initial");
        let branch = seed.head().unwrap().shorthand().unwrap().to_string();
        seed.remote("origin", &bare_url).unwrap();
        remote::push(&seed, "origin", &branch, false, |_, _, _| {}).unwrap();
        // The bare repo's HEAD may name another default branch; point it at ours.
        git2::Repository::open_bare(bare.path()).unwrap().set_head(&format!("refs/heads/{branch}")).unwrap();

        let clone = |dir: &tempfile::TempDir| {
            let repo = git2::Repository::clone(&bare_url, dir.path()).unwrap();
            configure(&repo);
            repo
        };
        let local_dir = tempfile::tempdir().unwrap();
        let other_dir = tempfile::tempdir().unwrap();
        let local = clone(&local_dir);
        let other = clone(&other_dir);
        Setup { _dirs: vec![bare, seed_dir, local_dir, other_dir], local, other, branch }
    }

    impl Setup {
        /// Commits on `other` and pushes it, returning the new remote tip.
        fn upstream_commit(&self, file: &str, content: &str) -> git2::Oid {
            let oid = commit_file(&self.other, file, content, &format!("remote {file}"));
            remote::push(&self.other, "origin", &self.branch, false, |_, _, _| {}).unwrap();
            oid
        }
    }

    fn pull_with(repo: &mut git2::Repository, choice: Option<PullStrategy>) -> Result<PullResult, GitCommandError> {
        pull(repo, choice, |_, _, _| {})
    }

    #[test]
    fn fast_forwards_when_behind() {
        let mut s = setup();
        let remote_tip = s.upstream_commit("b.txt", "remote");

        let result = pull_with(&mut s.local, None).unwrap();
        assert_eq!(result.outcome, PullOutcome::FastForwarded);
        assert_eq!(result.commits, 1);
        assert_eq!(result.upstream, format!("origin/{}", s.branch));
        assert_eq!(head_oid(&s.local), remote_tip);
        assert_eq!(read(&s.local, "b.txt"), "remote");
        assert!(!rebase::has_uncommitted_changes(&s.local).unwrap());

        let again = pull_with(&mut s.local, None).unwrap();
        assert_eq!(again.outcome, PullOutcome::UpToDate);
    }

    #[test]
    fn rebase_choice_still_fast_forwards_without_local_commits() {
        let mut s = setup();
        let remote_tip = s.upstream_commit("b.txt", "remote");
        let result = pull_with(&mut s.local, Some(PullStrategy::Rebase)).unwrap();
        assert_eq!(result.outcome, PullOutcome::FastForwarded);
        assert_eq!(head_oid(&s.local), remote_tip);
    }

    /// Pulls run the hooks `git pull` does and sign the merge commit it makes.
    #[cfg(unix)]
    #[test]
    fn pull_runs_merge_hooks_and_signs_the_merge() {
        use crate::git::hooks::tests::write_hook;
        use crate::git::signing::{self, tests::keygen};
        let mut s = setup();
        let hooks = s.local.path().join("hooks");
        let log = s.local.path().join("hooks.log");
        for name in ["pre-merge-commit", "prepare-commit-msg", "commit-msg", "post-merge"] {
            write_hook(&hooks, name, &format!("echo \"{name} $*\" >> '{}'", log.display()));
        }
        s.upstream_commit("b.txt", "remote");
        assert_eq!(pull_with(&mut s.local, None).unwrap().outcome, PullOutcome::FastForwarded);
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "post-merge 0\n");

        let Some(key) = keygen(s._dirs[0].path(), "id", "") else { return };
        let mut config = s.local.config().unwrap();
        config.set_bool("commit.gpgsign", true).unwrap();
        config.set_str("gpg.format", "ssh").unwrap();
        config.set_str("user.signingkey", &key.display().to_string()).unwrap();
        std::fs::remove_file(&log).unwrap();
        commit_file(&s.local, "c.txt", "local", "local");
        s.upstream_commit("d.txt", "remote 2");
        assert_eq!(pull_with(&mut s.local, None).unwrap().outcome, PullOutcome::Merged);
        let hooks_run: Vec<String> =
            std::fs::read_to_string(&log).unwrap().lines().map(|l| l.split(' ').next().unwrap().to_string()).collect();
        assert_eq!(hooks_run, ["pre-merge-commit", "prepare-commit-msg", "commit-msg", "post-merge"]);
        let head = head_oid(&s.local).to_string();
        assert!(signing::commit_signature(&s.local, &head).unwrap().is_some());
    }

    #[test]
    fn merges_a_diverged_upstream() {
        let mut s = setup();
        let remote_tip = s.upstream_commit("b.txt", "remote");
        let local_tip = commit_file(&s.local, "c.txt", "local", "local change");

        let result = pull_with(&mut s.local, Some(PullStrategy::Merge)).unwrap();
        assert_eq!(result.outcome, PullOutcome::Merged);
        let head = s.local.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.parent_ids().collect::<Vec<_>>(), [local_tip, remote_tip]);
        assert!(head.message().unwrap().starts_with(&format!("Merge branch '{}' of ", s.branch)));
        assert_eq!(s.local.state(), git2::RepositoryState::Clean);
        assert_eq!(read(&s.local, "b.txt"), "remote");
    }

    #[test]
    fn pull_ff_false_makes_a_merge_commit_even_when_fast_forwardable() {
        let mut s = setup();
        s.local.config().unwrap().set_str("pull.ff", "false").unwrap();
        let remote_tip = s.upstream_commit("b.txt", "remote");
        let result = pull_with(&mut s.local, None).unwrap();
        assert_eq!(result.outcome, PullOutcome::Merged);
        let head = s.local.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.parent_count(), 2);
        assert_eq!(head.parent_id(1).unwrap(), remote_tip);
    }

    #[test]
    fn rebases_local_commits_onto_the_upstream() {
        let mut s = setup();
        let remote_tip = s.upstream_commit("b.txt", "remote");
        commit_file(&s.local, "c.txt", "local", "local change");

        let result = pull_with(&mut s.local, Some(PullStrategy::Rebase)).unwrap();
        assert_eq!(result.outcome, PullOutcome::Rebased);
        let head = s.local.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.parent_ids().collect::<Vec<_>>(), [remote_tip]);
        assert_eq!(head.summary().unwrap(), Some("local change"));
        assert!(s.local.head().unwrap().is_branch());
        assert!(!rebase::is_rebase_in_progress(&s.local));
    }

    #[test]
    fn config_pull_rebase_picks_rebase() {
        let mut s = setup();
        s.local.config().unwrap().set_bool("pull.rebase", true).unwrap();
        s.upstream_commit("b.txt", "remote");
        commit_file(&s.local, "c.txt", "local", "local change");
        let result = pull_with(&mut s.local, None).unwrap();
        assert_eq!(result.strategy, PullStrategy::Rebase);
        assert_eq!(result.outcome, PullOutcome::Rebased);
    }

    #[test]
    fn ff_only_refuses_a_diverged_upstream_and_changes_nothing() {
        let mut s = setup();
        s.local.config().unwrap().set_str("pull.ff", "only").unwrap();
        s.upstream_commit("b.txt", "remote");
        let local_tip = commit_file(&s.local, "c.txt", "local", "local change");
        std::fs::write(s.local.workdir().unwrap().join("a.txt"), "dirty").unwrap();

        let err = pull_with(&mut s.local, None).unwrap_err();
        assert!(matches!(err, GitCommandError::PullNotFastForward { .. }), "{err:?}");
        assert_eq!(head_oid(&s.local), local_tip);
        assert_eq!(read(&s.local, "a.txt"), "dirty");

        // The one-off override still works.
        let result = pull_with(&mut s.local, Some(PullStrategy::Merge)).unwrap();
        assert_eq!(result.outcome, PullOutcome::Merged);
        assert_eq!(read(&s.local, "a.txt"), "dirty");
    }

    #[test]
    fn merge_conflict_leaves_merge_state_for_the_conflict_panel() {
        let mut s = setup();
        let remote_tip = s.upstream_commit("a.txt", "remote");
        commit_file(&s.local, "a.txt", "local", "local change");

        let result = pull_with(&mut s.local, Some(PullStrategy::Merge)).unwrap();
        assert_eq!(result.outcome, PullOutcome::Conflicts);
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts, ["a.txt"]);
        assert_eq!(s.local.state(), git2::RepositoryState::Merge);
        // Same status the conflict panel reads.
        assert_eq!(rebase::read_status(&s.local).unwrap().unwrap().conflicts.len(), 1);

        rebase::resolve_conflict(&s.local, "a.txt", rebase::ConflictResolution::UseTheirs).unwrap();
        merge::continue_merge(&mut s.local).unwrap();
        let head = s.local.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.parent_id(1).unwrap(), remote_tip);
        assert_eq!(read(&s.local, "a.txt"), "remote");
    }

    #[test]
    fn rebase_conflict_leaves_a_rebase_in_progress() {
        let mut s = setup();
        s.upstream_commit("a.txt", "remote");
        commit_file(&s.local, "a.txt", "local", "local change");

        let result = pull_with(&mut s.local, Some(PullStrategy::Rebase)).unwrap();
        assert_eq!(result.outcome, PullOutcome::Conflicts);
        assert_eq!(result.conflicts, ["a.txt"]);
        assert!(rebase::is_rebase_in_progress(&s.local));

        let err = pull_with(&mut s.local, None).unwrap_err();
        assert!(matches!(err, GitCommandError::Internal(_)));

        rebase::resolve_conflict(&s.local, "a.txt", rebase::ConflictResolution::UseOurs).unwrap();
        let mut session = s.local.open_rebase(None).unwrap();
        let status = rebase::advance(&s.local, &mut session, true).unwrap();
        assert!(matches!(status.state, RebaseState::Complete));
    }

    #[test]
    fn errors_clearly_without_an_upstream() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = git2::Repository::init(dir.path()).unwrap();
        configure(&repo);
        commit_file(&repo, "a.txt", "base", "initial");
        let branch = repo.head().unwrap().shorthand().unwrap().to_string();

        let err = pull_with(&mut repo, None).unwrap_err();
        assert!(matches!(&err, GitCommandError::NoUpstream { branch: b } if *b == branch), "{err:?}");
        assert!(upstream_status(&repo).unwrap().is_none());
    }

    #[test]
    fn autostashes_dirty_changes_around_the_pull() {
        let mut s = setup();
        s.upstream_commit("b.txt", "remote");
        commit_file(&s.local, "c.txt", "local", "local change");
        std::fs::write(s.local.workdir().unwrap().join("a.txt"), "dirty").unwrap();

        let result = pull_with(&mut s.local, Some(PullStrategy::Rebase)).unwrap();
        assert_eq!(result.outcome, PullOutcome::Rebased);
        assert!(result.autostash_conflicts.is_empty());
        assert_eq!(read(&s.local, "a.txt"), "dirty");
        assert_eq!(read(&s.local, "b.txt"), "remote");
        assert!(crate::git::stash::list(&mut s.local).unwrap().is_empty());
    }

    #[test]
    fn failed_fast_forward_puts_the_autostash_back() {
        let mut s = setup();
        let before = head_oid(&s.local);
        s.upstream_commit("b.txt", "remote");
        // Untracked (so not stashed) and in the way of the incoming file.
        std::fs::write(s.local.workdir().unwrap().join("b.txt"), "mine").unwrap();
        std::fs::write(s.local.workdir().unwrap().join("a.txt"), "dirty").unwrap();

        assert!(pull_with(&mut s.local, None).is_err());
        assert_eq!(head_oid(&s.local), before);
        assert_eq!(read(&s.local, "a.txt"), "dirty");
        assert_eq!(read(&s.local, "b.txt"), "mine");
        assert!(crate::git::stash::list(&mut s.local).unwrap().is_empty());
    }

    #[test]
    fn autostash_waits_for_the_conflicted_merge_to_finish() {
        let mut s = setup();
        s.upstream_commit("a.txt", "remote");
        commit_file(&s.local, "a.txt", "local", "local change");
        std::fs::write(s.local.workdir().unwrap().join("c.txt"), "dirty").unwrap();
        let mut index = s.local.index().unwrap();
        index.add_path(Path::new("c.txt")).unwrap();
        index.write().unwrap();

        let result = pull_with(&mut s.local, Some(PullStrategy::Merge)).unwrap();
        assert_eq!(result.outcome, PullOutcome::Conflicts);
        assert!(!s.local.workdir().unwrap().join("c.txt").exists());

        rebase::resolve_conflict(&s.local, "a.txt", rebase::ConflictResolution::UseOurs).unwrap();
        merge::continue_merge(&mut s.local).unwrap();
        rebase::pop_autostash_if_any(&mut s.local).unwrap();
        assert_eq!(read(&s.local, "c.txt"), "dirty");
    }

    #[test]
    fn reports_ahead_and_behind_from_the_last_fetch() {
        let mut s = setup();
        s.upstream_commit("b.txt", "remote");
        commit_file(&s.local, "c.txt", "local", "local change");

        let before = upstream_status(&s.local).unwrap().unwrap();
        assert_eq!((before.ahead, before.behind), (1, 0));
        remote::fetch(&s.local, "origin", AuthMode::Interactive, |_, _, _| {}).unwrap();
        let after = upstream_status(&s.local).unwrap().unwrap();
        assert_eq!((after.ahead, after.behind), (1, 1));
        assert_eq!(after.strategy, PullStrategy::Merge);

        pull_with(&mut s.local, None).unwrap();
        let merged = upstream_status(&s.local).unwrap().unwrap();
        assert_eq!((merged.ahead, merged.behind), (2, 0));
    }

    fn commit_removal(repo: &git2::Repository, file: &str) {
        std::fs::remove_file(repo.workdir().unwrap().join(file)).unwrap();
        let mut index = repo.index().unwrap();
        index.remove_path(Path::new(file)).unwrap();
        let tree_id = index.write_tree().unwrap();
        index.write().unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "remove", &tree, &[&parent]).unwrap();
    }

    #[test]
    fn fast_forward_applies_deletions_and_keeps_untracked_files() {
        let mut s = setup();
        commit_file(&s.other, "gone.txt", "x", "add gone");
        commit_removal(&s.other, "gone.txt");
        commit_removal(&s.other, "a.txt");
        remote::push(&s.other, "origin", &s.branch, false, |_, _, _| {}).unwrap();
        // Only an untracked file: nothing to stash, and it must survive.
        std::fs::write(s.local.workdir().unwrap().join("notes.txt"), "mine").unwrap();

        let result = pull_with(&mut s.local, None).unwrap();
        assert_eq!(result.outcome, PullOutcome::FastForwarded);
        assert_eq!(result.commits, 3);
        assert!(!s.local.workdir().unwrap().join("a.txt").exists());
        assert!(!s.local.workdir().unwrap().join("gone.txt").exists());
        assert_eq!(read(&s.local, "notes.txt"), "mine");
        assert!(crate::git::stash::list(&mut s.local).unwrap().is_empty());
    }

    #[test]
    fn untracked_files_alone_do_not_break_a_merge_or_rebase_pull() {
        for choice in [PullStrategy::Merge, PullStrategy::Rebase] {
            let mut s = setup();
            s.upstream_commit("b.txt", "remote");
            commit_file(&s.local, "c.txt", "local", "local change");
            std::fs::write(s.local.workdir().unwrap().join("notes.txt"), "mine").unwrap();

            let result = pull_with(&mut s.local, Some(choice)).unwrap();
            assert_ne!(result.outcome, PullOutcome::Conflicts, "{choice:?}");
            assert_eq!(read(&s.local, "notes.txt"), "mine");
            assert_eq!(read(&s.local, "b.txt"), "remote");
        }
    }

    /// An untracked file in the way of an incoming one makes the checkout
    /// fail half-way into the merge/rebase: everything must be put back.
    #[test]
    fn a_blocked_merge_or_rebase_is_rolled_back_with_the_autostash() {
        for choice in [PullStrategy::Merge, PullStrategy::Rebase] {
            let mut s = setup();
            s.upstream_commit("b.txt", "remote");
            let local_tip = commit_file(&s.local, "c.txt", "local", "local change");
            std::fs::write(s.local.workdir().unwrap().join("b.txt"), "mine").unwrap();
            std::fs::write(s.local.workdir().unwrap().join("a.txt"), "dirty").unwrap();

            assert!(pull_with(&mut s.local, Some(choice)).is_err(), "{choice:?}");
            assert_eq!(s.local.state(), git2::RepositoryState::Clean, "{choice:?}");
            assert!(!rebase::is_rebase_in_progress(&s.local), "{choice:?}");
            assert!(s.local.head().unwrap().is_branch());
            assert_eq!(head_oid(&s.local), local_tip, "{choice:?}");
            assert_eq!(read(&s.local, "a.txt"), "dirty", "{choice:?}");
            assert_eq!(read(&s.local, "b.txt"), "mine", "{choice:?}");
            assert!(crate::git::stash::list(&mut s.local).unwrap().is_empty(), "{choice:?}");

            // Out of the way, the same pull goes through.
            std::fs::remove_file(s.local.workdir().unwrap().join("b.txt")).unwrap();
            pull_with(&mut s.local, Some(choice)).unwrap();
            assert_eq!(read(&s.local, "b.txt"), "remote");
            assert_eq!(read(&s.local, "a.txt"), "dirty");
        }
    }

    #[test]
    fn reports_autostash_conflicts_after_a_clean_pull() {
        let mut s = setup();
        s.upstream_commit("a.txt", "remote");
        std::fs::write(s.local.workdir().unwrap().join("a.txt"), "dirty").unwrap();

        let result = pull_with(&mut s.local, None).unwrap();
        assert_eq!(result.outcome, PullOutcome::FastForwarded);
        assert_eq!(result.autostash_conflicts, ["a.txt"]);
        // The conflict panel can pick them up.
        assert_eq!(rebase::read_status(&s.local).unwrap().unwrap().conflicts.len(), 1);
        // And the stashed changes survive the conflict, as with git.
        assert_eq!(crate::git::stash::list(&mut s.local).unwrap().len(), 1);
    }

    #[test]
    fn first_pull_into_an_unborn_branch_checks_out_the_upstream() {
        let s = setup();
        let remote_tip = s.upstream_commit("b.txt", "remote");

        let dir = tempfile::tempdir().unwrap();
        let mut repo = git2::Repository::init(dir.path()).unwrap();
        configure(&repo);
        repo.set_head(&format!("refs/heads/{}", s.branch)).unwrap();
        repo.remote("origin", s.other.find_remote("origin").unwrap().url().unwrap()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str(&format!("branch.{}.remote", s.branch), "origin").unwrap();
        config.set_str(&format!("branch.{}.merge", s.branch), &format!("refs/heads/{}", s.branch)).unwrap();
        std::fs::write(dir.path().join("notes.txt"), "mine").unwrap();

        let result = pull_with(&mut repo, None).unwrap();
        assert_eq!(result.outcome, PullOutcome::FastForwarded);
        assert_eq!(result.commits, 2);
        assert_eq!(head_oid(&repo), remote_tip);
        assert!(repo.head().unwrap().is_branch());
        assert_eq!(read(&repo, "a.txt"), "base");
        assert_eq!(read(&repo, "notes.txt"), "mine");
        assert!(!rebase::has_uncommitted_changes(&repo).unwrap());
    }

    #[test]
    fn pulls_from_a_local_upstream_without_fetching() {
        let mut s = setup();
        let trunk_tip = {
            let base = s.local.head().unwrap().peel_to_commit().unwrap();
            s.local.branch("trunk", &base, false).unwrap();
            let mut config = s.local.config().unwrap();
            config.set_str(&format!("branch.{}.remote", s.branch), ".").unwrap();
            config.set_str(&format!("branch.{}.merge", s.branch), "refs/heads/trunk").unwrap();
            // Advance trunk without checking it out.
            let tree = base.tree().unwrap();
            let sig = git2::Signature::now("Test", "test@example.com").unwrap();
            s.local.commit(Some("refs/heads/trunk"), &sig, &sig, "trunk", &tree, &[&base]).unwrap()
        };

        let result = pull_with(&mut s.local, None).unwrap();
        assert_eq!(result.upstream, "trunk");
        assert_eq!(result.outcome, PullOutcome::FastForwarded);
        assert_eq!(head_oid(&s.local), trunk_tip);
    }

    #[test]
    fn refuses_a_detached_head() {
        let mut s = setup();
        let oid = head_oid(&s.local);
        s.local.set_head_detached(oid).unwrap();
        let err = pull_with(&mut s.local, None).unwrap_err();
        assert!(matches!(&err, GitCommandError::Internal(m) if m.contains("detached")), "{err:?}");
        assert!(upstream_status(&s.local).unwrap().is_none());
    }

    fn config_with(entries: &[(&str, &str)]) -> (tempfile::TempDir, git2::Config) {
        let dir = tempfile::tempdir().unwrap();
        let mut config = git2::Config::open(&dir.path().join("config")).unwrap();
        for (k, v) in entries {
            config.set_str(k, v).unwrap();
        }
        (dir, config)
    }

    #[test]
    fn configured_strategy_follows_git_precedence() {
        let cases: &[(&[(&str, &str)], (PullStrategy, bool))] = &[
            (&[], (PullStrategy::Merge, false)),
            (&[("pull.ff", "only")], (PullStrategy::FfOnly, false)),
            (&[("pull.ff", "false")], (PullStrategy::Merge, true)),
            (&[("pull.rebase", "true")], (PullStrategy::Rebase, false)),
            (&[("pull.rebase", "merges")], (PullStrategy::Rebase, false)),
            (&[("pull.rebase", "true"), ("pull.ff", "only")], (PullStrategy::Rebase, false)),
            (&[("pull.rebase", "true"), ("branch.main.rebase", "false")], (PullStrategy::Merge, false)),
            (&[("pull.rebase", "false"), ("branch.main.rebase", "true")], (PullStrategy::Rebase, false)),
            (&[("branch.other.rebase", "true")], (PullStrategy::Merge, false)),
        ];
        for (entries, expected) in cases {
            let (_dir, config) = config_with(entries);
            assert_eq!(configured_strategy(&config, "main"), *expected, "{entries:?}");
        }
    }
}

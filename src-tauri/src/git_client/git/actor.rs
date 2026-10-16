use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;

use crate::git_client::proto::Reply;
use crate::git_client::remote_agent::RemoteRepo;

use crate::git_client::error::GitCommandError;

use super::branch::{self, BranchInfo, TagInfo};
use super::cherry_pick::{self, CherryPickResult};
use super::diff::{self, CommitDiff, FileDiff};
use super::external_git;
use super::history::{self, BlameChunk, BranchCompare, FileHistoryPage, FileHistoryQuery};
use super::log::{self, CommitLogPage, CommitNode, LogQuery};
use super::merge::{self, MergeResult};
use super::partial::{self, LineAction, LineSelection};
use super::pull::{self, PullResult, PullStrategy, UpstreamStatus};
use super::rebase::{self, ConflictResolution, RebaseStatus};
use super::remote::{self, AuthMode, FetchSummary, PushSummary, RemoteInfo};
use super::repo::{self, RepoStatus};
use super::search::{self, SearchMode};
use super::signing;
use super::stash::{self, StashInfo, StashPopResult};
use super::reflog::{self, ReflogEntry};
use super::revert::{self, RevertResult};
use super::stats::{self, RepoStats};
use super::undo::{self, UndoHistory, UndoStatus};
use super::working::{self, WorkingStatus};
use super::interactive_rebase::{self, RebasePlan, TodoItem};
use super::submodule::{self, SubmoduleInfo};
use super::worktree::{self, AddWorktreeArgs, WorktreeInfo};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchProgressEvent {
    repo_id: String,
    received_objects: usize,
    total_objects: usize,
    received_bytes: usize,
    /// Periodic / on-open fetches: the UI shows no progress toast for these.
    background: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PushProgressEvent {
    repo_id: String,
    current: u32,
    total: u32,
    bytes: usize,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutostashConflictEvent {
    repo_id: String,
    paths: Vec<String>,
}

/// Rare edge case: the rebase itself completed fine, but re-applying the
/// changes autostash tucked away beforehand conflicted with it. Surfaced as
/// a toast via this event rather than folded into `RebaseStatus` — it's a
/// working-directory conflict, not a rebase-step conflict, so it doesn't fit
/// that shape and the existing stash-pop conflict handling already resolves
/// this the same way (manual fix, then stage + commit as usual).
fn emit_autostash_conflicts(
    repo_id: &str,
    conflicts: Vec<rebase::ConflictedFile>,
) {
    if conflicts.is_empty() {
        return;
    }
    crate::git_client::events::emit(
        "git://autostash-conflict",
        AutostashConflictEvent {
            repo_id: repo_id.to_string(),
            paths: conflicts.into_iter().map(|c| c.path).collect(),
        },
    );
}

/// Fetch progress as `git://fetch-progress` events, throttled to one per 100ms.
fn fetch_progress(
    repo_id: &str,
    background: bool,
) -> impl FnMut(usize, usize, usize) + Send + 'static {
    let emit_repo_id = repo_id.to_string();
    let mut last_emit = Instant::now();
    move |received, total, bytes| {
        if last_emit.elapsed().as_millis() >= 100 {
            crate::git_client::events::emit(
                "git://fetch-progress",
                FetchProgressEvent {
                    repo_id: emit_repo_id.clone(),
                    received_objects: received,
                    total_objects: total,
                    received_bytes: bytes,
                    background,
                },
            );
            last_emit = Instant::now();
        }
    }
}

/// Runs a slow read-only op (seconds, on a big repo) on its own handle of
/// the repo and thread, so it doesn't hold up every other git op of the
/// repo (polling, staging...) meanwhile.
fn off_actor<T: Send + 'static>(
    repo: &git2::Repository,
    reply: Reply<T>,
    op: impl FnOnce(&git2::Repository) -> Result<T, GitCommandError> + Send + 'static,
) {
    let path = repo.workdir().unwrap_or(repo.path()).to_path_buf();
    std::thread::spawn(move || {
        let result = git2::Repository::open(&path)
            .map_err(GitCommandError::from)
            .and_then(|repo| op(&repo));
        let _ = reply.send(result);
    });
}

/// A poisoned lock only means an earlier op panicked; the guarded state is `()`.
fn lock(m: &Mutex<()>) -> std::sync::MutexGuard<'_, ()> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Serialize, Deserialize)]
pub enum RepoCommand {
    Status(Reply<RepoStatus>),
    Log(
        LogQuery,
        Reply<CommitLogPage>,
    ),
    SearchCommits(
        String,
        SearchMode,
        Reply<Vec<CommitNode>>,
    ),
    Diff(String, Reply<CommitDiff>),
    RangeDiff(
        RangeDiffArgs,
        Reply<CommitDiff>,
    ),
    GetFileContent(
        FileContentArgs,
        Reply<String>,
    ),
    ListBranches(Reply<Vec<BranchInfo>>),
    ListTags(Reply<Vec<TagInfo>>),
    Tag(TagOp, Reply<Vec<String>>),
    CreateBranch(
        CreateBranchArgs,
        Reply<BranchInfo>,
    ),
    CheckoutBranch(String, Reply<RepoStatus>),
    CheckoutAt(
        CheckoutAtArgs,
        Reply<RepoStatus>,
    ),
    StartRebase(
        String,
        Reply<RebaseStatus>,
    ),
    GetRebaseStatus(Reply<Option<RebaseStatus>>),
    ResolveRebaseConflict(
        String,
        ConflictResolution,
        Reply<RebaseStatus>,
    ),
    ContinueRebase(Reply<RebaseStatus>),
    AbortRebase(Reply<()>),
    /// `None`: every remote.
    Fetch(
        Option<String>,
        AuthMode,
        Reply<FetchSummary>,
    ),
    Push(
        PushArgs,
        Reply<PushSummary>,
    ),
    Pull(
        Option<PullStrategy>,
        Reply<PullResult>,
    ),
    GetUpstreamStatus(Reply<Option<UpstreamStatus>>),
    StashSave(
        StashSaveArgs,
        Reply<StashInfo>,
    ),
    StashList(Reply<Vec<StashInfo>>),
    StashPop(
        usize,
        Reply<StashPopResult>,
    ),
    StashApply(
        usize,
        Reply<StashPopResult>,
    ),
    StashDrop(usize, Reply<()>),
    GetWorkingStatus(Reply<WorkingStatus>),
    StagePath(String, Reply<()>),
    StageAll(Reply<()>),
    UnstagePath(String, Reply<()>),
    UnstageAll(Reply<()>),
    DiscardPath(String, Reply<()>),
    DiscardAll(Reply<()>),
    ApplyLines(ApplyLinesArgs, Reply<()>),
    IgnorePattern(IgnoreArgs, Reply<bool>),
    GetWorkdirDiff(bool, Reply<CommitDiff>),
    CommitChanges(
        CommitChangesArgs,
        Reply<String>,
    ),
    AmendMessage(
        AmendMessageArgs,
        Reply<CommitNode>,
    ),
    StagedUnits(Reply<super::compose::StagedUnits>),
    ApplyCompose(ComposeArgs, Reply<Vec<String>>),
    GetHeadCommit(Reply<Option<CommitNode>>),
    GetCommitSignature(
        String,
        Reply<Option<signing::CommitSignature>>,
    ),
    GetStats(Reply<RepoStats>),
    GetReflog(String, Reply<Vec<ReflogEntry>>),
    CheckoutCommitDetached(String, Reply<RepoStatus>),
    CherryPick(
        CherryPickArgs,
        Reply<CherryPickResult>,
    ),
    ResetToCommit(
        ResetToCommitArgs,
        Reply<RepoStatus>,
    ),
    ResetToRef(
        ResetToRefArgs,
        Reply<RepoStatus>,
    ),
    SyncBranchToRemote(
        String,
        Reply<StashPopResult>,
    ),
    GetRemoteUrl(
        String,
        Reply<Option<String>>,
    ),
    ListRemotes(Reply<Vec<RemoteInfo>>),
    Remote(RemoteOp, Reply<Vec<RemoteInfo>>),
    DeleteLocalBranch(String, Reply<()>),
    DeleteRemoteBranch(
        DeleteRemoteBranchArgs,
        Reply<()>,
    ),
    DeleteBranchBoth(
        DeleteBranchEverywhereArgs,
        Reply<()>,
    ),
    MergeBranch(
        String,
        Reply<MergeResult>,
    ),
    Undo(Reply<String>),
    Redo(Reply<String>),
    GetUndoStatus(Reply<UndoStatus>),
    RevertCommit(
        RevertArgs,
        Reply<RevertResult>,
    ),
    FileHistory(
        FileHistoryQuery,
        Reply<FileHistoryPage>,
    ),
    CommitFileDiff(
        CommitFileDiffArgs,
        Reply<Option<FileDiff>>,
    ),
    Blame(BlameArgs, Reply<BlameChunk>),
    CompareRefs(
        CompareRefsArgs,
        Reply<BranchCompare>,
    ),
    TreeDiff(
        RangeDiffArgs,
        Reply<CommitDiff>,
    ),
    GetRebasePlan(String, Reply<RebasePlan>),
    StartInteractiveRebase(
        InteractiveRebaseArgs,
        Reply<RebaseStatus>,
    ),
    /// Every worktree op replies with the updated list.
    Worktree(WorktreeOp, Reply<Vec<WorktreeInfo>>),
    /// `Some(paths)`: update those first (all when empty); replies with the list.
    Submodules(
        Option<Vec<String>>,
        Reply<Vec<SubmoduleInfo>>,
    ),
    /// Just how many: parses `.gitmodules`, no status walk.
    SubmoduleCount(Reply<usize>),
    /// For work done off the actor thread (external processes that may run long).
    Workdir(Reply<PathBuf>),
}

#[derive(Serialize, Deserialize)]
pub struct InteractiveRebaseArgs {
    pub upstream: Option<String>,
    pub items: Vec<TodoItem>,
}

#[derive(Serialize, Deserialize)]
pub enum WorktreeOp {
    List,
    Add(AddWorktreeArgs),
    Remove { name: String, force: bool },
    Prune,
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

/// How a local, state-changing command shows up in the undo history, and
/// whether it continues an operation already in progress (rebase/merge
/// conflict handling). `None` for reads and for remote-only actions (fetch,
/// push, deleting a remote branch), which undo can't take back.
fn undo_label(cmd: &RepoCommand) -> Option<(String, bool)> {
    let label = match cmd {
        RepoCommand::CreateBranch(a, _) => format!("Criar branch {}", a.name),
        RepoCommand::CheckoutBranch(name, _) => format!("Checkout de {name}"),
        RepoCommand::CheckoutAt(a, _) => format!("Checkout de {} em {}", a.name, short(&a.target_sha)),
        RepoCommand::StartRebase(onto, _) => format!("Rebase sobre {onto}"),
        RepoCommand::ResolveRebaseConflict(..)
        | RepoCommand::ContinueRebase(_)
        | RepoCommand::AbortRebase(_) => return Some(("Resolver conflitos".into(), true)),
        RepoCommand::StashSave(..) => "Fazer stash das alterações".into(),
        RepoCommand::StashPop(i, _) => format!("Pop de stash@{{{i}}}"),
        RepoCommand::StashApply(i, _) => format!("Aplicar stash@{{{i}}}"),
        RepoCommand::StashDrop(i, _) => format!("Excluir stash@{{{i}}}"),
        RepoCommand::StagePath(path, _) => format!("Adicionar {path} ao staging"),
        RepoCommand::StageAll(_) => "Adicionar todas as alterações ao staging".into(),
        RepoCommand::UnstagePath(path, _) => format!("Remover {path} do staging"),
        RepoCommand::UnstageAll(_) => "Remover todas as alterações do staging".into(),
        RepoCommand::DiscardPath(path, _) => format!("Descartar alterações em {path}"),
        RepoCommand::DiscardAll(_) => "Descartar todas as alterações".into(),
        RepoCommand::ApplyLines(a, _) => match a.action {
            LineAction::Stage => format!("Adicionar linhas de {} ao staging", a.path),
            LineAction::Unstage => format!("Remover linhas de {} do staging", a.path),
            LineAction::Discard => format!("Descartar linhas em {}", a.path),
        },
        RepoCommand::CommitChanges(a, _) if a.amend => format!("Corrigir commit \"{}\"", a.summary),
        RepoCommand::CommitChanges(a, _) => format!("Commit \"{}\"", a.summary),
        RepoCommand::ApplyCompose(a, _) => format!("Compor {} commits", a.commits.len()),
        RepoCommand::AmendMessage(a, _) => format!("Reescrever mensagem de {}", short(&a.sha)),
        RepoCommand::CheckoutCommitDetached(sha, _) => format!("Checkout de {}", short(sha)),
        RepoCommand::CherryPick(a, _) => format!("Cherry-pick {}", short(&a.sha)),
        RepoCommand::ResetToCommit(a, _) => format!("Reset para {}", short(&a.sha)),
        RepoCommand::ResetToRef(a, _) => format!("Reset para {}", a.ref_name),
        RepoCommand::SyncBranchToRemote(..) => "Sincronizar branch com o remote".into(),
        RepoCommand::DeleteLocalBranch(name, _) => format!("Excluir branch {name}"),
        RepoCommand::DeleteBranchBoth(a, _) => format!("Excluir branch {} (cópia local)", a.branch),
        RepoCommand::MergeBranch(name, _) => format!("Merge de {name}"),
        // Desfazer devolve a branch ao ponto anterior; as refs remotas buscadas ficam.
        RepoCommand::Pull(..) => "Pull".into(),
        RepoCommand::RevertCommit(a, _) => format!("Reverter {}", short(&a.sha)),
        RepoCommand::StartInteractiveRebase(..) => "Rebase interativo".into(),
        _ => return None,
    };
    Some((label, false))
}

/// Tag operations; they reply with the remote's tag names for `ListRemote`
/// and an empty list otherwise. Not part of undo history (undo doesn't
/// snapshot tags).
#[derive(Serialize, Deserialize)]
pub enum TagOp {
    Create { name: String, sha: String },
    DeleteLocal(String),
    Push { remote: String, name: String },
    DeleteRemote { remote: String, name: String },
    ListRemote(String),
}

/// Remote configuration changes; they reply with the updated remote list.
/// Not part of undo history (undo doesn't snapshot config).
#[derive(Serialize, Deserialize)]
pub enum RemoteOp {
    Add { name: String, url: String },
    Remove(String),
    Rename { from: String, to: String },
    SetUrl { name: String, url: String },
}

#[derive(Serialize, Deserialize)]
pub struct RevertArgs {
    pub sha: String,
    pub mainline: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct CommitFileDiffArgs {
    pub sha: String,
    pub path: String,
    pub old_path: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct BlameArgs {
    pub path: String,
    pub sha: Option<String>,
    pub start_line: u32,
    pub max_lines: u32,
}

#[derive(Serialize, Deserialize)]
pub struct CompareRefsArgs {
    pub a: String,
    pub b: String,
    pub limit: u32,
}

#[derive(Serialize, Deserialize)]
pub struct DeleteRemoteBranchArgs {
    pub remote_name: String,
    pub branch: String,
}

#[derive(Serialize, Deserialize)]
pub struct DeleteBranchEverywhereArgs {
    pub remote_names: Vec<String>,
    pub branch: String,
}

#[derive(Serialize, Deserialize)]
pub struct ResetToCommitArgs {
    pub sha: String,
    pub hard: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ResetToRefArgs {
    pub ref_name: String,
    pub hard: bool,
}

#[derive(Serialize, Deserialize)]
pub struct CherryPickArgs {
    pub sha: String,
    pub create_commit: bool,
}

#[derive(Serialize, Deserialize)]
pub struct CommitChangesArgs {
    pub summary: String,
    pub body: String,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub amend: bool,
    pub no_verify: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ComposeArgs {
    pub tree_id: String,
    pub commits: Vec<super::compose::ComposedCommitInput>,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub no_verify: bool,
}

#[derive(Serialize, Deserialize)]
pub struct AmendMessageArgs {
    pub sha: String,
    pub summary: String,
    pub body: String,
    pub no_verify: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ApplyLinesArgs {
    pub path: String,
    pub action: LineAction,
    pub selection: LineSelection,
}

#[derive(Serialize, Deserialize)]
pub struct IgnoreArgs {
    pub pattern: String,
    pub local: bool,
}

#[derive(Serialize, Deserialize)]
pub struct RangeDiffArgs {
    pub from_sha: String,
    pub to_sha: String,
}

#[derive(Serialize, Deserialize)]
pub struct FileContentArgs {
    pub sha: Option<String>,
    pub path: String,
}

#[derive(Serialize, Deserialize)]
pub struct StashSaveArgs {
    pub message: Option<String>,
    pub include_untracked: bool,
}

#[derive(Serialize, Deserialize)]
pub struct CreateBranchArgs {
    pub name: String,
    pub start_point: Option<String>,
    pub checkout: bool,
}

#[derive(Serialize, Deserialize)]
pub struct CheckoutAtArgs {
    pub name: String,
    pub target_sha: String,
    pub remote_name: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct PushArgs {
    /// `None`: where `git push` would go (see `remote::push_remote_for`).
    pub remote_name: Option<String>,
    pub branch: String,
    pub force: bool,
}

#[derive(Clone)]
pub enum RepoActor {
    Local { tx: mpsc::Sender<RepoCommand> },
    /// Lives in a `githydra-agent` inside WSL; commands cross its pipe.
    Remote(RemoteRepo),
}

impl RepoActor {
    pub fn spawn(
        mut repo: git2::Repository,
        repo_id: String,
    ) -> Self {
        let (tx, rx) = mpsc::channel::<RepoCommand>();
        std::thread::spawn(move || {
            let mut history = UndoHistory::default();
            let mut log_cache = log::LogCache::default();
            let blame_cache = history::BlameCache::default();
            // Held by fetch, push and pull: they update the same remote-tracking refs.
            let network = Arc::new(Mutex::new(()));
            for cmd in rx {
                // Snapshot around every local mutation so it can be undone.
                // A failed snapshot just means this action isn't undoable.
                let recording = undo_label(&cmd)
                    .and_then(|(label, cont)| Some((label, cont, undo::capture(&mut repo).ok()?)));
                match cmd {
                    RepoCommand::Undo(reply) => {
                        let _ = reply.send(history.undo(&mut repo));
                    }
                    RepoCommand::Redo(reply) => {
                        let _ = reply.send(history.redo(&mut repo));
                    }
                    RepoCommand::GetUndoStatus(reply) => {
                        let _ = reply.send(Ok(history.status()));
                    }
                    RepoCommand::Status(reply) => {
                        let _ = reply.send(repo::status(&repo));
                    }
                    RepoCommand::Log(query, reply) => {
                        let _ = reply.send(log::commit_log_cached(&repo, &query, &mut log_cache));
                    }
                    RepoCommand::SearchCommits(query, mode, reply) => {
                        let _ = reply.send(search::search_commits(&repo, &query, mode, 200));
                    }
                    RepoCommand::Diff(sha, reply) => {
                        let _ = reply.send(diff::commit_diff(&repo, &sha));
                    }
                    RepoCommand::RangeDiff(args, reply) => {
                        let _ = reply.send(diff::range_diff(&repo, &args.from_sha, &args.to_sha));
                    }
                    RepoCommand::GetFileContent(args, reply) => {
                        let _ = reply.send(diff::read_file_base64(
                            &repo,
                            args.sha.as_deref(),
                            &args.path,
                        ));
                    }
                    RepoCommand::ListBranches(reply) => {
                        let _ = reply.send(branch::list_branches(&repo));
                    }
                    RepoCommand::ListTags(reply) => {
                        let _ = reply.send(branch::list_tags(&repo));
                    }
                    // A network round trip with no timeout: on the actor thread an
                    // unresponsive server froze every other command of the repo.
                    RepoCommand::Tag(TagOp::ListRemote(remote), reply) => {
                        off_actor(&repo, reply, move |repo| remote::list_remote_tags(repo, &remote));
                    }
                    RepoCommand::Tag(op, reply) => {
                        let result = match op {
                            TagOp::Create { name, sha } => {
                                branch::create_tag(&repo, &name, &sha).map(|_| Vec::new())
                            }
                            TagOp::DeleteLocal(name) => {
                                branch::delete_local_tag(&repo, &name).map(|_| Vec::new())
                            }
                            TagOp::Push { remote, name } => {
                                remote::push_tag(&repo, &remote, &name).map(|_| Vec::new())
                            }
                            TagOp::DeleteRemote { remote, name } => {
                                remote::delete_remote_tag(&repo, &remote, &name).map(|_| Vec::new())
                            }
                            TagOp::ListRemote(remote) => remote::list_remote_tags(&repo, &remote),
                        };
                        let _ = reply.send(result);
                    }
                    RepoCommand::CreateBranch(args, reply) => {
                        let _ = reply.send(branch::create_branch(
                            &repo,
                            &args.name,
                            args.start_point.as_deref(),
                            args.checkout,
                        ));
                    }
                    RepoCommand::CheckoutBranch(name, reply) => {
                        let result =
                            branch::checkout_branch(&repo, &name).and_then(|_| repo::status(&repo));
                        let _ = reply.send(result);
                    }
                    RepoCommand::CheckoutAt(args, reply) => {
                        let result = branch::checkout_at(
                            &repo,
                            &args.name,
                            &args.target_sha,
                            args.remote_name.as_deref(),
                        )
                        .and_then(|_| repo::status(&repo));
                        let _ = reply.send(result);
                    }
                    // Rebase sessions are never kept in memory across commands: `Rebase<'_>`
                    // borrows `repo`, and holding that borrow across loop iterations would
                    // permanently block the `&mut repo` stash operations need. libgit2
                    // already persists rebase state to `.git/rebase-merge` between calls, so
                    // each command just reopens it fresh via `repo.open_rebase(None)`.
                    RepoCommand::StartRebase(onto, reply) => {
                        let result = if rebase::is_rebase_in_progress(&repo) {
                            Err(GitCommandError::Internal(
                                "já há um rebase em andamento neste repositório".into(),
                            ))
                        } else {
                            (|| -> Result<RebaseStatus, GitCommandError> {
                                if rebase::has_uncommitted_changes(&repo)? {
                                    rebase::autostash(&mut repo)?;
                                }
                                let status =
                                    rebase::start(&repo, &onto).map(|(_session, status)| status)?;
                                if matches!(status.state, rebase::RebaseState::Complete) {
                                    let conflicts = rebase::pop_autostash_if_any(&mut repo)?;
                                    emit_autostash_conflicts(&repo_id, conflicts);
                                }
                                Ok(status)
                            })()
                        };
                        let _ = reply.send(result);
                    }
                    RepoCommand::GetRebaseStatus(reply) => {
                        let _ = reply.send(rebase::read_status(&repo));
                    }
                    RepoCommand::ResolveRebaseConflict(path, resolution, reply) => {
                        let result =
                            rebase::resolve_conflict(&repo, &path, resolution).and_then(|_| {
                                // `None` here just means nothing is conflicted anymore and
                                // there's no rebase session to report on (e.g. this conflict
                                // came from a stash apply, not a rebase) — that's success,
                                // not an error condition.
                                Ok(rebase::read_status(&repo)?.unwrap_or(RebaseStatus {
                                    state: rebase::RebaseState::Complete,
                                    current_step: 0,
                                    total_steps: 0,
                                    conflicts: Vec::new(),
                                }))
                            });
                        let _ = reply.send(result);
                    }
                    RepoCommand::ContinueRebase(reply) => {
                        let result = (|| -> Result<RebaseStatus, GitCommandError> {
                            // Checked before `open_rebase`, which would otherwise hold an
                            // immutable borrow of `repo` across this whole match, conflicting
                            // with `continue_merge`'s need for a mutable one.
                            if matches!(
                                repo.state(),
                                git2::RepositoryState::Revert | git2::RepositoryState::RevertSequence
                            ) {
                                revert::continue_revert(&repo)?;
                                return Ok(RebaseStatus {
                                    state: rebase::RebaseState::Complete,
                                    current_step: 0,
                                    total_steps: 0,
                                    conflicts: Vec::new(),
                                });
                            }
                            if repo.state() == git2::RepositoryState::Merge {
                                merge::continue_merge(&mut repo)?;
                                // A pull that merged may have autostashed first.
                                let conflicts = rebase::pop_autostash_if_any(&mut repo)?;
                                emit_autostash_conflicts(&repo_id, conflicts);
                                return Ok(RebaseStatus {
                                    state: rebase::RebaseState::Complete,
                                    current_step: 0,
                                    total_steps: 0,
                                    conflicts: Vec::new(),
                                });
                            }
                            let status = match repo.open_rebase(None) {
                                Ok(mut session) => rebase::advance(&repo, &mut session, true)?,
                                // Interactive rebases (or any other state libgit2 refuses to
                                // open) can't be advanced through the `Rebase` API at all —
                                // shell out to the real `git` binary instead.
                                Err(_) if rebase::is_rebase_in_progress(&repo) => {
                                    let workdir = repo.workdir().ok_or_else(|| {
                                        GitCommandError::Internal(
                                            "repositório bare não tem diretório de trabalho".into(),
                                        )
                                    })?;
                                    let env = signing::git_cli_env(&repo)?;
                                    let continued = external_git::rebase_continue(workdir, &env);
                                    interactive_rebase::after_git(&repo);
                                    continued?;
                                    rebase::read_status(&repo)?.unwrap_or(RebaseStatus {
                                        state: rebase::RebaseState::Complete,
                                        current_step: 0,
                                        total_steps: 0,
                                        conflicts: Vec::new(),
                                    })
                                }
                                Err(_) => {
                                    return Err(GitCommandError::Internal(
                                        "no rebase in progress".into(),
                                    ))
                                }
                            };
                            if matches!(status.state, rebase::RebaseState::Complete) {
                                let conflicts = rebase::pop_autostash_if_any(&mut repo)?;
                                emit_autostash_conflicts(&repo_id, conflicts);
                            }
                            Ok(status)
                        })();
                        let _ = reply.send(result);
                    }
                    RepoCommand::AbortRebase(reply) => {
                        // Idempotent: nothing to abort is not an error, the panel just dismisses.
                        let result = (|| -> Result<(), GitCommandError> {
                            match repo.open_rebase(None) {
                                Ok(mut session) => rebase::abort(&mut session)?,
                                Err(_) if rebase::is_rebase_in_progress(&repo) => {
                                    if let Some(workdir) = repo.workdir() {
                                        external_git::rebase_abort(workdir)?;
                                    }
                                    interactive_rebase::after_git(&repo);
                                }
                                // Merge, cherry-pick and revert don't move HEAD until
                                // their final commit: aborting is restoring HEAD's tree.
                                Err(_)
                                    if matches!(
                                        repo.state(),
                                        git2::RepositoryState::Merge
                                            | git2::RepositoryState::CherryPick
                                            | git2::RepositoryState::CherryPickSequence
                                            | git2::RepositoryState::Revert
                                            | git2::RepositoryState::RevertSequence
                                    ) =>
                                {
                                    merge::abort_merge(&repo)?;
                                }
                                Err(_) => {}
                            }
                            let conflicts = rebase::pop_autostash_if_any(&mut repo)?;
                            emit_autostash_conflicts(&repo_id, conflicts);
                            Ok(())
                        })();
                        let _ = reply.send(result);
                    }
                    // Network round trips (seconds, or a passphrase prompt) run off
                    // the actor: a periodic background fetch must not stall
                    // every other op of the repo while it waits on the remote.
                    RepoCommand::Fetch(remote_name, mode, reply) => {
                        let progress =
                            fetch_progress(&repo_id, mode == AuthMode::Background);
                        let network = network.clone();
                        off_actor(&repo, reply, move |repo| {
                            let _one_at_a_time = lock(&network);
                            remote::fetch_remotes(repo, remote_name.as_deref(), mode, progress)
                        });
                    }
                    RepoCommand::Pull(choice, reply) => {
                        let progress = fetch_progress(&repo_id, false);
                        // Pull fetches too, and then must see that fetch's refs.
                        let _one_at_a_time = lock(&network);
                        // Autostash conflicts ride in the result: the UI opens the
                        // conflict panel for them right away.
                        let _ = reply.send(pull::pull(&mut repo, choice, progress));
                    }
                    RepoCommand::GetUpstreamStatus(reply) => {
                        let _ = reply.send(pull::upstream_status(&repo));
                    }
                    RepoCommand::Push(args, reply) => {
                        let emit_repo_id = repo_id.clone();
                        let network = network.clone();
                        off_actor(&repo, reply, move |repo| {
                            let _one_at_a_time = lock(&network);
                            let mut last_emit = Instant::now();
                            let remote_name = match args.remote_name {
                                Some(name) => name,
                                None => remote::push_remote_for(repo, &args.branch)?,
                            };
                            remote::push(
                                repo,
                                &remote_name,
                                &args.branch,
                                args.force,
                                move |current, total, bytes| {
                                    if last_emit.elapsed().as_millis() >= 100 || current == total {
                                        crate::git_client::events::emit(
                                            "git://push-progress",
                                            PushProgressEvent {
                                                repo_id: emit_repo_id.clone(),
                                                current,
                                                total,
                                                bytes,
                                            },
                                        );
                                        last_emit = Instant::now();
                                    }
                                },
                            )
                        });
                    }
                    RepoCommand::StashSave(args, reply) => {
                        let _ = reply.send(stash::save(
                            &mut repo,
                            args.message.as_deref(),
                            args.include_untracked,
                        ));
                    }
                    RepoCommand::StashList(reply) => {
                        let _ = reply.send(stash::list(&mut repo));
                    }
                    RepoCommand::StashPop(index, reply) => {
                        let _ = reply.send(stash::pop(&mut repo, index));
                    }
                    RepoCommand::StashApply(index, reply) => {
                        let _ = reply.send(stash::apply(&mut repo, index));
                    }
                    RepoCommand::StashDrop(index, reply) => {
                        let _ = reply.send(stash::drop(&mut repo, index));
                    }
                    RepoCommand::GetWorkingStatus(reply) => {
                        let _ = reply.send(working::working_status(&repo));
                    }
                    RepoCommand::StagePath(path, reply) => {
                        let _ = reply.send(working::stage_path(&repo, &path));
                    }
                    RepoCommand::StageAll(reply) => {
                        let _ = reply.send(working::stage_all(&repo));
                    }
                    RepoCommand::UnstagePath(path, reply) => {
                        let _ = reply.send(working::unstage_path(&repo, &path));
                    }
                    RepoCommand::UnstageAll(reply) => {
                        let _ = reply.send(working::unstage_all(&repo));
                    }
                    RepoCommand::DiscardPath(path, reply) => {
                        let _ = reply.send(working::discard_path(&repo, &path));
                    }
                    RepoCommand::DiscardAll(reply) => {
                        let _ = reply.send(working::discard_all(&repo));
                    }
                    RepoCommand::ApplyLines(args, reply) => {
                        let _ = reply.send(partial::apply_lines(
                            &repo,
                            &args.path,
                            args.action,
                            &args.selection,
                        ));
                    }
                    RepoCommand::IgnorePattern(args, reply) => {
                        let _ = reply.send(super::ignore::add_pattern(&repo, &args.pattern, args.local));
                    }
                    RepoCommand::GetWorkdirDiff(staged, reply) => {
                        let _ = reply.send(diff::workdir_diff(&repo, staged));
                    }
                    RepoCommand::CommitChanges(args, reply) => {
                        let _ = reply.send(working::commit(
                            &repo,
                            &args.summary,
                            &args.body,
                            args.author_name.as_deref(),
                            args.author_email.as_deref(),
                            args.amend,
                            args.no_verify,
                        ));
                    }
                    RepoCommand::StagedUnits(reply) => {
                        let _ = reply.send(super::compose::staged_units(&repo));
                    }
                    RepoCommand::ApplyCompose(args, reply) => {
                        let _ = reply.send(super::compose::apply(
                            &repo,
                            &args.tree_id,
                            &args.commits,
                            args.author_name.as_deref(),
                            args.author_email.as_deref(),
                            args.no_verify,
                        ));
                    }
                    RepoCommand::AmendMessage(args, reply) => {
                        let _ = reply.send(working::amend_message(
                            &repo,
                            &args.sha,
                            &args.summary,
                            &args.body,
                            args.no_verify,
                        ));
                    }
                    RepoCommand::GetReflog(ref_name, reply) => {
                        let _ = reply.send(reflog::reflog(&repo, &ref_name));
                    }
                    RepoCommand::GetStats(reply) => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);
                        let _ = reply.send(stats::repo_stats(&repo, now));
                    }
                    RepoCommand::GetHeadCommit(reply) => {
                        let _ = reply.send(log::head_commit(&repo));
                    }
                    RepoCommand::GetCommitSignature(sha, reply) => {
                        let _ = reply.send(signing::commit_signature(&repo, &sha));
                    }
                    RepoCommand::CheckoutCommitDetached(sha, reply) => {
                        let result = branch::checkout_detached(&repo, &sha)
                            .and_then(|_| repo::status(&repo));
                        let _ = reply.send(result);
                    }
                    RepoCommand::CherryPick(args, reply) => {
                        let _ = reply.send(cherry_pick::cherry_pick(
                            &mut repo,
                            &args.sha,
                            args.create_commit,
                        ));
                    }
                    RepoCommand::ResetToCommit(args, reply) => {
                        let result = branch::reset_to_commit(&repo, &args.sha, args.hard)
                            .and_then(|_| repo::status(&repo));
                        let _ = reply.send(result);
                    }
                    RepoCommand::ResetToRef(args, reply) => {
                        let result = branch::reset_to_ref(&repo, &args.ref_name, args.hard)
                            .and_then(|_| repo::status(&repo));
                        let _ = reply.send(result);
                    }
                    RepoCommand::SyncBranchToRemote(target_sha, reply) => {
                        let _ = reply.send(branch::sync_branch_to_commit(&mut repo, &target_sha));
                    }
                    RepoCommand::GetRemoteUrl(remote_name, reply) => {
                        let _ = reply.send(remote::get_remote_url(&repo, &remote_name));
                    }
                    RepoCommand::ListRemotes(reply) => {
                        let _ = reply.send(remote::list_remotes(&repo));
                    }
                    RepoCommand::Remote(op, reply) => {
                        let result = match op {
                            RemoteOp::Add { name, url } => remote::add_remote(&repo, &name, &url),
                            RemoteOp::Remove(name) => remote::remove_remote(&repo, &name),
                            RemoteOp::Rename { from, to } => remote::rename_remote(&repo, &from, &to),
                            RemoteOp::SetUrl { name, url } => remote::set_remote_url(&repo, &name, &url),
                        };
                        let _ = reply.send(result.and_then(|_| remote::list_remotes(&repo)));
                    }
                    RepoCommand::DeleteLocalBranch(name, reply) => {
                        let _ = reply.send(branch::delete_local_branch(&repo, &name));
                    }
                    RepoCommand::DeleteRemoteBranch(args, reply) => {
                        let _ = reply.send(remote::delete_remote_branch(
                            &repo,
                            &args.remote_name,
                            &args.branch,
                        ));
                    }
                    RepoCommand::DeleteBranchBoth(args, reply) => {
                        let _ = reply.send(remote::delete_branch_everywhere(
                            &repo,
                            &args.remote_names,
                            &args.branch,
                        ));
                    }
                    RepoCommand::MergeBranch(branch_name, reply) => {
                        let _ = reply.send(merge::merge_branch(&repo, &branch_name));
                    }
                    RepoCommand::RevertCommit(args, reply) => {
                        let _ = reply.send(revert::revert_commit(&repo, &args.sha, args.mainline));
                    }
                    RepoCommand::FileHistory(query, reply) => {
                        let _ = reply.send(history::file_history(&repo, &query));
                    }
                    RepoCommand::CommitFileDiff(args, reply) => {
                        let _ = reply.send(history::commit_file_diff(
                            &repo,
                            &args.sha,
                            &args.path,
                            args.old_path.as_deref(),
                        ));
                    }
                    RepoCommand::Blame(args, reply) => {
                        let cache = blame_cache.clone();
                        off_actor(&repo, reply, move |repo| {
                            history::blame(repo, &args.path, args.sha.as_deref(), args.start_line, args.max_lines, &cache)
                        });
                    }
                    RepoCommand::CompareRefs(args, reply) => {
                        let _ = reply.send(history::compare_refs(&repo, &args.a, &args.b, args.limit));
                    }
                    RepoCommand::TreeDiff(args, reply) => {
                        let _ = reply.send(history::tree_diff(&repo, &args.from_sha, &args.to_sha));
                    }
                    RepoCommand::GetRebasePlan(from_sha, reply) => {
                        let _ = reply.send(interactive_rebase::plan(&repo, &from_sha));
                    }
                    RepoCommand::StartInteractiveRebase(args, reply) => {
                        let _ = reply.send(interactive_rebase::start(
                            &repo,
                            args.upstream.as_deref(),
                            &args.items,
                        ));
                    }
                    RepoCommand::Worktree(op, reply) => {
                        let result = match op {
                            WorktreeOp::List => Ok(()),
                            WorktreeOp::Add(args) => worktree::add(&repo, &args),
                            WorktreeOp::Remove { name, force } => {
                                worktree::remove(&repo, &name, force)
                            }
                            WorktreeOp::Prune => worktree::prune(&repo).map(|_| ()),
                        };
                        let _ = reply.send(result.and_then(|_| worktree::list(&repo)));
                    }
                    RepoCommand::Submodules(update, reply) => {
                        let result = match update {
                            Some(paths) => submodule::update(&repo, &paths),
                            None => Ok(()),
                        };
                        let _ = reply.send(result.and_then(|_| submodule::list(&repo)));
                    }
                    RepoCommand::SubmoduleCount(reply) => {
                        let _ = reply.send(repo.submodules().map(|s| s.len()).map_err(Into::into));
                    }
                    RepoCommand::Workdir(reply) => {
                        let _ = reply.send(repo.workdir().map(Path::to_path_buf).ok_or_else(|| {
                            GitCommandError::Internal("repositório bare não tem diretório de trabalho".into())
                        }));
                    }
                }
                if let Some((label, continuation, before)) = recording {
                    if let Ok(after) = undo::capture(&mut repo) {
                        history.record(label, before, after, continuation);
                    }
                }
            }
        });
        Self::Local { tx }
    }

    /// Queues a command decoded from the wire (agent side; local actors only).
    pub fn send(&self, cmd: RepoCommand) -> Result<(), GitCommandError> {
        match self {
            Self::Local { tx } => tx
                .send(cmd)
                .map_err(|_| GitCommandError::Internal("repo actor is no longer running".into())),
            Self::Remote(_) => Err(GitCommandError::Internal("cannot forward a command twice".into())),
        }
    }

    async fn dispatch<T: Serialize + DeserializeOwned + Send + 'static>(
        &self,
        make_cmd: impl FnOnce(Reply<T>) -> RepoCommand,
    ) -> Result<T, GitCommandError> {
        let (reply, reply_rx) = Reply::channel();
        let cmd = make_cmd(reply);
        let tx = match self {
            Self::Local { tx } => tx,
            Self::Remote(remote) => return remote.call(&cmd).await,
        };
        tx.send(cmd)
            .map_err(|_| GitCommandError::Internal("o processo do repositório não está mais em execução".into()))?;
        reply_rx
            .await
            .map_err(|_| GitCommandError::Internal("o processo do repositório não respondeu".into()))?
    }

    pub async fn undo(&self) -> Result<String, GitCommandError> {
        self.dispatch(RepoCommand::Undo).await
    }

    pub async fn redo(&self) -> Result<String, GitCommandError> {
        self.dispatch(RepoCommand::Redo).await
    }

    pub async fn undo_status(&self) -> Result<UndoStatus, GitCommandError> {
        self.dispatch(RepoCommand::GetUndoStatus).await
    }

    pub async fn status(&self) -> Result<RepoStatus, GitCommandError> {
        self.dispatch(RepoCommand::Status).await
    }

    pub async fn log(&self, query: LogQuery) -> Result<CommitLogPage, GitCommandError> {
        self.dispatch(|tx| RepoCommand::Log(query, tx)).await
    }

    pub async fn search_commits(
        &self,
        query: String,
        mode: SearchMode,
    ) -> Result<Vec<CommitNode>, GitCommandError> {
        self.dispatch(|tx| RepoCommand::SearchCommits(query, mode, tx)).await
    }

    pub async fn diff(&self, sha: String) -> Result<CommitDiff, GitCommandError> {
        self.dispatch(|tx| RepoCommand::Diff(sha, tx)).await
    }

    pub async fn range_diff(&self, args: RangeDiffArgs) -> Result<CommitDiff, GitCommandError> {
        self.dispatch(|tx| RepoCommand::RangeDiff(args, tx)).await
    }

    pub async fn file_content(&self, args: FileContentArgs) -> Result<String, GitCommandError> {
        self.dispatch(|tx| RepoCommand::GetFileContent(args, tx))
            .await
    }

    pub async fn list_branches(&self) -> Result<Vec<BranchInfo>, GitCommandError> {
        self.dispatch(RepoCommand::ListBranches).await
    }

    pub async fn tag(&self, op: TagOp) -> Result<Vec<String>, GitCommandError> {
        self.dispatch(|tx| RepoCommand::Tag(op, tx)).await
    }

    pub async fn list_tags(&self) -> Result<Vec<TagInfo>, GitCommandError> {
        self.dispatch(RepoCommand::ListTags).await
    }

    pub async fn create_branch(
        &self,
        args: CreateBranchArgs,
    ) -> Result<BranchInfo, GitCommandError> {
        self.dispatch(|tx| RepoCommand::CreateBranch(args, tx))
            .await
    }

    pub async fn checkout_branch(&self, name: String) -> Result<RepoStatus, GitCommandError> {
        self.dispatch(|tx| RepoCommand::CheckoutBranch(name, tx))
            .await
    }

    pub async fn checkout_at(&self, args: CheckoutAtArgs) -> Result<RepoStatus, GitCommandError> {
        self.dispatch(|tx| RepoCommand::CheckoutAt(args, tx)).await
    }

    pub async fn start_rebase(&self, onto_branch: String) -> Result<RebaseStatus, GitCommandError> {
        self.dispatch(|tx| RepoCommand::StartRebase(onto_branch, tx))
            .await
    }

    pub async fn rebase_status(&self) -> Result<Option<RebaseStatus>, GitCommandError> {
        self.dispatch(RepoCommand::GetRebaseStatus).await
    }

    pub async fn resolve_rebase_conflict(
        &self,
        path: String,
        resolution: ConflictResolution,
    ) -> Result<RebaseStatus, GitCommandError> {
        self.dispatch(|tx| RepoCommand::ResolveRebaseConflict(path, resolution, tx))
            .await
    }

    pub async fn continue_rebase(&self) -> Result<RebaseStatus, GitCommandError> {
        self.dispatch(RepoCommand::ContinueRebase).await
    }

    pub async fn abort_rebase(&self) -> Result<(), GitCommandError> {
        self.dispatch(RepoCommand::AbortRebase).await
    }

    pub async fn fetch(
        &self,
        remote_name: Option<String>,
        mode: AuthMode,
    ) -> Result<FetchSummary, GitCommandError> {
        self.dispatch(|tx| RepoCommand::Fetch(remote_name, mode, tx))
            .await
    }

    pub async fn push(&self, args: PushArgs) -> Result<PushSummary, GitCommandError> {
        self.dispatch(|tx| RepoCommand::Push(args, tx)).await
    }

    pub async fn pull(&self, choice: Option<PullStrategy>) -> Result<PullResult, GitCommandError> {
        self.dispatch(|tx| RepoCommand::Pull(choice, tx)).await
    }

    pub async fn upstream_status(&self) -> Result<Option<UpstreamStatus>, GitCommandError> {
        self.dispatch(RepoCommand::GetUpstreamStatus).await
    }

    pub async fn stash_save(&self, args: StashSaveArgs) -> Result<StashInfo, GitCommandError> {
        self.dispatch(|tx| RepoCommand::StashSave(args, tx)).await
    }

    pub async fn stash_list(&self) -> Result<Vec<StashInfo>, GitCommandError> {
        self.dispatch(RepoCommand::StashList).await
    }

    pub async fn stash_pop(&self, index: usize) -> Result<StashPopResult, GitCommandError> {
        self.dispatch(|tx| RepoCommand::StashPop(index, tx)).await
    }

    pub async fn stash_apply(&self, index: usize) -> Result<StashPopResult, GitCommandError> {
        self.dispatch(|tx| RepoCommand::StashApply(index, tx)).await
    }

    pub async fn stash_drop(&self, index: usize) -> Result<(), GitCommandError> {
        self.dispatch(|tx| RepoCommand::StashDrop(index, tx)).await
    }

    pub async fn working_status(&self) -> Result<WorkingStatus, GitCommandError> {
        self.dispatch(RepoCommand::GetWorkingStatus).await
    }

    pub async fn stage_path(&self, path: String) -> Result<(), GitCommandError> {
        self.dispatch(|tx| RepoCommand::StagePath(path, tx)).await
    }

    pub async fn stage_all(&self) -> Result<(), GitCommandError> {
        self.dispatch(RepoCommand::StageAll).await
    }

    pub async fn unstage_path(&self, path: String) -> Result<(), GitCommandError> {
        self.dispatch(|tx| RepoCommand::UnstagePath(path, tx)).await
    }

    pub async fn unstage_all(&self) -> Result<(), GitCommandError> {
        self.dispatch(RepoCommand::UnstageAll).await
    }

    pub async fn discard_path(&self, path: String) -> Result<(), GitCommandError> {
        self.dispatch(|tx| RepoCommand::DiscardPath(path, tx)).await
    }

    pub async fn discard_all(&self) -> Result<(), GitCommandError> {
        self.dispatch(RepoCommand::DiscardAll).await
    }

    pub async fn apply_lines(&self, args: ApplyLinesArgs) -> Result<(), GitCommandError> {
        self.dispatch(|tx| RepoCommand::ApplyLines(args, tx)).await
    }

    pub async fn ignore_pattern(&self, args: IgnoreArgs) -> Result<bool, GitCommandError> {
        self.dispatch(|tx| RepoCommand::IgnorePattern(args, tx)).await
    }

    pub async fn workdir_diff(&self, staged: bool) -> Result<CommitDiff, GitCommandError> {
        self.dispatch(|tx| RepoCommand::GetWorkdirDiff(staged, tx))
            .await
    }

    pub async fn commit_changes(&self, args: CommitChangesArgs) -> Result<String, GitCommandError> {
        self.dispatch(|tx| RepoCommand::CommitChanges(args, tx))
            .await
    }

    pub async fn staged_units(&self) -> Result<super::compose::StagedUnits, GitCommandError> {
        self.dispatch(RepoCommand::StagedUnits).await
    }

    pub async fn apply_compose(&self, args: ComposeArgs) -> Result<Vec<String>, GitCommandError> {
        self.dispatch(|tx| RepoCommand::ApplyCompose(args, tx)).await
    }

    pub async fn amend_message(
        &self,
        args: AmendMessageArgs,
    ) -> Result<CommitNode, GitCommandError> {
        self.dispatch(|tx| RepoCommand::AmendMessage(args, tx))
            .await
    }

    pub async fn head_commit(&self) -> Result<Option<CommitNode>, GitCommandError> {
        self.dispatch(RepoCommand::GetHeadCommit).await
    }

    pub async fn commit_signature(
        &self,
        sha: String,
    ) -> Result<Option<signing::CommitSignature>, GitCommandError> {
        self.dispatch(|tx| RepoCommand::GetCommitSignature(sha, tx)).await
    }

    pub async fn stats(&self) -> Result<RepoStats, GitCommandError> {
        self.dispatch(RepoCommand::GetStats).await
    }

    pub async fn reflog(&self, ref_name: String) -> Result<Vec<ReflogEntry>, GitCommandError> {
        self.dispatch(|tx| RepoCommand::GetReflog(ref_name, tx)).await
    }

    pub async fn checkout_commit_detached(
        &self,
        sha: String,
    ) -> Result<RepoStatus, GitCommandError> {
        self.dispatch(|tx| RepoCommand::CheckoutCommitDetached(sha, tx))
            .await
    }

    pub async fn cherry_pick(
        &self,
        args: CherryPickArgs,
    ) -> Result<CherryPickResult, GitCommandError> {
        self.dispatch(|tx| RepoCommand::CherryPick(args, tx)).await
    }

    pub async fn reset_to_commit(
        &self,
        args: ResetToCommitArgs,
    ) -> Result<RepoStatus, GitCommandError> {
        self.dispatch(|tx| RepoCommand::ResetToCommit(args, tx))
            .await
    }

    pub async fn reset_to_ref(&self, args: ResetToRefArgs) -> Result<RepoStatus, GitCommandError> {
        self.dispatch(|tx| RepoCommand::ResetToRef(args, tx)).await
    }

    pub async fn sync_branch_to_remote(
        &self,
        target_sha: String,
    ) -> Result<StashPopResult, GitCommandError> {
        self.dispatch(|tx| RepoCommand::SyncBranchToRemote(target_sha, tx))
            .await
    }

    pub async fn get_remote_url(
        &self,
        remote_name: String,
    ) -> Result<Option<String>, GitCommandError> {
        self.dispatch(|tx| RepoCommand::GetRemoteUrl(remote_name, tx))
            .await
    }

    pub async fn list_remotes(&self) -> Result<Vec<RemoteInfo>, GitCommandError> {
        self.dispatch(RepoCommand::ListRemotes).await
    }

    pub async fn remote(&self, op: RemoteOp) -> Result<Vec<RemoteInfo>, GitCommandError> {
        self.dispatch(|tx| RepoCommand::Remote(op, tx)).await
    }

    pub async fn delete_local_branch(&self, name: String) -> Result<(), GitCommandError> {
        self.dispatch(|tx| RepoCommand::DeleteLocalBranch(name, tx))
            .await
    }

    pub async fn delete_remote_branch(
        &self,
        args: DeleteRemoteBranchArgs,
    ) -> Result<(), GitCommandError> {
        self.dispatch(|tx| RepoCommand::DeleteRemoteBranch(args, tx))
            .await
    }

    pub async fn delete_branch_both(
        &self,
        args: DeleteBranchEverywhereArgs,
    ) -> Result<(), GitCommandError> {
        self.dispatch(|tx| RepoCommand::DeleteBranchBoth(args, tx))
            .await
    }

    pub async fn merge_branch(&self, branch_name: String) -> Result<MergeResult, GitCommandError> {
        self.dispatch(|tx| RepoCommand::MergeBranch(branch_name, tx))
            .await
    }

    pub async fn revert_commit(&self, args: RevertArgs) -> Result<RevertResult, GitCommandError> {
        self.dispatch(|tx| RepoCommand::RevertCommit(args, tx)).await
    }

    pub async fn file_history(
        &self,
        query: FileHistoryQuery,
    ) -> Result<FileHistoryPage, GitCommandError> {
        self.dispatch(|tx| RepoCommand::FileHistory(query, tx)).await
    }

    pub async fn commit_file_diff(
        &self,
        args: CommitFileDiffArgs,
    ) -> Result<Option<FileDiff>, GitCommandError> {
        self.dispatch(|tx| RepoCommand::CommitFileDiff(args, tx)).await
    }

    pub async fn blame(&self, args: BlameArgs) -> Result<BlameChunk, GitCommandError> {
        self.dispatch(|tx| RepoCommand::Blame(args, tx)).await
    }

    pub async fn compare_refs(&self, args: CompareRefsArgs) -> Result<BranchCompare, GitCommandError> {
        self.dispatch(|tx| RepoCommand::CompareRefs(args, tx)).await
    }

    pub async fn tree_diff(&self, args: RangeDiffArgs) -> Result<CommitDiff, GitCommandError> {
        self.dispatch(|tx| RepoCommand::TreeDiff(args, tx)).await
    }

    pub async fn rebase_plan(&self, from_sha: String) -> Result<RebasePlan, GitCommandError> {
        self.dispatch(|tx| RepoCommand::GetRebasePlan(from_sha, tx)).await
    }

    pub async fn start_interactive_rebase(
        &self,
        args: InteractiveRebaseArgs,
    ) -> Result<RebaseStatus, GitCommandError> {
        self.dispatch(|tx| RepoCommand::StartInteractiveRebase(args, tx))
            .await
    }

    pub async fn worktree(&self, op: WorktreeOp) -> Result<Vec<WorktreeInfo>, GitCommandError> {
        self.dispatch(|tx| RepoCommand::Worktree(op, tx)).await
    }

    pub async fn submodules(
        &self,
        update: Option<Vec<String>>,
    ) -> Result<Vec<SubmoduleInfo>, GitCommandError> {
        self.dispatch(|tx| RepoCommand::Submodules(update, tx)).await
    }

    pub async fn submodule_count(&self) -> Result<usize, GitCommandError> {
        self.dispatch(RepoCommand::SubmoduleCount).await
    }

    pub async fn workdir(&self) -> Result<PathBuf, GitCommandError> {
        self.dispatch(RepoCommand::Workdir).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_actor_opens_the_same_worktree() {
        let tmp = tempfile::tempdir().unwrap();
        let main = git2::Repository::init(tmp.path().join("main")).unwrap();
        let sig = git2::Signature::now("T", "t@example.com").unwrap();
        let tree = main.find_tree(main.index().unwrap().write_tree().unwrap()).unwrap();
        main.commit(Some("HEAD"), &sig, &sig, "first", &tree, &[]).unwrap();
        let wt_path = tmp.path().join("wt");
        let wt = main.worktree("wt", &wt_path, None).unwrap();
        let wt_repo = git2::Repository::open_from_worktree(&wt).unwrap();

        let (tx, rx) = Reply::channel();
        off_actor(&wt_repo, tx, |repo| {
            Ok((repo.workdir().map(Path::to_path_buf), repo.is_worktree()))
        });
        let (workdir, is_worktree) = rx.blocking_recv().unwrap().unwrap();
        assert!(is_worktree);
        assert_eq!(
            workdir.unwrap().canonicalize().unwrap(),
            wt_path.canonicalize().unwrap()
        );
    }
}

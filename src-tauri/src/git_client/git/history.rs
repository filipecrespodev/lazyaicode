//! Read-only history views: a file's history (following renames), blame,
//! and comparing two branches. All of it runs on the repo actor thread and
//! is paginated/capped so one request never walks unbounded history.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::git_client::error::GitCommandError;

use super::diff::{self, CommitDiff, FileDiff, FileStatus};
use super::log::{to_commit_node, CommitNode};

/// Commits a single file-history request may look at before handing back a
/// cursor: keeps a rarely-touched file in a huge repo from holding the actor
/// (and with it every other git op of the repo) for seconds.
const MAX_SCAN_PER_PAGE: usize = 20_000;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHistoryQuery {
    pub path: String,
    /// Where the walk starts; HEAD when omitted.
    pub start_sha: Option<String>,
    pub cursor: Option<FileHistoryCursor>,
    pub limit: u32,
}

/// Where the previous page stopped: the walk's frontier (commits queued but
/// not visited yet, in visit order) and the file's name at that point, since
/// following renames changes it along the way. Resuming from the frontier
/// continues the walk exactly where it was, instead of re-walking (or
/// skipping over) everything the earlier pages already covered.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileHistoryCursor {
    pub frontier: Vec<String>,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHistoryEntry {
    pub commit: CommitNode,
    /// The file's name in this commit.
    pub path: String,
    /// Its previous name, when this commit renamed it.
    pub old_path: Option<String>,
    pub status: FileStatus,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHistoryPage {
    pub entries: Vec<FileHistoryEntry>,
    pub next_cursor: Option<FileHistoryCursor>,
}

fn parse_oid(sha: &str) -> Result<git2::Oid, GitCommandError> {
    git2::Oid::from_str(sha).map_err(|_| GitCommandError::RepoNotFound {
        path: sha.to_string(),
    })
}

fn start_commit<'r>(
    repo: &'r git2::Repository,
    sha: Option<&str>,
) -> Result<git2::Commit<'r>, GitCommandError> {
    match sha.filter(|s| !s.is_empty()) {
        Some(sha) => Ok(repo.find_commit(parse_oid(sha)?)?),
        None => Ok(repo.head()?.peel_to_commit()?),
    }
}

fn blob_at(tree: &git2::Tree, path: &str) -> Option<git2::Oid> {
    tree.get_path(Path::new(path))
        .ok()
        .filter(|e| e.kind() != Some(git2::ObjectType::Tree))
        .map(|e| e.id())
}

/// The path `path` had in `parent` when `commit` renamed it, if it did.
/// Only called where the file appears out of nowhere, so the full-tree
/// rename detection runs once per rename, not once per commit.
fn renamed_from(
    repo: &git2::Repository,
    parent: &git2::Tree,
    tree: &git2::Tree,
    path: &str,
) -> Result<Option<String>, GitCommandError> {
    let mut diff = repo.diff_tree_to_tree(Some(parent), Some(tree), None)?;
    diff.find_similar(Some(git2::DiffFindOptions::new().renames(true)))?;
    Ok(diff.deltas().find_map(|d| {
        let new = d.new_file().path()?.to_string_lossy();
        (d.status() == git2::Delta::Renamed && new == path)
            .then(|| d.old_file().path().map(|p| p.to_string_lossy().into_owned()))
            .flatten()
    }))
}

/// `git log`'s default walk order: newest committer date first (ties in
/// queueing order), produced lazily from a priority queue. libgit2's sorted
/// revwalks prepare the whole graph before yielding the first commit, which
/// on a big repo costs seconds per page; this reads only the commits it
/// visits.
struct DateWalk<'r> {
    repo: &'r git2::Repository,
    queue: BinaryHeap<(i64, Reverse<u64>, git2::Oid)>,
    queued: HashSet<git2::Oid>,
    seq: u64,
}

impl<'r> DateWalk<'r> {
    fn new(repo: &'r git2::Repository) -> Self {
        Self {
            repo,
            queue: BinaryHeap::new(),
            queued: HashSet::new(),
            seq: 0,
        }
    }

    fn push(&mut self, commit: &git2::Commit) {
        if self.queued.insert(commit.id()) {
            self.seq += 1;
            self.queue
                .push((commit.time().seconds(), Reverse(self.seq), commit.id()));
        }
    }

    fn next(&mut self) -> Result<Option<git2::Commit<'r>>, GitCommandError> {
        match self.queue.pop() {
            Some((_, _, oid)) => Ok(Some(self.repo.find_commit(oid)?)),
            None => Ok(None),
        }
    }

    /// Queued commits in the order they'd be visited.
    fn frontier(&self) -> Vec<String> {
        let mut pending: Vec<_> = self.queue.iter().collect();
        pending.sort_by(|a, b| b.cmp(a));
        pending.iter().map(|(_, _, oid)| oid.to_string()).collect()
    }
}

/// `git log --follow -- <path>`, one page at a time. Whether a commit
/// touched the file is decided by comparing blob ids of that one path in
/// each tree (no diff); rename detection only runs where the file appears.
/// Like git's default history simplification, a merge that kept one
/// parent's version of the file isn't listed and only that parent is walked
/// further, which also prunes side branches that never touched the file.
pub fn file_history(
    repo: &git2::Repository,
    query: &FileHistoryQuery,
) -> Result<FileHistoryPage, GitCommandError> {
    let mut walk = DateWalk::new(repo);
    let mut path = match &query.cursor {
        Some(c) => {
            for sha in &c.frontier {
                walk.push(&repo.find_commit(parse_oid(sha)?)?);
            }
            c.path.clone()
        }
        None => {
            walk.push(&start_commit(repo, query.start_sha.as_deref())?);
            query.path.clone()
        }
    };
    let limit = query.limit.max(1) as usize;
    let mut entries = Vec::new();
    let mut scanned = 0;
    // The file's blob in commits already looked at as someone's parent: in
    // a mostly linear history that halves the tree reads.
    let mut known: HashMap<git2::Oid, Option<git2::Oid>> = HashMap::new();

    loop {
        if entries.len() >= limit || scanned == MAX_SCAN_PER_PAGE {
            let frontier = walk.frontier();
            return Ok(FileHistoryPage {
                entries,
                next_cursor: (!frontier.is_empty()).then_some(FileHistoryCursor { frontier, path }),
            });
        }
        let Some(commit) = walk.next()? else {
            break;
        };
        scanned += 1;
        let blob = match known.remove(&commit.id()) {
            Some(b) => b,
            None => blob_at(&commit.tree()?, &path),
        };
        let parents: Vec<git2::Commit> = commit.parents().collect();
        let mut parent_blobs = Vec::with_capacity(parents.len());
        for p in &parents {
            let b = match known.get(&p.id()) {
                Some(b) => *b,
                None => {
                    let b = blob_at(&p.tree()?, &path);
                    known.insert(p.id(), b);
                    b
                }
            };
            parent_blobs.push(b);
        }

        // A merge that took one parent's version as is: that parent explains
        // the file, the other sides don't need walking.
        if parents.len() > 1 {
            if let Some(i) = parent_blobs.iter().position(|b| *b == blob) {
                walk.push(&parents[i]);
                continue;
            }
        }
        for p in &parents {
            walk.push(p);
        }

        let (status, old_path) = match (blob, parent_blobs.as_slice()) {
            // Gone here, there before: this commit deleted it (it may have
            // been re-added later under the same name).
            (None, prev) if prev.iter().any(Option::is_some) => (FileStatus::Deleted, None),
            (None, _) => continue,
            (Some(b), [Some(prev)]) if *prev == b => continue,
            (Some(_), [None]) => match renamed_from(repo, &parents[0].tree()?, &commit.tree()?, &path)? {
                Some(old) => (FileStatus::Renamed, Some(old)),
                None => (FileStatus::Added, None),
            },
            (Some(_), prev) if prev.iter().all(Option::is_none) => (FileStatus::Added, None),
            (Some(_), _) => (FileStatus::Modified, None),
        };

        entries.push(FileHistoryEntry {
            commit: to_commit_node(&commit),
            path: path.clone(),
            old_path: old_path.clone(),
            status,
        });
        // Older commits know the file by its previous name.
        if let Some(old) = old_path {
            path = old;
            known.clear();
        }
    }

    Ok(FileHistoryPage {
        entries,
        next_cursor: None,
    })
}

/// One file's diff in one commit (against its first parent), following a
/// rename when `old_path` is given. `None` when the commit didn't touch it.
pub fn commit_file_diff(
    repo: &git2::Repository,
    sha: &str,
    path: &str,
    old_path: Option<&str>,
) -> Result<Option<FileDiff>, GitCommandError> {
    let commit = repo.find_commit(parse_oid(sha)?)?;
    let tree = commit.tree()?;
    let parent_tree = commit.parent(0).ok().map(|p| p.tree()).transpose()?;

    let mut opts = diff::full_context_opts();
    opts.pathspec(path).disable_pathspec_match(true);
    if let Some(old) = old_path {
        opts.pathspec(old);
    }
    let mut d = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))?;
    if old_path.is_some() {
        d.find_similar(Some(git2::DiffFindOptions::new().renames(true)))?;
    }
    Ok(diff::diff_to_file_list(&d)?
        .into_iter()
        .find(|f| f.path == path))
}

/// `git diff <from_sha> <to_sha>`: tree against tree, whatever lies between.
pub fn tree_diff(
    repo: &git2::Repository,
    from_sha: &str,
    to_sha: &str,
) -> Result<CommitDiff, GitCommandError> {
    let from = repo.find_commit(parse_oid(from_sha)?)?.tree()?;
    let to = repo.find_commit(parse_oid(to_sha)?)?.tree()?;
    let mut d = repo.diff_tree_to_tree(Some(&from), Some(&to), Some(&mut diff::full_context_opts()))?;
    d.find_similar(Some(git2::DiffFindOptions::new().renames(true)))?;
    Ok(CommitDiff {
        sha: to_sha.to_string(),
        parent_shas: vec![from_sha.to_string()],
        files: diff::diff_to_file_list(&d)?,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameHunk {
    /// 1-based line in the blamed version of the file.
    pub start_line: u32,
    pub line_count: u32,
    pub sha: String,
    /// The file's path in that commit, when it was named differently.
    pub orig_path: Option<String>,
}

/// One window of a file's blame: the file is blamed in chunks of lines so a
/// big file shows its top right away and the rest loads as it's scrolled.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameChunk {
    /// The commit that was blamed (HEAD resolved).
    pub sha: String,
    pub total_lines: u32,
    pub start_line: u32,
    pub lines: Vec<String>,
    /// Consecutive lines from the same commit, merged into one hunk.
    pub hunks: Vec<BlameHunk>,
    /// Every commit the hunks point at, once each.
    pub commits: Vec<CommitNode>,
}

/// Files blamed lately, whole, keyed by (commit, path). libgit2's blame
/// costs the same for ten lines as for the whole file (it walks the file's
/// history either way, seconds for a big, often-changed file), so a file is
/// blamed once and every chunk is cut from that. Shared with the threads
/// blame runs on.
#[derive(Clone, Default)]
pub struct BlameCache(Arc<Mutex<VecDeque<(BlameKey, Arc<Vec<BlameHunk>>)>>>);

type BlameKey = (git2::Oid, String);

const BLAME_CACHE_FILES: usize = 4;

impl BlameCache {
    fn get(&self, key: &BlameKey) -> Option<Arc<Vec<BlameHunk>>> {
        let files = self.0.lock().unwrap();
        files.iter().find(|(k, _)| k == key).map(|(_, h)| h.clone())
    }

    fn put(&self, key: BlameKey, hunks: Arc<Vec<BlameHunk>>) {
        let mut files = self.0.lock().unwrap();
        files.retain(|(k, _)| *k != key);
        if files.len() == BLAME_CACHE_FILES {
            files.pop_front();
        }
        files.push_back((key, hunks));
    }
}

/// The whole file's blame, consecutive lines from the same commit merged.
/// git's own blame is several times faster than libgit2's on a long-lived
/// file (1.3 s against 8 s for 10k lines changed over 1000 commits), so it
/// is used when git is installed; libgit2 is the fallback.
fn blame_file(repo: &git2::Repository, path: &str, commit: git2::Oid) -> Result<Vec<BlameHunk>, GitCommandError> {
    if let Some(hunks) = blame_with_git(repo, path, commit) {
        return Ok(hunks);
    }
    let mut opts = git2::BlameOptions::new();
    opts.newest_commit(commit).use_mailmap(true);
    let blame = repo.blame_file(Path::new(path), Some(&mut opts))?;
    let mut hunks: Vec<BlameHunk> = Vec::new();
    for h in blame.iter() {
        if h.lines_in_hunk() == 0 {
            continue;
        }
        let first = h.final_start_line() as u32;
        let count = h.lines_in_hunk() as u32;
        let sha = h.final_commit_id().to_string();
        match hunks.last_mut() {
            Some(prev) if prev.sha == sha && prev.start_line + prev.line_count == first => {
                prev.line_count += count;
            }
            _ => hunks.push(BlameHunk {
                start_line: first,
                line_count: count,
                sha,
                orig_path: h
                    .path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .filter(|p| p != path),
            }),
        }
    }
    Ok(hunks)
}

/// `git blame --incremental <commit> -- <path>`: one entry per hunk, in no
/// particular order, each ending with the file's name in that commit.
fn blame_with_git(repo: &git2::Repository, path: &str, commit: git2::Oid) -> Option<Vec<BlameHunk>> {
    let rev = commit.to_string();
    let dir = repo.workdir();
    crate::git_client::activity::info(dir, format!("$ git blame --incremental {} -- {path}", &rev[..7]));
    let mut cmd = std::process::Command::new("git");
    cmd.arg("--git-dir")
        .arg(repo.path())
        .args(["-c", "core.quotepath=off", "blame", "--incremental", &rev, "--", path])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        crate::git_client::activity::error(dir, format!("git blame falhou: {}", stderr.trim()));
        return None;
    }
    let mut hunks = parse_incremental_blame(&String::from_utf8_lossy(&out.stdout))?;
    for h in &mut hunks {
        h.orig_path = h.orig_path.take().filter(|p| p != path);
    }
    Some(hunks)
}

/// Parses `--incremental` output into line-ordered hunks, neighbours from
/// the same commit merged. `orig_path` is always set (to the commit's name
/// of the file). `None` on output it doesn't understand.
fn parse_incremental_blame(out: &str) -> Option<Vec<BlameHunk>> {
    let mut hunks = Vec::new();
    let mut current: Option<(String, u32, u32)> = None;
    for line in out.lines() {
        if let Some(name) = line.strip_prefix("filename ") {
            let (sha, start_line, line_count) = current.take()?;
            let name = name.strip_prefix('"').and_then(|n| n.strip_suffix('"')).unwrap_or(name);
            hunks.push(BlameHunk { start_line, line_count, sha, orig_path: Some(name.to_string()) });
        } else if current.is_none() {
            let mut parts = line.split(' ');
            let sha = parts.next()?;
            let (_, start, count) = (parts.next()?, parts.next()?, parts.next()?);
            if sha.len() < 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            current = Some((sha.to_string(), start.parse().ok()?, count.parse().ok()?));
        }
        // Other lines are the commit's details (author, summary...), given
        // once per commit: the commit list is read from the repo instead.
    }
    hunks.sort_by_key(|h: &BlameHunk| h.start_line);
    let mut merged: Vec<BlameHunk> = Vec::with_capacity(hunks.len());
    for h in hunks {
        match merged.last_mut() {
            Some(prev) if prev.sha == h.sha && prev.start_line + prev.line_count == h.start_line => {
                prev.line_count += h.line_count;
            }
            _ => merged.push(h),
        }
    }
    Some(merged)
}

pub fn blame(
    repo: &git2::Repository,
    path: &str,
    sha: Option<&str>,
    start_line: u32,
    max_lines: u32,
    cache: &BlameCache,
) -> Result<BlameChunk, GitCommandError> {
    let commit = start_commit(repo, sha)?;
    let entry = commit.tree()?.get_path(Path::new(path))?;
    let blob = repo.find_blob(entry.id())?;
    if blob.is_binary() {
        return Err(GitCommandError::Internal(format!("{path} é um arquivo binário")));
    }
    let text = String::from_utf8_lossy(blob.content());
    let all: Vec<&str> = text.lines().collect();
    let total = all.len() as u32;
    let start = start_line.max(1);
    let end = start.saturating_add(max_lines.max(1) - 1).min(total);

    let mut chunk = BlameChunk {
        sha: commit.id().to_string(),
        total_lines: total,
        start_line: start,
        lines: Vec::new(),
        hunks: Vec::new(),
        commits: Vec::new(),
    };
    if start > end {
        return Ok(chunk);
    }
    chunk.lines = all[(start - 1) as usize..end as usize]
        .iter()
        .map(|l| l.to_string())
        .collect();

    let key = (commit.id(), path.to_string());
    let hunks = match cache.get(&key) {
        Some(hunks) => hunks,
        None => {
            let hunks = Arc::new(blame_file(repo, path, commit.id())?);
            cache.put(key, hunks.clone());
            hunks
        }
    };

    let mut seen = HashSet::new();
    // Hunks are in line order: skip to the first one reaching `start`.
    let from = hunks.partition_point(|h| h.start_line + h.line_count <= start);
    for h in hunks[from..].iter().take_while(|h| h.start_line <= end) {
        let first = h.start_line.max(start);
        let last = (h.start_line + h.line_count - 1).min(end);
        chunk.hunks.push(BlameHunk {
            start_line: first,
            line_count: last - first + 1,
            ..h.clone()
        });
        if seen.insert(h.sha.as_str()) {
            chunk.commits.push(to_commit_node(&repo.find_commit(parse_oid(&h.sha)?)?));
        }
    }
    Ok(chunk)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchCompare {
    pub a_sha: String,
    pub b_sha: String,
    pub merge_base: Option<String>,
    /// `git log b..a` (capped at the requested limit).
    pub only_in_a: Vec<CommitNode>,
    /// `git log a..b` (capped).
    pub only_in_b: Vec<CommitNode>,
    /// Uncapped counts.
    pub only_in_a_total: u32,
    pub only_in_b_total: u32,
}

fn walk_excluding(
    repo: &git2::Repository,
    include: git2::Oid,
    exclude: git2::Oid,
    limit: usize,
) -> Result<Vec<CommitNode>, GitCommandError> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
    walk.push(include)?;
    walk.hide(exclude)?;
    walk.take(limit)
        .map(|oid| Ok(to_commit_node(&repo.find_commit(oid?)?)))
        .collect()
}

/// Branch menus name branches by short name, and remote-only ones without
/// their remote ("feature" for origin/feature or upstream/feature, origin
/// first). Branches are looked up first: a bare revparse would prefer a tag
/// of the same name.
fn resolve_branch(repo: &git2::Repository, name: &str) -> Result<git2::Oid, GitCommandError> {
    let mut remotes = super::remote::remote_names(repo).unwrap_or_default();
    if !remotes.iter().any(|r| r == "origin") {
        remotes.insert(0, "origin".into());
    }
    let candidates = std::iter::once(format!("refs/heads/{name}"))
        .chain(remotes.iter().map(|r| format!("refs/remotes/{r}/{name}")))
        .chain(std::iter::once(format!("refs/remotes/{name}")));
    for full in candidates {
        if let Ok(reference) = repo.find_reference(&full) {
            return Ok(reference.peel_to_commit()?.id());
        }
    }
    Ok(repo.revparse_single(name)?.peel_to_commit()?.id())
}

/// Commits on either side of two refs since they diverged — the revwalks
/// hide the other side, so they stop at the merge base instead of walking
/// all of history.
pub fn compare_refs(
    repo: &git2::Repository,
    a: &str,
    b: &str,
    limit: u32,
) -> Result<BranchCompare, GitCommandError> {
    let a_id = resolve_branch(repo, a)?;
    let b_id = resolve_branch(repo, b)?;
    let limit = limit.max(1) as usize;
    let merge_base = match repo.merge_base(a_id, b_id) {
        Ok(oid) => Some(oid.to_string()),
        Err(e) if e.code() == git2::ErrorCode::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let only_in_a = walk_excluding(repo, a_id, b_id, limit)?;
    let only_in_b = walk_excluding(repo, b_id, a_id, limit)?;
    // A list that wasn't cut short is its own count; only a capped one
    // needs the (second) full walk for the exact number.
    let (a_total, b_total) = if only_in_a.len() < limit && only_in_b.len() < limit {
        (only_in_a.len(), only_in_b.len())
    } else {
        repo.graph_ahead_behind(a_id, b_id)?
    };
    Ok(BranchCompare {
        a_sha: a_id.to_string(),
        b_sha: b_id.to_string(),
        merge_base,
        only_in_a,
        only_in_b,
        only_in_a_total: a_total as u32,
        only_in_b_total: b_total as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig() -> git2::Signature<'static> {
        git2::Signature::now("Test", "test@example.com").unwrap()
    }

    /// Commits the given files (None deletes) on top of HEAD.
    fn commit(repo: &git2::Repository, files: &[(&str, Option<&str>)], msg: &str) -> git2::Oid {
        let mut index = repo.index().unwrap();
        for (path, content) in files {
            let full = repo.workdir().unwrap().join(path);
            match content {
                Some(c) => {
                    std::fs::write(&full, c).unwrap();
                    index.add_path(Path::new(path)).unwrap();
                }
                None => {
                    std::fs::remove_file(&full).unwrap();
                    index.remove_path(Path::new(path)).unwrap();
                }
            }
        }
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let parents: Vec<_> = repo
            .head()
            .ok()
            .and_then(|h| h.target())
            .and_then(|oid| repo.find_commit(oid).ok())
            .into_iter()
            .collect();
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        repo.commit(Some("HEAD"), &sig(), &sig(), msg, &tree, &parent_refs)
            .unwrap()
    }

    fn query(path: &str, cursor: Option<FileHistoryCursor>, limit: u32) -> FileHistoryQuery {
        FileHistoryQuery {
            path: path.into(),
            start_sha: None,
            cursor,
            limit,
        }
    }

    fn summaries(page: &FileHistoryPage) -> Vec<String> {
        page.entries.iter().map(|e| e.commit.summary.clone()).collect()
    }

    const BODY: &str = "line one\nline two\nline three\nline four\nline five\n";

    #[test]
    fn file_history_follows_renames() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit(&repo, &[("old.txt", Some(BODY))], "create");
        commit(&repo, &[("other.txt", Some("x\n"))], "unrelated");
        commit(&repo, &[("old.txt", Some(&format!("{BODY}line six\n")))], "edit old");
        commit(
            &repo,
            &[("old.txt", None), ("new.txt", Some(&format!("{BODY}line six\n")))],
            "rename",
        );
        commit(&repo, &[("new.txt", Some(&format!("{BODY}line six\nline seven\n")))], "edit new");

        let page = file_history(&repo, &query("new.txt", None, 300)).unwrap();
        assert_eq!(summaries(&page), ["edit new", "rename", "edit old", "create"]);
        assert_eq!(page.next_cursor, None);
        let rename = &page.entries[1];
        assert!(matches!(rename.status, FileStatus::Renamed));
        assert_eq!(rename.path, "new.txt");
        assert_eq!(rename.old_path.as_deref(), Some("old.txt"));
        assert_eq!(page.entries[2].path, "old.txt");
        assert!(matches!(page.entries[3].status, FileStatus::Added));

        // Per-commit diff of the renamed file shows the rename, not an add.
        let d = commit_file_diff(&repo, &rename.commit.sha, "new.txt", Some("old.txt"))
            .unwrap()
            .unwrap();
        assert!(matches!(d.status, FileStatus::Renamed));
        assert_eq!(d.old_path.as_deref(), Some("old.txt"));
        let d = commit_file_diff(&repo, &page.entries[0].commit.sha, "new.txt", None)
            .unwrap()
            .unwrap();
        assert!(d.patch.contains("+line seven"));
    }

    fn git_cli(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    }

    /// Same commits, same order as `git log --follow`, across a rename and a
    /// delete + re-add of the file.
    #[test]
    fn file_history_matches_git_log_follow() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit(&repo, &[("old.txt", Some(BODY))], "create");
        commit(&repo, &[("old.txt", Some(&format!("{BODY}6\n")))], "edit old");
        commit(&repo, &[("old.txt", None), ("new.txt", Some(&format!("{BODY}6\n")))], "rename");
        commit(&repo, &[("x.txt", Some("x\n"))], "unrelated");
        commit(&repo, &[("new.txt", None)], "delete");
        commit(&repo, &[("new.txt", Some(&format!("{BODY}6\n7\n")))], "re-add");

        let page = file_history(&repo, &query("new.txt", None, 300)).unwrap();
        let ours: Vec<String> = page.entries.iter().map(|e| e.commit.sha.clone()).collect();
        let theirs: Vec<String> = git_cli(tmp.path(), &["log", "--follow", "--format=%H", "--", "new.txt"])
            .lines()
            .map(str::to_string)
            .collect();
        assert_eq!(ours, theirs);
        assert_eq!(summaries(&page), ["re-add", "delete", "rename", "edit old", "create"]);
        assert!(matches!(page.entries[1].status, FileStatus::Deleted));

        // The deleted file's diff in that commit is available too.
        let d = commit_file_diff(&repo, &page.entries[1].commit.sha, "new.txt", None)
            .unwrap()
            .unwrap();
        assert!(matches!(d.status, FileStatus::Deleted));
    }

    #[test]
    fn file_history_lists_a_merge_only_when_it_changed_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let base = commit(&repo, &[("f.txt", Some(BODY))], "base");
        let main = repo.head().unwrap().shorthand().unwrap().to_string();
        commit(&repo, &[("m.txt", Some("m\n"))], "main side");
        repo.branch("feature", &repo.find_commit(base).unwrap(), false).unwrap();
        repo.set_head("refs/heads/feature").unwrap();
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force())).unwrap();
        commit(&repo, &[("f.txt", Some(&format!("{BODY}feature\n")))], "feature edit");
        repo.set_head(&format!("refs/heads/{main}")).unwrap();
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force())).unwrap();
        crate::git_client::git::merge::merge_branch(&repo, "feature").unwrap();

        // The merge took the feature side's f.txt unchanged: not listed.
        let page = file_history(&repo, &query("f.txt", None, 300)).unwrap();
        assert_eq!(summaries(&page), ["feature edit", "base"]);
        let theirs = git_cli(tmp.path(), &["log", "--format=%s", "--", "f.txt"]);
        assert_eq!(theirs.lines().collect::<Vec<_>>(), ["feature edit", "base"]);
    }

    /// Both sides of a merge edit the file and the merge resolves it to new
    /// content: every page boundary (even inside the fork) resumes without
    /// losing or repeating commits, in `git log`'s order.
    #[test]
    fn file_history_pages_through_a_fork_like_git_log() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let base = commit(&repo, &[("f.txt", Some("base\n"))], "base");
        let main = repo.head().unwrap().shorthand().unwrap().to_string();
        let m1 = commit(&repo, &[("f.txt", Some("main\n"))], "main edit");
        commit(&repo, &[("other.txt", Some("o\n"))], "main unrelated");
        repo.branch("feature", &repo.find_commit(base).unwrap(), false).unwrap();
        repo.set_head("refs/heads/feature").unwrap();
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force())).unwrap();
        let f1 = commit(&repo, &[("f.txt", Some("feature\n"))], "feature edit");
        repo.set_head(&format!("refs/heads/{main}")).unwrap();
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force())).unwrap();
        let ours = repo.head().unwrap().peel_to_commit().unwrap();
        let mut index = repo.index().unwrap();
        std::fs::write(tmp.path().join("f.txt"), "resolved\n").unwrap();
        index.add_path(Path::new("f.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let theirs = repo.find_commit(f1).unwrap();
        repo.commit(Some("HEAD"), &sig(), &sig(), "merge", &tree, &[&ours, &theirs]).unwrap();
        let _ = m1;

        let full = file_history(&repo, &query("f.txt", None, 300)).unwrap();
        let expected = git_cli(tmp.path(), &["log", "--format=%s", "--", "f.txt"]);
        assert_eq!(summaries(&full), expected.lines().collect::<Vec<_>>());
        assert_eq!(full.entries.len(), 4);

        let mut paged = Vec::new();
        let mut cursor = None;
        loop {
            let page = file_history(&repo, &query("f.txt", cursor, 1)).unwrap();
            paged.extend(summaries(&page));
            match page.next_cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        assert_eq!(paged, summaries(&full));
    }

    #[test]
    fn file_history_paginates_with_a_cursor_that_keeps_the_followed_name() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit(&repo, &[("a.txt", Some(BODY))], "c1");
        commit(&repo, &[("a.txt", Some(&format!("{BODY}2\n")))], "c2");
        commit(&repo, &[("a.txt", None), ("b.txt", Some(&format!("{BODY}2\n")))], "c3 rename");
        commit(&repo, &[("b.txt", Some(&format!("{BODY}3\n")))], "c4");

        let first = file_history(&repo, &query("b.txt", None, 2)).unwrap();
        assert_eq!(summaries(&first), ["c4", "c3 rename"]);
        let cursor = first.next_cursor.clone().unwrap();
        assert_eq!(cursor.path, "a.txt");

        let second = file_history(&repo, &query("b.txt", Some(cursor), 2)).unwrap();
        assert_eq!(summaries(&second), ["c2", "c1"]);
        assert_eq!(second.next_cursor, None);
    }

    #[test]
    fn blame_attributes_lines_and_merges_hunks() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let first = commit(&repo, &[("f.txt", Some("a\nb\nc\nd\n"))], "first");
        let second = commit(&repo, &[("f.txt", Some("a\nB\nC\nd\ne\n"))], "second");
        // Shared by every call below: windows are cut from one cached blame per commit.
        let cache = BlameCache::default();

        let chunk = blame(&repo, "f.txt", None, 1, 1000, &cache).unwrap();
        assert_eq!(chunk.sha, second.to_string());
        assert_eq!(chunk.total_lines, 5);
        assert_eq!(chunk.lines, ["a", "B", "C", "d", "e"]);
        let spans: Vec<(u32, u32, String)> = chunk
            .hunks
            .iter()
            .map(|h| (h.start_line, h.line_count, h.sha.clone()))
            .collect();
        assert_eq!(
            spans,
            [
                (1, 1, first.to_string()),
                (2, 2, second.to_string()),
                (4, 1, first.to_string()),
                (5, 1, second.to_string()),
            ]
        );
        assert_eq!(chunk.commits.len(), 2);

        // Line by line, the same attribution as `git blame --porcelain`.
        let mut theirs = vec![String::new(); 5];
        for l in git_cli(tmp.path(), &["blame", "--porcelain", "HEAD", "--", "f.txt"]).lines() {
            let parts: Vec<&str> = l.split(' ').collect();
            if parts.len() >= 3 && parts[0].len() == 40 && parts[0].chars().all(|c| c.is_ascii_hexdigit()) {
                theirs[parts[2].parse::<usize>().unwrap() - 1] = parts[0].to_string();
            }
        }
        let mut ours = vec![String::new(); 5];
        for h in &chunk.hunks {
            for line in h.start_line..h.start_line + h.line_count {
                ours[line as usize - 1] = h.sha.clone();
            }
        }
        assert_eq!(ours, theirs);

        // Blaming at the older commit sees only its lines.
        let old = blame(&repo, "f.txt", Some(&first.to_string()), 1, 1000, &cache).unwrap();
        assert_eq!(old.total_lines, 4);
        assert!(old.hunks.iter().all(|h| h.sha == first.to_string()));

        // A window covers just its lines, clamped to the file.
        let window = blame(&repo, "f.txt", None, 2, 2, &cache).unwrap();
        assert_eq!(window.lines, ["B", "C"]);
        assert_eq!(window.hunks.len(), 1);
        assert_eq!((window.hunks[0].start_line, window.hunks[0].line_count), (2, 2));
        let tail = blame(&repo, "f.txt", None, 5, 100, &cache).unwrap();
        assert_eq!(tail.lines, ["e"]);
        assert_eq!(tail.hunks.iter().map(|h| (h.start_line, h.line_count)).collect::<Vec<_>>(), [(5, 1)]);
        assert_eq!(cache.0.lock().unwrap().len(), 2);
    }

    #[test]
    fn parses_incremental_blame_out_of_order() {
        let a = "a".repeat(40);
        let b = "b".repeat(40);
        let out = format!(
            "{b} 3 3 1\nauthor B\nsummary second\nfilename f.txt\n\
             {a} 1 1 2\nauthor A\nsummary first\nprevious {b} f.txt\nfilename old name.txt\n\
             {b} 4 4 2\nfilename f.txt\n"
        );
        let hunks = parse_incremental_blame(&out).unwrap();
        let spans: Vec<_> = hunks.iter().map(|h| (h.start_line, h.line_count, &h.sha[..1], h.orig_path.as_deref())).collect();
        assert_eq!(spans, [(1, 2, "a", Some("old name.txt")), (3, 3, "b", Some("f.txt"))]);
        assert!(parse_incremental_blame("not blame output\n").is_none());
    }

    #[test]
    fn git_and_libgit2_blame_agree() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit(&repo, &[("f.txt", Some("a\nb\nc\nd\n"))], "first");
        commit(&repo, &[("f.txt", None), ("g.txt", Some("a\nb\nc\nd\n"))], "rename");
        let head = commit(&repo, &[("g.txt", Some("a\nB\nc\nd\ne\n"))], "second");
        let Some(git) = blame_with_git(&repo, "g.txt", head) else {
            return; // no git installed
        };
        let mut opts = git2::BlameOptions::new();
        opts.newest_commit(head);
        let lib: Vec<_> = repo
            .blame_file(Path::new("g.txt"), Some(&mut opts))
            .unwrap()
            .iter()
            .map(|h| (h.final_start_line() as u32, h.lines_in_hunk() as u32, h.final_commit_id().to_string()))
            .collect();
        let ours: Vec<_> = git.iter().map(|h| (h.start_line, h.line_count, h.sha.clone())).collect();
        assert_eq!(ours, lib);
    }

    #[test]
    fn compare_lists_commits_on_each_side_since_the_merge_base() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let base = commit(&repo, &[("f.txt", Some("base\n"))], "base");
        let main = repo.head().unwrap().shorthand().unwrap().to_string();
        commit(&repo, &[("m.txt", Some("1\n"))], "main 1");
        commit(&repo, &[("m.txt", Some("2\n"))], "main 2");
        repo.branch("feature", &repo.find_commit(base).unwrap(), false).unwrap();
        repo.set_head("refs/heads/feature").unwrap();
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force())).unwrap();
        let feat = commit(&repo, &[("x.txt", Some("x\n"))], "feature 1");

        let cmp = compare_refs(&repo, "feature", &main, 100).unwrap();
        assert_eq!(cmp.merge_base, Some(base.to_string()));
        assert_eq!(cmp.only_in_a.iter().map(|c| c.summary.as_str()).collect::<Vec<_>>(), ["feature 1"]);
        assert_eq!(
            cmp.only_in_b.iter().map(|c| c.summary.as_str()).collect::<Vec<_>>(),
            ["main 2", "main 1"]
        );
        assert_eq!((cmp.only_in_a_total, cmp.only_in_b_total), (1, 2));

        // Capped lists keep the real totals.
        let capped = compare_refs(&repo, "feature", &main, 1).unwrap();
        assert_eq!(capped.only_in_b.len(), 1);
        assert_eq!(capped.only_in_b_total, 2);

        // A remote-only branch is found by its short name.
        repo.reference("refs/remotes/origin/remote-only", feat, true, "test").unwrap();
        let remote = compare_refs(&repo, "remote-only", &main, 100).unwrap();
        assert_eq!(remote.a_sha, feat.to_string());
        // ...on any configured remote, not only origin.
        repo.remote("upstream", "https://example.com/up.git").unwrap();
        repo.reference("refs/remotes/upstream/up-only", feat, true, "test").unwrap();
        assert_eq!(compare_refs(&repo, "up-only", &main, 100).unwrap().a_sha, feat.to_string());

        // A tag named like the branch doesn't shadow it.
        repo.tag_lightweight("feature", repo.find_commit(base).unwrap().as_object(), false).unwrap();
        let tagged = compare_refs(&repo, "feature", &main, 100).unwrap();
        assert_eq!(tagged.a_sha, feat.to_string());

        // Same sets as `git rev-list`.
        let rev_list = |range: &str| -> Vec<String> {
            git_cli(tmp.path(), &["rev-list", range]).lines().map(str::to_string).collect()
        };
        let ids = |nodes: &[CommitNode]| nodes.iter().map(|c| c.sha.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&cmp.only_in_a), rev_list(&format!("refs/heads/{main}..refs/heads/feature")));
        assert_eq!(ids(&cmp.only_in_b), rev_list(&format!("refs/heads/feature..refs/heads/{main}")));

        // Aggregate diff from the merge base: only the feature side's file.
        let d = tree_diff(&repo, &base.to_string(), &feat.to_string()).unwrap();
        assert_eq!(d.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["x.txt"]);
    }
}

use serde::{Deserialize, Serialize};

use crate::git_client::error::GitCommandError;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDiff {
    pub sha: String,
    pub parent_shas: Vec<String>,
    pub files: Vec<FileDiff>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
    Copied,
    TypeChange,
    Other,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub is_binary: bool,
    /// Full unified diff text for this single file (as produced by `git diff`),
    /// ready to feed into a diff-rendering component. Empty when `is_binary`.
    pub patch: String,
}

/// Full-file context (not the default 3 lines) for every diff this module
/// generates — the diff viewer shows the whole file, not just the hunks.
pub(crate) fn full_context_opts() -> git2::DiffOptions {
    let mut opts = git2::DiffOptions::new();
    opts.context_lines(u32::MAX);
    opts
}

pub fn commit_diff(repo: &git2::Repository, sha: &str) -> Result<CommitDiff, GitCommandError> {
    let oid = git2::Oid::from_str(sha).map_err(|_| GitCommandError::RepoNotFound {
        path: sha.to_string(),
    })?;
    let commit = repo.find_commit(oid)?;
    let new_tree = commit.tree()?;
    let old_tree = commit.parent(0).ok().map(|p| p.tree()).transpose()?;

    let diff = repo.diff_tree_to_tree(
        old_tree.as_ref(),
        Some(&new_tree),
        Some(&mut full_context_opts()),
    )?;

    Ok(CommitDiff {
        sha: commit.id().to_string(),
        parent_shas: commit.parent_ids().map(|id| id.to_string()).collect(),
        files: diff_to_file_list(&diff)?,
    })
}

/// Diff spanning several commits: the parent of `from_sha` (the oldest commit
/// in the selection) against the tree of `to_sha` (the newest) — equivalent
/// to `git diff from_sha^..to_sha`, regardless of which commits in between
/// were individually toggled on/off in a multi-select.
pub fn range_diff(
    repo: &git2::Repository,
    from_sha: &str,
    to_sha: &str,
) -> Result<CommitDiff, GitCommandError> {
    let from_oid = git2::Oid::from_str(from_sha).map_err(|_| GitCommandError::RepoNotFound {
        path: from_sha.to_string(),
    })?;
    let to_oid = git2::Oid::from_str(to_sha).map_err(|_| GitCommandError::RepoNotFound {
        path: to_sha.to_string(),
    })?;
    let from_commit = repo.find_commit(from_oid)?;
    let to_commit = repo.find_commit(to_oid)?;

    let old_tree = from_commit.parent(0).ok().map(|p| p.tree()).transpose()?;
    let new_tree = to_commit.tree()?;

    let diff = repo.diff_tree_to_tree(
        old_tree.as_ref(),
        Some(&new_tree),
        Some(&mut full_context_opts()),
    )?;

    Ok(CommitDiff {
        sha: String::new(),
        parent_shas: vec![from_sha.to_string()],
        files: diff_to_file_list(&diff)?,
    })
}

/// Diff of the working directory against either the index (unstaged changes)
/// or HEAD (staged changes) — no real commit involved, so `sha`/`parent_shas`
/// are left empty as a sentinel for "not a real commit".
pub fn workdir_diff(repo: &git2::Repository, staged: bool) -> Result<CommitDiff, GitCommandError> {
    let diff = if staged {
        // Unborn HEAD (no commit yet): everything in the index is an addition.
        let head_tree = match repo.head() {
            Ok(head) => Some(head.peel_to_tree()?),
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
            Err(e) => return Err(e.into()),
        };
        repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut full_context_opts()))?
    } else {
        // Untracked files are excluded from this diff by default — without
        // this, a newly added (not yet staged) file has no patch content to
        // show, even though `working_status` already lists it as "Added".
        let mut opts = full_context_opts();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .show_untracked_content(true);
        repo.diff_index_to_workdir(None, Some(&mut opts))?
    };

    Ok(CommitDiff {
        sha: String::new(),
        parent_shas: Vec::new(),
        files: diff_to_file_list(&diff)?,
    })
}

/// Reads the *current* content of `path` — from `sha`'s tree if given, or
/// straight off disk (the working directory) otherwise — and returns it
/// base64-encoded, ready for a `data:` URI. Used to render images in the
/// diff viewer: only the new version is ever needed, never the old one.
pub fn read_file_base64(
    repo: &git2::Repository,
    sha: Option<&str>,
    path: &str,
) -> Result<String, GitCommandError> {
    let bytes = match sha.filter(|s| !s.is_empty()) {
        Some(sha) => {
            let oid = git2::Oid::from_str(sha).map_err(|_| GitCommandError::RepoNotFound {
                path: sha.to_string(),
            })?;
            let tree = repo.find_commit(oid)?.tree()?;
            let entry = tree.get_path(std::path::Path::new(path))?;
            repo.find_blob(entry.id())?.content().to_vec()
        }
        None => {
            let workdir = repo.workdir().ok_or_else(|| {
                GitCommandError::Internal("repositório bare não tem diretório de trabalho".into())
            })?;
            std::fs::read(workdir.join(path))
                .map_err(|e| GitCommandError::Internal(e.to_string()))?
        }
    };
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

pub(crate) fn diff_to_file_list(diff: &git2::Diff) -> Result<Vec<FileDiff>, GitCommandError> {
    let mut files = Vec::new();
    for idx in 0..diff.deltas().len() {
        let delta = diff.get_delta(idx).unwrap();
        let old_file_path = delta.old_file().path().map(path_to_string);
        let new_file_path = delta.new_file().path().map(path_to_string);
        let is_binary = delta.flags().is_binary();

        let patch = if is_binary {
            String::new()
        } else {
            git2::Patch::from_diff(diff, idx)?
                .map(|mut p| p.to_buf())
                .transpose()?
                .map(|buf| String::from_utf8_lossy(&buf).into_owned())
                .unwrap_or_default()
        };

        files.push(FileDiff {
            path: new_file_path
                .clone()
                .or_else(|| old_file_path.clone())
                .unwrap_or_default(),
            old_path: match (&old_file_path, &new_file_path) {
                (Some(o), Some(n)) if o != n => Some(o.clone()),
                _ => None,
            },
            status: to_file_status(delta.status()),
            is_binary,
            patch,
        });
    }
    Ok(files)
}

fn path_to_string(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}

fn to_file_status(status: git2::Delta) -> FileStatus {
    match status {
        // An untracked workdir file is exactly an "added" file from the
        // user's point of view — same rendering, just not indexed yet.
        git2::Delta::Added | git2::Delta::Untracked => FileStatus::Added,
        git2::Delta::Deleted => FileStatus::Deleted,
        git2::Delta::Modified => FileStatus::Modified,
        git2::Delta::Renamed => FileStatus::Renamed,
        git2::Delta::Copied => FileStatus::Copied,
        git2::Delta::Typechange => FileStatus::TypeChange,
        _ => FileStatus::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_commit(repo: &git2::Repository, file: &str, content: &str) -> git2::Oid {
        std::fs::write(repo.workdir().unwrap().join(file), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new(file)).unwrap();
        let tree_id = index.write_tree().unwrap();
        index.write().unwrap();

        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().ok().and_then(|h| h.target());
        let parents: Vec<_> = parent
            .and_then(|oid| repo.find_commit(oid).ok())
            .into_iter()
            .collect();
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "msg", &tree, &parent_refs)
            .unwrap()
    }

    #[test]
    fn diffs_a_modified_file_against_its_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        write_commit(&repo, "a.txt", "line1\n");
        let second = write_commit(&repo, "a.txt", "line1\nline2\n");

        let diff = commit_diff(&repo, &second.to_string()).unwrap();
        assert_eq!(diff.files.len(), 1);
        assert_eq!(diff.files[0].path, "a.txt");
        assert!(matches!(diff.files[0].status, FileStatus::Modified));
        assert!(diff.files[0].patch.contains("+line2"));
    }

    #[test]
    fn diffs_root_commit_against_empty_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let first = write_commit(&repo, "a.txt", "hello\n");

        let diff = commit_diff(&repo, &first.to_string()).unwrap();
        assert_eq!(diff.parent_shas.len(), 0);
        assert!(matches!(diff.files[0].status, FileStatus::Added));
    }

    #[test]
    fn workdir_diff_shows_full_content_for_a_new_untracked_file() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        write_commit(&repo, "a.txt", "hello\n");

        // untracked, never staged
        std::fs::write(tmp.path().join("new.txt"), "line1\nline2\n").unwrap();

        let diff = workdir_diff(&repo, false).unwrap();
        let file = diff.files.iter().find(|f| f.path == "new.txt").unwrap();
        assert!(matches!(file.status, FileStatus::Added));
        assert!(!file.is_binary);
        assert!(file.patch.contains("+line1"));
        assert!(file.patch.contains("+line2"));
    }

    #[test]
    fn reads_file_content_from_a_commit_and_from_the_working_directory() {
        use base64::Engine;

        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let sha = write_commit(&repo, "img.bin", "committed-bytes");

        let from_commit = read_file_base64(&repo, Some(&sha.to_string()), "img.bin").unwrap();
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(from_commit)
                .unwrap(),
            b"committed-bytes"
        );

        std::fs::write(tmp.path().join("img.bin"), "workdir-bytes").unwrap();
        let from_workdir = read_file_base64(&repo, None, "img.bin").unwrap();
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(from_workdir)
                .unwrap(),
            b"workdir-bytes"
        );
    }
}

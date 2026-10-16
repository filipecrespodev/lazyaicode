use serde::{Deserialize, Serialize};

use crate::error::GitCommandError;

/// How many entries are read at most (newest first).
const MAX_ENTRIES: usize = 1000;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflogEntry {
    /// `HEAD@{3}` style selector.
    pub selector: String,
    pub new_sha: String,
    /// All zeros when the ref was created by this entry.
    pub old_sha: String,
    pub committer_name: String,
    pub committer_email: String,
    /// Unix seconds.
    pub time: i64,
    /// e.g. "commit: fix login", "checkout: moving from main to dev".
    pub message: String,
    /// Summary of the commit the ref pointed to after this entry; `None`
    /// when that commit no longer exists (garbage-collected).
    pub commit_summary: Option<String>,
}

/// The reflog of `ref_name` ("HEAD" or a local branch name), newest first.
pub fn reflog(repo: &git2::Repository, ref_name: &str) -> Result<Vec<ReflogEntry>, GitCommandError> {
    let full_name =
        if ref_name == "HEAD" || ref_name.starts_with("refs/") { ref_name.to_string() } else { format!("refs/heads/{ref_name}") };
    let log = repo.reflog(&full_name)?;

    Ok(log
        .iter()
        .take(MAX_ENTRIES)
        .enumerate()
        .map(|(i, e)| {
            let committer = e.committer();
            ReflogEntry {
                selector: format!("{ref_name}@{{{i}}}"),
                new_sha: e.id_new().to_string(),
                old_sha: e.id_old().to_string(),
                committer_name: committer.name().unwrap_or("").to_string(),
                committer_email: committer.email().unwrap_or("").to_string(),
                time: committer.when().seconds(),
                message: e.message().ok().flatten().unwrap_or("").to_string(),
                commit_summary: repo.find_commit(e.id_new()).ok().and_then(|c| c.summary().ok().flatten().map(str::to_string)),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn lists_head_and_branch_reflogs_newest_first() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let commit = |msg: &str, file: &str| {
            std::fs::write(tmp.path().join(file), msg).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(Path::new(file)).unwrap();
            index.write().unwrap();
            let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
            let parents: Vec<git2::Commit> = repo.head().ok().and_then(|h| h.peel_to_commit().ok()).into_iter().collect();
            let refs: Vec<&git2::Commit> = parents.iter().collect();
            repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &refs).unwrap()
        };
        commit("first", "a");
        let second = commit("second", "b");

        let entries = reflog(&repo, "HEAD").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].selector, "HEAD@{0}");
        assert_eq!(entries[0].new_sha, second.to_string());
        assert_eq!(entries[0].commit_summary.as_deref(), Some("second"));
        assert_eq!(entries[0].committer_email, "test@example.com");
        assert!(entries[0].message.contains("second"));
        assert_eq!(entries[1].old_sha, "0".repeat(40));

        let branch = repo.head().unwrap().shorthand().unwrap().to_string();
        assert_eq!(reflog(&repo, &branch).unwrap().len(), 2);
    }
}

use serde::{Deserialize, Serialize};

use crate::git_client::error::GitCommandError;
use crate::git_client::git::log::{to_commit_node, walk_order, CommitNode, LogQuery};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchMode {
    Message,
    Author,
    Sha,
    File,
    Diff,
}

/// Commits (newest first, from every branch) matching `query` under `mode`,
/// case-insensitively, stopping at `limit` hits.
pub fn search_commits(
    repo: &git2::Repository,
    query: &str,
    mode: SearchMode,
    limit: usize,
) -> Result<Vec<CommitNode>, GitCommandError> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let order = walk_order(repo, &LogQuery { skip: 0, limit: 0, branch_filter: None })?;
    let mut hits = Vec::new();
    for oid in order {
        if hits.len() >= limit {
            break;
        }
        let commit = repo.find_commit(oid)?;
        if matches(repo, &commit, &q, mode)? {
            hits.push(to_commit_node(&commit));
        }
    }
    Ok(hits)
}

fn matches(
    repo: &git2::Repository,
    commit: &git2::Commit,
    q: &str,
    mode: SearchMode,
) -> Result<bool, GitCommandError> {
    Ok(match mode {
        SearchMode::Message => commit.message().unwrap_or_default().to_lowercase().contains(q),
        SearchMode::Author => {
            let a = commit.author();
            format!("{} {}", a.name().unwrap_or_default(), a.email().unwrap_or_default())
                .to_lowercase()
                .contains(q)
        }
        SearchMode::Sha => commit.id().to_string().starts_with(q),
        SearchMode::File | SearchMode::Diff => {
            let diff = first_parent_diff(repo, commit)?;
            if mode == SearchMode::File {
                diff.deltas().any(|d| {
                    [d.new_file().path(), d.old_file().path()]
                        .into_iter()
                        .flatten()
                        .any(|p| p.to_string_lossy().to_lowercase().contains(q))
                })
            } else {
                let mut found = false;
                // Returning false aborts the walk once a line matches.
                let _ = diff.foreach(
                    &mut |_, _| true,
                    None,
                    None,
                    Some(&mut |_, _, line| {
                        if matches!(line.origin(), '+' | '-')
                            && String::from_utf8_lossy(line.content()).to_lowercase().contains(q)
                        {
                            found = true;
                        }
                        !found
                    }),
                );
                found
            }
        }
    })
}

fn first_parent_diff<'r>(
    repo: &'r git2::Repository,
    commit: &git2::Commit,
) -> Result<git2::Diff<'r>, GitCommandError> {
    let old = commit.parent(0).ok().map(|p| p.tree()).transpose()?;
    let mut opts = git2::DiffOptions::new();
    opts.context_lines(0);
    Ok(repo.diff_tree_to_tree(old.as_ref(), Some(&commit.tree()?), Some(&mut opts))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(repo: &git2::Repository, file: &str, content: &str, msg: &str, author: &str) {
        let path = repo.workdir().unwrap().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new(file)).unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now(author, "a@example.com").unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<_> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents).unwrap();
    }

    fn summaries(hits: Vec<CommitNode>) -> Vec<String> {
        hits.into_iter().map(|c| c.summary).collect()
    }

    #[test]
    fn finds_commits_by_each_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        commit(&repo, "a.txt", "alpha", "add alpha", "Alice");
        commit(&repo, "docs/b.md", "needle here", "add docs", "Bob");
        commit(&repo, "a.txt", "alpha2", "tweak", "Alice");

        let s = |q, m| summaries(search_commits(&repo, q, m, 50).unwrap());
        assert_eq!(s("ALPHA", SearchMode::Message), ["add alpha"]);
        assert_eq!(s("bob", SearchMode::Author), ["add docs"]);
        assert_eq!(s("docs/b", SearchMode::File), ["add docs"]);
        assert_eq!(s("needle", SearchMode::Diff), ["add docs"]);
        let head = repo.head().unwrap().target().unwrap().to_string();
        assert_eq!(s(&head[..8], SearchMode::Sha), ["tweak"]);
        assert!(s("", SearchMode::Message).is_empty());
        assert_eq!(search_commits(&repo, "a", SearchMode::Author, 1).unwrap().len(), 1);
    }
}

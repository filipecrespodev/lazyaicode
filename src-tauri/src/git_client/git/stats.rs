use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::git_client::error::GitCommandError;

/// How far back `RepoStats::recent_commit_times` reaches — the activity
/// heatmap covers a year, plus a week so its first column is always full.
const ACTIVITY_WINDOW_SECS: i64 = 372 * 24 * 60 * 60;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorStats {
    pub name: String,
    pub email: String,
    pub commits: u32,
    /// Unix seconds of this author's most recent commit.
    pub last_commit_time: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStats {
    pub total_commits: u32,
    /// Unix seconds of the oldest commit; `None` for an empty repo.
    pub first_commit_time: Option<i64>,
    /// Sorted by commit count, most first.
    pub authors: Vec<AuthorStats>,
    /// Author times (unix seconds) of every commit in the last ~year. Raw
    /// timestamps rather than per-day buckets so the frontend buckets them by
    /// the viewer's local calendar day.
    pub recent_commit_times: Vec<i64>,
}

/// Statistics over the checked-out branch's history (everything reachable
/// from HEAD), like `git shortlog -sne HEAD`. Authors are grouped by
/// case-insensitive email; the name shown is the one on their latest commit.
pub fn repo_stats(repo: &git2::Repository, now: i64) -> Result<RepoStats, GitCommandError> {
    let empty = RepoStats {
        total_commits: 0,
        first_commit_time: None,
        authors: Vec::new(),
        recent_commit_times: Vec::new(),
    };
    let Ok(head) = repo.head() else {
        return Ok(empty); // unborn HEAD: nothing committed yet
    };
    let Some(head_oid) = head.target() else {
        return Ok(empty);
    };

    let mut revwalk = repo.revwalk()?;
    revwalk.push(head_oid)?;

    let mut total_commits = 0u32;
    let mut first_commit_time: Option<i64> = None;
    let mut authors: HashMap<String, AuthorStats> = HashMap::new();
    let mut recent_commit_times = Vec::new();
    let since = now - ACTIVITY_WINDOW_SECS;

    for oid in revwalk {
        let commit = repo.find_commit(oid?)?;
        let author = commit.author();
        let time = author.when().seconds();
        total_commits += 1;
        first_commit_time = Some(first_commit_time.map_or(time, |t| t.min(time)));
        if time >= since {
            recent_commit_times.push(time);
        }

        let email = author.email().unwrap_or("").to_string();
        let name = author.name().unwrap_or("").to_string();
        let entry = authors
            .entry(email.to_lowercase())
            .or_insert_with(|| AuthorStats {
                name: name.clone(),
                email: email.clone(),
                commits: 0,
                last_commit_time: time,
            });
        entry.commits += 1;
        if time >= entry.last_commit_time {
            entry.last_commit_time = time;
            entry.name = name;
        }
    }

    let mut authors: Vec<AuthorStats> = authors.into_values().collect();
    authors.sort_by(|a, b| b.commits.cmp(&a.commits).then(a.name.cmp(&b.name)));

    Ok(RepoStats {
        total_commits,
        first_commit_time,
        authors,
        recent_commit_times,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn commit_as(repo: &git2::Repository, name: &str, email: &str, time: i64, file: &str) {
        let dir = repo.workdir().unwrap();
        std::fs::write(dir.join(file), format!("{name}{time}")).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(file)).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::new(name, email, &git2::Time::new(time, 0)).unwrap();
        let parents: Vec<git2::Commit> =
            repo.head().ok().and_then(|h| h.peel_to_commit().ok()).into_iter().collect();
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, "msg", &tree, &parent_refs).unwrap();
    }

    #[test]
    fn empty_repo_has_no_stats() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let stats = repo_stats(&repo, 0).unwrap();
        assert_eq!(stats.total_commits, 0);
        assert!(stats.first_commit_time.is_none());
    }

    #[test]
    fn counts_commits_per_author_grouped_by_email() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let day = 24 * 60 * 60;
        let now = 1000 * day;
        // An old commit, outside the activity window.
        commit_as(&repo, "Ana", "ana@x.com", now - 500 * day, "a");
        commit_as(&repo, "Bob", "bob@x.com", now - 10 * day, "b");
        // Same person, different email case and a newer display name.
        commit_as(&repo, "Ana Silva", "ANA@x.com", now - day, "c");

        let stats = repo_stats(&repo, now).unwrap();
        assert_eq!(stats.total_commits, 3);
        assert_eq!(stats.first_commit_time, Some(now - 500 * day));
        assert_eq!(stats.authors.len(), 2);
        assert_eq!(stats.authors[0].name, "Ana Silva");
        assert_eq!(stats.authors[0].commits, 2);
        assert_eq!(stats.authors[0].last_commit_time, now - day);
        assert_eq!(stats.authors[1].name, "Bob");
        assert_eq!(stats.recent_commit_times.len(), 2);
    }
}

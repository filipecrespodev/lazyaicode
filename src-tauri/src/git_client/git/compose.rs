//! Splits the *staged* changes into several commits at hunk granularity.
//!
//! The staged diff is cut into "units" (zero-context hunks of modified text
//! files, or whole files for added/deleted/binary/mode-changed ones). The
//! caller assigns every unit to a commit; we then build each commit's tree
//! in memory (HEAD + the units of all commits so far), so the working tree
//! and index are never touched. Nothing moves until the last tree is proven
//! identical to the originally staged tree — so the commits always add up
//! to exactly what was staged.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::git_client::error::GitCommandError;
use crate::git_client::git::{hooks, signing};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Unit {
    /// `path#N` for a hunk, plain `path` for a whole-file unit.
    pub id: String,
    pub path: String,
    /// Human/AI readable patch text for this unit.
    pub text: String,
    #[serde(skip)]
    hunk: Option<Hunk>,
}

#[derive(Debug, Clone, Copy)]
struct Hunk {
    old_start: usize,
    old_lines: usize,
    new_start: usize,
    new_lines: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedUnits {
    /// Tree id of the index; `apply` refuses to run if it has changed since.
    pub tree_id: String,
    pub units: Vec<Unit>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposedCommitInput {
    pub summary: String,
    pub description: String,
    /// Unit ids.
    pub changes: Vec<String>,
}

fn err(m: impl Into<String>) -> GitCommandError {
    GitCommandError::Internal(m.into())
}

pub fn staged_units(repo: &git2::Repository) -> Result<StagedUnits, GitCommandError> {
    let head_tree = repo.head()?.peel_to_commit()?.tree()?;
    let mut index = repo.index()?;
    let tree_id = index.write_tree()?.to_string();
    let mut opts = git2::DiffOptions::new();
    opts.context_lines(0);
    let diff = repo.diff_tree_to_index(Some(&head_tree), Some(&index), Some(&mut opts))?;

    let mut units = Vec::new();
    for (i, delta) in diff.deltas().enumerate() {
        let path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy().into_owned())
            .ok_or_else(|| err("arquivo em staging sem caminho"))?;
        let Some(mut patch) = git2::Patch::from_diff(&diff, i)? else {
            continue;
        };
        let text_modify = delta.status() == git2::Delta::Modified
            && !delta.flags().is_binary()
            && delta.old_file().mode() == delta.new_file().mode()
            && patch.num_hunks() > 0;
        if !text_modify {
            let text = if delta.flags().is_binary() {
                "(arquivo binário)".to_string()
            } else {
                String::from_utf8_lossy(patch.to_buf()?.as_ref()).into_owned()
            };
            units.push(Unit { id: path.clone(), path, text, hunk: None });
            continue;
        }
        for h in 0..patch.num_hunks() {
            let (hunk, n_lines) = patch.hunk(h)?;
            let mut text = String::from_utf8_lossy(hunk.header()).into_owned();
            for l in 0..n_lines {
                let line = patch.line_in_hunk(h, l)?;
                text.push(line.origin());
                text.push_str(&String::from_utf8_lossy(line.content()));
            }
            units.push(Unit {
                id: format!("{path}#{h}"),
                path: path.clone(),
                text,
                hunk: Some(Hunk {
                    old_start: hunk.old_start() as usize,
                    old_lines: hunk.old_lines() as usize,
                    new_start: hunk.new_start() as usize,
                    new_lines: hunk.new_lines() as usize,
                }),
            });
        }
    }
    Ok(StagedUnits { tree_id, units })
}

fn lines(content: &[u8]) -> Vec<&[u8]> {
    content.split_inclusive(|b| *b == b'\n').collect()
}

/// `base` with only the `chosen` hunks (ascending, non-overlapping) taken
/// from `target`.
fn apply_hunks(base: &[u8], target: &[u8], chosen: &[Hunk]) -> Vec<u8> {
    let (b, t) = (lines(base), lines(target));
    let mut out: Vec<u8> = Vec::new();
    let mut cursor = 0;
    for h in chosen {
        // Zero-context headers: "-N,0" means "insert after line N".
        let start = if h.old_lines == 0 { h.old_start } else { h.old_start - 1 };
        b[cursor..start].iter().for_each(|l| out.extend_from_slice(l));
        let new_from = if h.new_lines == 0 { h.new_start } else { h.new_start - 1 };
        t[new_from..new_from + h.new_lines].iter().for_each(|l| out.extend_from_slice(l));
        cursor = start + h.old_lines;
    }
    b[cursor..].iter().for_each(|l| out.extend_from_slice(l));
    out
}

pub fn apply(
    repo: &git2::Repository,
    expected_tree_id: &str,
    commits: &[ComposedCommitInput],
    author_name: Option<&str>,
    author_email: Option<&str>,
    no_verify: bool,
) -> Result<Vec<String>, GitCommandError> {
    // pre-commit once, over everything staged, like a single `git commit`.
    hooks::pre_commit(repo, "pre-commit", no_verify)?;
    let staged = staged_units(repo)?;
    if staged.tree_id != expected_tree_id {
        return Err(err(
            "as alterações em staging mudaram desde que o plano foi feito (um hook pre-commit pode tê-las alterado); componha de novo",
        ));
    }
    let by_id: HashMap<&str, &Unit> = staged.units.iter().map(|u| (u.id.as_str(), u)).collect();
    let mut seen = HashSet::new();
    for id in commits.iter().flat_map(|c| &c.changes) {
        if !by_id.contains_key(id.as_str()) || !seen.insert(id.as_str()) {
            return Err(err(format!("alteração inválida ou duplicada \"{id}\"")));
        }
    }
    if seen.len() != by_id.len() {
        return Err(err("algumas alterações em staging não foram atribuídas a nenhum commit"));
    }

    let sig = match (author_name, author_email) {
        (Some(n), Some(e)) => git2::Signature::now(n, e)?,
        _ => repo.signature()?,
    };
    let head = repo.head()?;
    let mut parent = head.peel_to_commit()?;
    let head_tree = parent.tree()?;
    let staged_index = repo.index()?;

    let mut work = git2::Index::new()?;
    work.read_tree(&head_tree)?;
    let mut taken: HashSet<&str> = HashSet::new();
    let signer = signing::signer_for(repo)?;
    let mut created: Vec<git2::Commit> = Vec::new();

    for c in commits {
        taken.extend(c.changes.iter().map(String::as_str));
        let paths: Vec<&str> = c.changes.iter().map(|id| by_id[id.as_str()].path.as_str()).collect();
        for path in paths.iter().copied().collect::<HashSet<_>>() {
            let p = std::path::Path::new(path);
            let target = staged_index.get_path(p, 0);
            let Some(target) = target else {
                work.remove_path(p)?; // staged deletion
                continue;
            };
            let chosen: Vec<Hunk> = staged
                .units
                .iter()
                .filter(|u| u.path == path && taken.contains(u.id.as_str()))
                .filter_map(|u| u.hunk)
                .collect();
            let has_hunks = staged.units.iter().any(|u| u.path == path && u.hunk.is_some());
            let mut id = target.id;
            if has_hunks {
                let base = head_tree.get_path(p)?.to_object(repo)?.peel_to_blob()?;
                let full = repo.find_blob(target.id)?;
                id = repo.blob(&apply_hunks(base.content(), full.content(), &chosen))?;
            }
            work.add(&git2::IndexEntry {
                ctime: git2::IndexTime::new(0, 0),
                mtime: git2::IndexTime::new(0, 0),
                dev: 0,
                ino: 0,
                mode: target.mode,
                uid: 0,
                gid: 0,
                file_size: 0,
                id,
                flags: 0,
                flags_extended: 0,
                path: target.path.clone(),
            })?;
        }
        let tree = repo.find_tree(work.write_tree_to(repo)?)?;
        let message = if c.description.trim().is_empty() {
            c.summary.clone()
        } else {
            format!("{}\n\n{}", c.summary, c.description)
        };
        let message = hooks::prepare_message(repo, &message, "message", no_verify)?;
        let oid = signing::create_commit_with(repo, signer.as_ref(), &sig, &sig, &message, &tree, &[&parent])?;
        parent = repo.find_commit(oid)?;
        created.push(parent.clone());
    }

    if parent.tree_id().to_string() != expected_tree_id {
        return Err(err("erro interno: os commits compostos não correspondem às alterações em staging (nada foi commitado)"));
    }
    if head.is_branch() {
        repo.reference(head.name().unwrap_or("HEAD"), parent.id(), true, "compose commits")?;
    } else {
        repo.set_head_detached(parent.id())?;
    }
    hooks::run_post(repo, "post-commit", &[]);
    Ok(created.iter().map(|c| c.id().to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_one_file_into_two_commits_exactly() {
        let dir = std::env::temp_dir().join(format!("gh-compose-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = git2::Repository::init(&dir).unwrap();
        let sig = git2::Signature::now("T", "t@e.com").unwrap();
        let base = "a\nb\nc\nd\ne\nf\ng\nh\n";
        std::fs::write(dir.join("f.txt"), base).unwrap();
        let mut idx = repo.index().unwrap();
        idx.add_path(std::path::Path::new("f.txt")).unwrap();
        let t = repo.find_tree(idx.write_tree().unwrap()).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &t, &[]).unwrap();

        // change line 2 and add a line after 7, plus a new file
        std::fs::write(dir.join("f.txt"), "a\nB\nc\nd\ne\nf\ng\nNEW\nh\n").unwrap();
        std::fs::write(dir.join("n.txt"), "x\n").unwrap();
        idx.add_path(std::path::Path::new("f.txt")).unwrap();
        idx.add_path(std::path::Path::new("n.txt")).unwrap();
        idx.write().unwrap();

        let s = staged_units(&repo).unwrap();
        let ids: Vec<_> = s.units.iter().map(|u| u.id.as_str()).collect();
        assert_eq!(ids, ["f.txt#0", "f.txt#1", "n.txt"]);
        let c = |m: &str, ch: &[&str]| ComposedCommitInput {
            summary: m.into(),
            description: String::new(),
            changes: ch.iter().map(|s| s.to_string()).collect(),
        };
        let out = apply(&repo, &s.tree_id, &[c("one", &["f.txt#1"]), c("two", &["f.txt#0", "n.txt"])], None, None, false);
        assert_eq!(out.unwrap().len(), 2);
        let first = repo.head().unwrap().peel_to_commit().unwrap().parent(0).unwrap();
        let blob = first.tree().unwrap().get_path(std::path::Path::new("f.txt")).unwrap().to_object(&repo).unwrap();
        assert_eq!(blob.as_blob().unwrap().content(), b"a\nb\nc\nd\ne\nf\ng\nNEW\nh\n");
        assert_eq!(repo.head().unwrap().peel_to_commit().unwrap().tree_id().to_string(), s.tree_id);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

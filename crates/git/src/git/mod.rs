pub mod actor;
pub mod branch;
pub mod cherry_pick;
pub mod compose;
pub mod diff;
pub mod external_git;
pub mod fingerprint;
pub mod history;
pub mod hooks;
pub mod ignore;
pub mod interactive_rebase;
pub mod lfs;
pub mod log;
pub mod merge;
pub mod partial;
#[cfg(test)]
mod perf_tests;
pub mod pull;
pub mod rebase;
pub mod reflog;
pub mod remote;
pub mod repo;
pub mod revert;
pub mod search;
pub mod signing;
pub mod stash;
pub mod stats;
pub mod submodule;
pub mod undo;
pub mod working;
pub mod worktree;

/// O commit de uma revisão: o hash completo, um curto, `HEAD`, `main~1`,
/// uma tag… O que não resolver para um commit vira `CommitNotFound` (não
/// "repositório inválido", como antes, que culpava o repositório).
pub fn commit_oid(repo: &git2::Repository, rev: &str) -> Result<git2::Oid, crate::error::GitCommandError> {
    let not_found = || crate::error::GitCommandError::CommitNotFound { rev: rev.to_string() };
    if let Ok(oid) = git2::Oid::from_str(rev) {
        if rev.len() == 40 || rev.len() == 64 {
            return Ok(oid);
        }
    }
    let rev = rev.trim();
    if rev.is_empty() || rev.starts_with('-') {
        return Err(not_found());
    }
    repo.revparse_single(rev).and_then(|o| o.peel_to_commit()).map(|c| c.id()).map_err(|_| not_found())
}

#[cfg(test)]
mod commit_oid_tests {
    #[test]
    fn resolves_revisions_and_names_the_missing_commit() {
        let tmp = tempfile::tempdir().unwrap();
        lazyaicode_testkit::init_repo(tmp.path());
        let repo = git2::Repository::open(tmp.path()).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap().id();
        assert_eq!(super::commit_oid(&repo, "HEAD").unwrap(), head);
        assert_eq!(super::commit_oid(&repo, &head.to_string()).unwrap(), head);
        assert_eq!(super::commit_oid(&repo, &head.to_string()[..7]).unwrap(), head);
        assert_eq!(super::commit_oid(&repo, "main").unwrap(), head);
        for bad in ["nao-existe", "", "--all", "HEAD~5"] {
            let e = super::commit_oid(&repo, bad).unwrap_err();
            assert!(e.to_string().contains("commit não encontrado"), "{bad}: {e}");
        }
    }
}

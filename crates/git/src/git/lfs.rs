//! Git LFS detection. libgit2 doesn't run the LFS filters, so files the app
//! checks out stay pointer files until `git lfs pull` (see `external_git`).

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::external_git;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LfsStatus {
    pub uses_lfs: bool,
    /// `git lfs` is on PATH; only looked up when the repo uses LFS.
    pub installed: bool,
}

/// True when a `.gitattributes` text routes some path through the LFS filter.
pub fn uses_lfs(gitattributes: &str) -> bool {
    gitattributes
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .any(|l| l.split_whitespace().skip(1).any(|attr| attr == "filter=lfs"))
}

/// Only the top-level `.gitattributes` is read: one small file, which is
/// where `git lfs track` writes.
pub fn status(workdir: &Path) -> LfsStatus {
    let uses = std::fs::read_to_string(workdir.join(".gitattributes")).map(|text| uses_lfs(&text)).unwrap_or(false);
    LfsStatus { uses_lfs: uses, installed: uses && external_git::lfs_installed() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_lfs_tracked_patterns() {
        assert!(uses_lfs("*.psd filter=lfs diff=lfs merge=lfs -text\n"));
        assert!(uses_lfs("*.txt text\r\n  assets/** filter=lfs diff=lfs merge=lfs -text\r\n"));
        assert!(!uses_lfs("# *.psd filter=lfs diff=lfs merge=lfs -text\n*.sh text eol=lf\n"));
        assert!(!uses_lfs("*.bin -filter\n"));
        assert!(!uses_lfs(""));
        // a pattern literally named like the attribute isn't an attribute
        assert!(!uses_lfs("filter=lfs text\n"));
    }

    #[test]
    fn reads_the_repo_root_attributes() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!status(dir.path()).uses_lfs);
        std::fs::write(dir.path().join(".gitattributes"), "*.png filter=lfs -text\n").unwrap();
        assert!(status(dir.path()).uses_lfs);
    }
}

//! Adding ignore patterns: to the repo's `.gitignore` (shared, committed) or
//! to `.git/info/exclude` (this clone only).

use crate::git_client::activity;
use crate::git_client::error::GitCommandError;

/// `existing` with `pattern` appended on its own line, or `None` when the
/// pattern is already there. Keeps the file's line ending style and always
/// leaves a trailing newline.
pub fn append_pattern(existing: &str, pattern: &str) -> Option<String> {
    if existing.lines().any(|line| line.trim_end() == pattern) {
        return None;
    }
    let newline = if existing.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut out = String::with_capacity(existing.len() + pattern.len() + 2);
    out.push_str(existing);
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(newline);
    }
    out.push_str(pattern);
    out.push_str(newline);
    Some(out)
}

/// Appends `pattern` to the ignore file; `false` when it was already listed.
pub fn add_pattern(
    repo: &git2::Repository,
    pattern: &str,
    local: bool,
) -> Result<bool, GitCommandError> {
    let pattern = pattern.trim();
    if pattern.is_empty() || pattern.contains('\n') || pattern.contains('\r') {
        return Err(GitCommandError::Internal(format!(
            "padrão de ignore inválido: {pattern:?}"
        )));
    }
    let workdir = repo.workdir().ok_or_else(|| {
        GitCommandError::Internal("repositório bare não tem diretório de trabalho".into())
    })?;
    let (file, shown) = if local {
        // The common dir, not `path()`: a linked worktree's own gitdir has no
        // info/exclude of its own; git reads the main repository's.
        (
            repo.commondir().join("info").join("exclude"),
            ".git/info/exclude",
        )
    } else {
        (workdir.join(".gitignore"), ".gitignore")
    };
    let io = |e: std::io::Error| GitCommandError::Internal(format!("{shown}: {e}"));

    let existing = match std::fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(io(e)),
    };
    let Some(updated) = append_pattern(&existing, pattern) else {
        return Ok(false);
    };
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    activity::info(Some(workdir), format!("$ echo '{pattern}' >> {shown}"));
    std::fs::write(&file, updated).map_err(io)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_to_an_empty_or_missing_file() {
        assert_eq!(append_pattern("", "*.log").unwrap(), "*.log\n");
    }

    #[test]
    fn adds_the_missing_trailing_newline_first() {
        assert_eq!(
            append_pattern("target", "*.log").unwrap(),
            "target\n*.log\n"
        );
        assert_eq!(
            append_pattern("target\n", "*.log").unwrap(),
            "target\n*.log\n"
        );
    }

    #[test]
    fn skips_duplicates_regardless_of_line_endings_and_trailing_spaces() {
        assert_eq!(append_pattern("a\n*.log\nb\n", "*.log"), None);
        assert_eq!(append_pattern("a\r\n*.log  \r\n", "*.log"), None);
        // Not a duplicate: a different (anchored) pattern.
        assert!(append_pattern("*.log\n", "/*.log").is_some());
    }

    #[test]
    fn keeps_crlf_files_crlf() {
        assert_eq!(append_pattern("a\r\nb", "c").unwrap(), "a\r\nb\r\nc\r\n");
    }

    #[test]
    fn writes_gitignore_or_info_exclude_and_hides_the_file_from_status() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        std::fs::write(tmp.path().join("debug.log"), "x").unwrap();
        std::fs::write(tmp.path().join("secret.txt"), "x").unwrap();

        assert!(add_pattern(&repo, "*.log", false).unwrap());
        assert!(!add_pattern(&repo, "*.log", false).unwrap());
        assert_eq!(
            std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap(),
            "*.log\n"
        );

        assert!(add_pattern(&repo, "/secret.txt", true).unwrap());
        let exclude = std::fs::read_to_string(repo.path().join("info/exclude")).unwrap();
        assert!(exclude.ends_with("/secret.txt\n"));

        assert!(repo
            .status_should_ignore(std::path::Path::new("debug.log"))
            .unwrap());
        assert!(repo
            .status_should_ignore(std::path::Path::new("secret.txt"))
            .unwrap());
        assert!(add_pattern(&repo, " \n", false).is_err());
    }

    #[test]
    fn ignores_locally_from_a_linked_worktree() {
        let tmp = tempfile::tempdir().unwrap();
        let main_dir = tmp.path().join("main");
        let repo = git2::Repository::init(&main_dir).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let tree = repo
            .find_tree(repo.index().unwrap().write_tree().unwrap())
            .unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();
        let wt_dir = tmp.path().join("wt");
        let wt = repo.worktree("wt", &wt_dir, None).unwrap();
        let wt_repo = git2::Repository::open_from_worktree(&wt).unwrap();
        std::fs::write(wt_dir.join("scratch.txt"), "x").unwrap();

        assert!(add_pattern(&wt_repo, "/scratch.txt", true).unwrap());
        let exclude = std::fs::read_to_string(repo.path().join("info/exclude")).unwrap();
        assert!(exclude.ends_with("/scratch.txt\n"));
        assert!(wt_repo
            .status_should_ignore(std::path::Path::new("scratch.txt"))
            .unwrap());
    }
}

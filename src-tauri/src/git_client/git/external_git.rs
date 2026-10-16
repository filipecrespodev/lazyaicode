use std::path::Path;
use std::process::{Command, Stdio};

use crate::git_client::activity;

/// No flashing console window for git child processes on Windows.
#[cfg(windows)]
fn no_window(cmd: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000) // CREATE_NO_WINDOW
}

#[cfg(not(windows))]
fn no_window(cmd: &mut Command) -> &mut Command {
    cmd
}
use crate::git_client::error::GitCommandError;

/// Continues/aborts a rebase by shelling out to the system `git` binary.
/// Needed only for rebases libgit2 can't open itself — most notably
/// interactive rebases (`git rebase -i`), which `git2::Repository::open_rebase`
/// categorically refuses ("interactive rebase is not supported"). Everything
/// else in this app talks to git2/libgit2 directly; this is the one
/// deliberate exception, scoped to what libgit2 can't do (interactive
/// rebases, Git LFS).
fn run(repo_path: &Path, args: &[&str]) -> Result<(), GitCommandError> {
    run_with_env(repo_path, args, &[])
}

fn run_with_env(
    repo_path: &Path,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<(), GitCommandError> {
    activity::info(Some(repo_path), format!("$ git {}", args.join(" ")));
    let output = no_window(&mut Command::new("git"))
        .current_dir(repo_path)
        .args(args)
        // Continuing a plain `pick` step never needs an editor, but a
        // `reword`/`squash`/`edit` step further down the todo list might —
        // fail fast instead of hanging on a prompt we can't answer.
        .env("GIT_EDITOR", "true")
        .env("GIT_SEQUENCE_EDITOR", "true")
        .env("GIT_TERMINAL_PROMPT", "0")
        .envs(env.iter().copied())
        .stdin(Stdio::null())
        .output()
        .map_err(|e| GitCommandError::Internal(format!("falha ao executar o git: {e}")))?;

    if !output.status.success() {
        let mut stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        // Algumas falhas (ex.: "could not apply ...") só se explicam no stdout.
        if stderr.is_empty() {
            stderr = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
        activity::error(Some(repo_path), format!("git {} falhou: {stderr}", args.join(" ")));
        return Err(GitCommandError::Internal(stderr));
    }
    Ok(())
}

/// `env`: see [`super::signing::git_cli_env`] (later steps make commits too).
pub fn rebase_continue(repo_path: &Path, env: &[(String, String)]) -> Result<(), GitCommandError> {
    run_with_env(repo_path, &["rebase", "--continue"], &borrowed(env))
}

fn borrowed(env: &[(String, String)]) -> Vec<(&str, &str)> {
    env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
}

pub fn rebase_abort(repo_path: &Path) -> Result<(), GitCommandError> {
    run(repo_path, &["rebase", "--abort"])
}

/// `git rebase -i` with the todo list at `todo` (see `interactive_rebase`).
/// `upstream: None` rebases from the root commit. Git's own `--autostash`
/// takes care of uncommitted changes, restoring them when it finishes or aborts.
/// `env`: what signs the commits it makes ([`super::signing::git_cli_env`]).
pub fn rebase_interactive(
    repo_path: &Path,
    upstream: Option<&str>,
    todo: &Path,
    env: &[(String, String)],
) -> Result<(), GitCommandError> {
    let editor = format!("cp {}", super::interactive_rebase::sh_quote(todo));
    // `--empty=drop`: a commit emptied by the reordering would otherwise
    // stop the rebase with nothing to resolve.
    let args = ["rebase", "-i", "--autostash", "--empty=drop", upstream.unwrap_or("--root")];
    let mut env = borrowed(env);
    env.push(("GIT_SEQUENCE_EDITOR", &editor));
    run_with_env(repo_path, &args, &env)
}

/// libgit2 doesn't run the LFS smudge filter, so checkouts it makes leave
/// pointer files; this fetches and checks out the real content.
///
/// git-lfs talks to SSH remotes by running `ssh` itself, outside the app's
/// credential callbacks: with no terminal, a passphrase or host-key prompt
/// would hang the pull forever. So unless the user configured their own
/// SSH command, it runs in batch mode (agent keys still work) with the
/// key picked for this repo in the app.
pub fn lfs_pull(repo_path: &Path) -> Result<(), GitCommandError> {
    let custom_ssh = std::env::var_os("GIT_SSH_COMMAND").is_some()
        || std::env::var_os("GIT_SSH").is_some()
        || git2::Repository::open(repo_path)
            .and_then(|r| r.config()?.get_string("core.sshCommand"))
            .is_ok();
    let ssh = (!custom_ssh)
        .then(|| batch_ssh_command(crate::git_client::ssh_keys::configured_key(repo_path).as_deref()));
    let env: Vec<(&str, &str)> = ssh.iter().map(|s| ("GIT_SSH_COMMAND", s.as_str())).collect();
    run_with_env(repo_path, &["lfs", "pull"], &env)
}

/// `GIT_SSH_COMMAND` that fails instead of prompting. Git runs it through
/// `sh`, hence the quoting.
fn batch_ssh_command(key: Option<&Path>) -> String {
    match key {
        Some(key) => format!(
            "ssh -o BatchMode=yes -o IdentitiesOnly=yes -i {}",
            super::interactive_rebase::sh_quote(key)
        ),
        None => "ssh -o BatchMode=yes".into(),
    }
}

/// Whether `git lfs` is available — asked once per app run.
pub fn lfs_installed() -> bool {
    static INSTALLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *INSTALLED.get_or_init(|| {
        no_window(&mut Command::new("git"))
            .args(["lfs", "version"])
            .stdin(Stdio::null())
            .output()
            .is_ok_and(|o| o.status.success())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lfs_ssh_never_prompts() {
        assert_eq!(batch_ssh_command(None), "ssh -o BatchMode=yes");
        assert_eq!(
            batch_ssh_command(Some(Path::new("/home/me/.ssh/it's"))),
            r"ssh -o BatchMode=yes -o IdentitiesOnly=yes -i '/home/me/.ssh/it'\''s'"
        );
    }
}

//! Runs git hooks around the commits the app creates. libgit2 never runs
//! hooks, so without this husky / lint-staged / commit-msg linters would be
//! silently skipped. Hooks are spawned the way git does it: from the hooks
//! dir (`core.hooksPath` or `.git/hooks`), cwd = workdir, `GIT_DIR` and
//! `GIT_INDEX_FILE` set, and on Windows through Git for Windows' `sh`.
//! Their output is streamed to the Activity Logs; there's no timeout (a slow
//! hook only holds this repo's actor thread, never the UI).

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::activity;
use crate::error::GitCommandError;

/// `core.hooksPath` (relative paths are relative to the workdir, like git)
/// or the repo's own hooks dir, shared by all its worktrees.
pub fn hooks_dir(repo: &git2::Repository) -> PathBuf {
    let base = repo.workdir().unwrap_or_else(|| repo.path());
    let configured = repo.config().and_then(|c| c.get_path("core.hooksPath")).ok();
    match configured {
        Some(path) if path.is_absolute() => path,
        Some(path) => base.join(path),
        None => repo.commondir().join("hooks"),
    }
}

/// The hook's file if it exists and git would run it.
fn find(repo: &git2::Repository, name: &str) -> Option<PathBuf> {
    let path = hooks_dir(repo).join(name);
    if !path.is_file() {
        return None;
    }
    if !is_executable(&path) {
        activity::info(repo.workdir(), format!("dica: o hook '{name}' foi ignorado porque não está marcado como executável"));
        return None;
    }
    Some(path)
}

pub fn exists(repo: &git2::Repository, name: &str) -> bool {
    find(repo, name).is_some()
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata().map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

/// Windows has no exec bit; git for Windows runs any hook file it finds.
#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}

/// Runs hook `name` if present. `Ok(false)` when there is none. A non-zero
/// exit becomes [`GitCommandError::HookFailed`] carrying its output.
pub fn run(repo: &git2::Repository, name: &str, args: &[&str]) -> Result<bool, GitCommandError> {
    let Some(path) = find(repo, name) else {
        return Ok(false);
    };
    let workdir = repo.workdir().unwrap_or_else(|| repo.path()).to_path_buf();
    let shown = display_path(&path, &workdir);
    let log_dir = Some(workdir.as_path());
    activity::info(log_dir, format!("$ {shown} {}", args.join(" ")).trim_end().to_string());

    let mut child =
        spawn(&path, args, &workdir, repo).map_err(|e| GitCommandError::Internal(format!("falha ao executar o hook {name}: {e}")))?;
    // Both pipes are drained on their own threads (a full pipe would stall
    // the hook) and relayed in arrival order. Lines are lossily decoded: a
    // tool printing non-UTF-8 must not stop the draining.
    let (tx, rx) = mpsc::channel::<String>();
    let readers: Vec<_> = [
        child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>),
        child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    .map(|pipe| {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(pipe);
            let mut buf = Vec::new();
            while matches!(reader.read_until(b'\n', &mut buf), Ok(n) if n > 0) {
                let line = String::from_utf8_lossy(&buf);
                let _ = tx.send(line.trim_end_matches(['\n', '\r']).to_string());
                buf.clear();
            }
        })
    })
    .collect();
    drop(tx);
    let output = relay_output(rx, |chunk| activity::info(log_dir, chunk));
    readers.into_iter().for_each(|r| {
        let _ = r.join();
    });
    let status = child.wait().map_err(|e| GitCommandError::Internal(format!("{name} hook: {e}")))?;

    if status.success() {
        activity::info(log_dir, format!("{shown} terminou com código 0"));
        return Ok(true);
    }
    let code = status.code().map_or("signal".to_string(), |c| c.to_string());
    activity::error(log_dir, format!("{shown} falhou (código de saída {code})"));
    Err(GitCommandError::HookFailed { hook: name.to_string(), output: Vec::from(output).join("\n") })
}

/// Lines a failed hook's error keeps (the UI shows the tail; the Activity
/// Logs have everything).
const KEPT_LINES: usize = 200;
/// Output is sent to the Activity Logs in chunks: one event per line would
/// flood the UI when a hook runs a chatty test suite.
const FLUSH_LINES: usize = 100;
const FLUSH_EVERY: Duration = Duration::from_millis(150);

/// Relays `rx`'s lines to `emit` in multi-line chunks (by count or time,
/// whichever comes first) and returns the last [`KEPT_LINES`] of them.
fn relay_output(rx: mpsc::Receiver<String>, emit: impl Fn(String)) -> VecDeque<String> {
    let mut kept = VecDeque::new();
    let mut pending: Vec<String> = Vec::new();
    let mut since = Instant::now();
    loop {
        let next = rx.recv_timeout(FLUSH_EVERY.saturating_sub(since.elapsed()));
        let done = matches!(next, Err(mpsc::RecvTimeoutError::Disconnected));
        if let Ok(line) = next {
            if kept.len() == KEPT_LINES {
                kept.pop_front();
            }
            kept.push_back(line.clone());
            pending.push(line);
        }
        if !pending.is_empty() && (done || pending.len() >= FLUSH_LINES || since.elapsed() >= FLUSH_EVERY) {
            emit(std::mem::take(&mut pending).join("\n"));
        }
        if done {
            return kept;
        }
        if pending.is_empty() {
            since = Instant::now();
        }
    }
}

/// Like [`run`], but a failure is only logged: post-* hooks can't undo
/// what already happened, and git ignores their exit code too.
pub fn run_post(repo: &git2::Repository, name: &str, args: &[&str]) {
    let _ = run(repo, name, args);
}

fn spawn(hook: &Path, args: &[&str], workdir: &Path, repo: &git2::Repository) -> std::io::Result<std::process::Child> {
    let build = |via_shell: bool| {
        let mut cmd = if via_shell {
            let mut c = Command::new(shell());
            // MSYS sh copes best with `C:/…` paths.
            c.arg(hook.to_string_lossy().replace('\\', "/"));
            c
        } else {
            Command::new(hook)
        };
        cmd.args(args)
            .current_dir(workdir)
            .env("GIT_DIR", repo.path())
            .env("GIT_INDEX_FILE", repo.path().join("index"))
            // Hooks that would open an editor get a no-op one, like `git commit -m`.
            .env("GIT_EDITOR", ":")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        no_console_window(&mut cmd);
        cmd
    };
    if cfg!(windows) {
        return build(true).spawn();
    }
    // ETXTBSY: the hook was just written (husky, a test) and another thread
    // forked while its write fd was still open, so exec sees it "busy" for a
    // few ms. Git itself never hits this (single-threaded); we retry briefly.
    let mut tries = 0;
    loop {
        match build(false).spawn() {
            // A script without a shebang: git falls back to sh, so do we.
            Err(e) if e.raw_os_error() == Some(8 /* ENOEXEC */) => return build(true).spawn(),
            Err(e) if e.raw_os_error() == Some(26 /* ETXTBSY */) && tries < 20 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            r => return r,
        }
    }
}

#[cfg(windows)]
fn no_console_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}

#[cfg(not(windows))]
fn no_console_window(_cmd: &mut Command) {}

/// `sh` to run hooks with. On Windows that's Git for Windows' own, found
/// next to the `git.exe` on PATH (`…\Git\cmd\git.exe` → `…\Git\bin\sh.exe`).
fn shell() -> PathBuf {
    #[cfg(windows)]
    {
        let from_path = std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>());
        for dir in from_path {
            if dir.join("sh.exe").is_file() {
                return dir.join("sh.exe");
            }
            if dir.join("git.exe").is_file() {
                // From Git\cmd, or Git\mingw64\bin.
                for rel in ["../bin/sh.exe", "../usr/bin/sh.exe", "../../bin/sh.exe"] {
                    let sh = dir.join(rel);
                    if sh.is_file() {
                        return sh;
                    }
                }
            }
        }
        let default = PathBuf::from(r"C:\Program Files\Git\bin\sh.exe");
        if default.is_file() {
            return default;
        }
    }
    PathBuf::from("sh")
}

/// Hook path as the user would type it: relative to the workdir when inside.
fn display_path(hook: &Path, workdir: &Path) -> String {
    hook.strip_prefix(workdir).unwrap_or(hook).to_string_lossy().replace('\\', "/")
}

/// Hooks git runs between `pre-commit` and `post-commit`: what the commit
/// message ends up being. Writes it to `COMMIT_EDITMSG`, lets
/// `prepare-commit-msg` (always) and `commit-msg` (unless `no_verify`)
/// edit it, then reads it back with git's whitespace cleanup. Without any
/// of those hooks the message is returned as is, touching no files.
pub fn prepare_message(repo: &git2::Repository, message: &str, source: &str, no_verify: bool) -> Result<String, GitCommandError> {
    let prepare = exists(repo, "prepare-commit-msg");
    let verify = !no_verify && exists(repo, "commit-msg");
    if !prepare && !verify {
        return Ok(message.to_string());
    }
    let file = repo.path().join("COMMIT_EDITMSG");
    let io = |e: std::io::Error| GitCommandError::Internal(format!("COMMIT_EDITMSG: {e}"));
    std::fs::write(&file, ensure_newline(message)).map_err(io)?;
    // Relative to the hook's cwd, as git passes it (`.git/COMMIT_EDITMSG`):
    // husky-style hooks use `$1` unquoted, which an absolute path with a
    // space in it (C:\Users\Jane Doe\…) would split.
    let file_arg = match repo.workdir() {
        Some(workdir) => display_path(&file, workdir),
        None => file.to_string_lossy().into_owned(),
    };
    if prepare {
        run(repo, "prepare-commit-msg", &[&file_arg, source])?;
    }
    if verify {
        run(repo, "commit-msg", &[&file_arg])?;
    }
    let edited = std::fs::read_to_string(&file).map_err(io)?;
    let cleaned = git2::message_prettify(&edited, None)?;
    if cleaned.trim().is_empty() {
        return Err(GitCommandError::Internal("commit abortado: a mensagem ficou vazia (após os hooks)".into()));
    }
    Ok(cleaned.trim_end().to_string())
}

fn ensure_newline(message: &str) -> String {
    if message.ends_with('\n') {
        message.to_string()
    } else {
        format!("{message}\n")
    }
}

/// `pre-commit` (unless `no_verify`), then reloads the index from disk:
/// hooks like lint-staged re-stage files, and libgit2 would otherwise keep
/// committing its stale in-memory copy.
pub fn pre_commit(repo: &git2::Repository, hook: &str, no_verify: bool) -> Result<(), GitCommandError> {
    if !no_verify && run(repo, hook, &[])? {
        repo.index()?.read(true)?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
pub(crate) mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Installs an executable hook script in `dir`.
    pub fn write_hook(dir: &Path, name: &str, body: &str) {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn hooks_dir_honours_core_hooks_path_relative_to_workdir() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        assert_eq!(hooks_dir(&repo), repo.path().join("hooks"));
        repo.config().unwrap().set_str("core.hooksPath", ".husky").unwrap();
        assert_eq!(hooks_dir(&repo), repo.workdir().unwrap().join(".husky"));
    }

    #[test]
    fn missing_or_non_executable_hooks_are_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        assert!(!run(&repo, "pre-commit", &[]).unwrap());
        std::fs::write(repo.path().join("hooks/pre-commit"), "#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(repo.path().join("hooks/pre-commit"), std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!run(&repo, "pre-commit", &[]).unwrap());
    }

    #[test]
    fn failing_hook_reports_its_output_and_env() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        write_hook(&repo.path().join("hooks"), "pre-commit", "echo \"dir=$GIT_DIR\"; echo \"cwd=$(pwd)\"; echo nope >&2; exit 3");
        match run(&repo, "pre-commit", &[]) {
            Err(GitCommandError::HookFailed { hook, output }) => {
                assert_eq!(hook, "pre-commit");
                assert!(output.contains("nope"), "{output}");
                assert!(output.contains(&format!("dir={}", repo.path().display())), "{output}");
                let workdir = repo.workdir().unwrap().canonicalize().unwrap();
                assert!(output.contains(&format!("cwd={}", workdir.display())), "{output}");
            }
            other => panic!("expected HookFailed, got {other:?}"),
        }
    }

    #[test]
    fn script_without_shebang_runs_through_sh() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let path = repo.path().join("hooks/pre-commit");
        std::fs::write(&path, "exit 0\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(run(&repo, "pre-commit", &[]).unwrap());
    }

    #[test]
    fn message_hooks_edit_the_message_and_no_verify_skips_commit_msg() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        assert_eq!(prepare_message(&repo, "plain", "message", false).unwrap(), "plain");
        let hooks = repo.path().join("hooks");
        write_hook(
            &hooks,
            "prepare-commit-msg",
            "test \"$2\" = message && printf '[pre] ' | cat - \"$1\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\"",
        );
        write_hook(&hooks, "commit-msg", "echo 'Signed-off-by: hook' >> \"$1\"");
        assert_eq!(prepare_message(&repo, "feat: x", "message", false).unwrap(), "[pre] feat: x\nSigned-off-by: hook");
        assert_eq!(prepare_message(&repo, "feat: x", "message", true).unwrap(), "[pre] feat: x");
    }

    #[test]
    fn message_file_is_passed_relative_so_unquoted_args_survive_spaces() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path().join("my repo")).unwrap();
        // husky's own template: `$1` unquoted.
        write_hook(&repo.path().join("hooks"), "commit-msg", "echo \"arg=$1\" >&2; echo 'Refs: x' >> $1");
        assert_eq!(prepare_message(&repo, "feat: x", "message", false).unwrap(), "feat: x\nRefs: x");
    }

    #[test]
    fn non_utf8_and_long_output_is_drained_and_its_tail_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        // Way past a pipe's buffer, after a non-UTF-8 line.
        write_hook(
            &repo.path().join("hooks"),
            "pre-commit",
            "printf 'caf\\351\\n'; i=0; while [ $i -lt 20000 ]; do echo \"line $i\"; i=$((i+1)); done; echo last; exit 1",
        );
        match run(&repo, "pre-commit", &[]) {
            Err(GitCommandError::HookFailed { output, .. }) => {
                let lines: Vec<_> = output.lines().collect();
                assert_eq!(lines.len(), KEPT_LINES);
                assert_eq!(lines.last(), Some(&"last"));
            }
            other => panic!("expected HookFailed, got {other:?}"),
        }
    }

    #[test]
    fn output_is_relayed_in_chunks() {
        let (tx, rx) = mpsc::channel();
        for i in 0..250 {
            tx.send(format!("l{i}")).unwrap();
        }
        drop(tx);
        let chunks = std::cell::RefCell::new(Vec::new());
        let kept = relay_output(rx, |c| chunks.borrow_mut().push(c));
        let chunks = chunks.into_inner();
        assert_eq!(chunks.len(), 3, "100 + 100 + 50 lines");
        assert_eq!(chunks.concat().matches('l').count(), 250);
        assert_eq!(kept.len(), KEPT_LINES);
        assert_eq!(kept.back().map(String::as_str), Some("l249"));
    }

    #[test]
    fn hook_emptying_the_message_aborts() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        write_hook(&repo.path().join("hooks"), "commit-msg", ": > \"$1\"");
        assert!(prepare_message(&repo, "feat: x", "message", false).is_err());
    }
}

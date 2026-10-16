use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::error::GitCommandError;
use crate::{activity, https_credentials, passphrase, ssh_keys};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchSummary {
    pub remote_name: String,
    pub received_objects: usize,
    pub total_objects: usize,
    /// Refs (remote-tracking branches, tags) the fetch created or moved. 0
    /// lets background fetches skip refreshing the graph when nothing changed.
    pub updated_refs: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushSummary {
    pub remote_name: String,
    pub branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub name: String,
    pub fetch_url: Option<String>,
    /// Only when set apart from the fetch URL (`remote.<name>.pushurl`).
    pub push_url: Option<String>,
}

pub fn fetch(
    repo: &git2::Repository,
    remote_name: &str,
    mode: AuthMode,
    mut on_progress: impl FnMut(/* received_objects */ usize, /* total_objects */ usize, /* received_bytes */ usize) + 'static,
) -> Result<FetchSummary, GitCommandError> {
    let mut remote = repo.find_remote(remote_name)?;
    log_network(repo, &remote, &format!("fetch {remote_name}"), mode);
    let Auth { mut callbacks, locked } = Auth::new(repo, mode);
    callbacks.transfer_progress(move |progress| {
        on_progress(progress.received_objects(), progress.total_objects(), progress.received_bytes());
        true
    });
    let updated = Rc::new(Cell::new(0));
    let updated_cb = updated.clone();
    callbacks.update_tips(move |_, _, _| {
        updated_cb.set(updated_cb.get() + 1);
        true
    });

    let mut opts = git2::FetchOptions::new();
    opts.remote_callbacks(callbacks);
    remote.fetch(&[] as &[&str], Some(&mut opts), None).map_err(|e| Auth::error(&locked, e))?;

    let stats = remote.stats();
    Ok(FetchSummary {
        remote_name: remote_name.to_string(),
        received_objects: stats.received_objects(),
        total_objects: stats.total_objects(),
        updated_refs: updated.get(),
    })
}

/// `remote_name`, or with `None` every remote (`git fetch --all`, so not
/// those with `remote.<name>.skipFetchAll`), one after the other. A failing
/// remote doesn't stop the rest; the first error is returned once all were
/// tried. No remotes at all is not an error.
pub fn fetch_remotes(
    repo: &git2::Repository,
    remote_name: Option<&str>,
    mode: AuthMode,
    on_progress: impl FnMut(usize, usize, usize) + 'static,
) -> Result<FetchSummary, GitCommandError> {
    let names = match remote_name {
        Some(name) => vec![name.to_string()],
        None => {
            let config = repo.config()?.snapshot()?;
            let mut names = remote_names(repo)?;
            names.retain(|n| !config.get_bool(&format!("remote.{n}.skipFetchAll")).unwrap_or(false));
            names
        }
    };
    let on_progress = Rc::new(RefCell::new(on_progress));
    let mut summary = FetchSummary { remote_name: names.join(", "), received_objects: 0, total_objects: 0, updated_refs: 0 };
    let mut first_error = None;
    for name in &names {
        let progress = on_progress.clone();
        match fetch(repo, name, mode, move |r, t, b| (progress.borrow_mut())(r, t, b)) {
            Ok(s) => {
                summary.received_objects += s.received_objects;
                summary.total_objects += s.total_objects;
                summary.updated_refs += s.updated_refs;
            }
            Err(e) => {
                let dir = repo.workdir().unwrap_or(repo.path());
                activity::error(Some(dir), format!("fetch de {name} falhou: {e}"));
                first_error.get_or_insert(e);
            }
        }
    }
    match first_error {
        Some(e) => Err(e),
        None => Ok(summary),
    }
}

/// Remote names, `origin` first (it's the default everywhere), then by name.
pub(crate) fn remote_names(repo: &git2::Repository) -> Result<Vec<String>, GitCommandError> {
    let mut names: Vec<String> = repo.remotes()?.iter().flatten().flatten().map(String::from).collect();
    names.sort_by_key(|n| (n != "origin", n.clone()));
    Ok(names)
}

pub fn list_remotes(repo: &git2::Repository) -> Result<Vec<RemoteInfo>, GitCommandError> {
    remote_names(repo)?
        .into_iter()
        .map(|name| {
            let remote = repo.find_remote(&name)?;
            Ok(RemoteInfo {
                fetch_url: remote.url().ok().filter(|u| !u.is_empty()).map(String::from),
                push_url: remote.pushurl().ok().flatten().filter(|u| !u.is_empty()).map(String::from),
                name,
            })
        })
        .collect()
}

fn valid_remote_name(name: &str) -> Result<(), GitCommandError> {
    if git2::Remote::is_valid_name(name) {
        Ok(())
    } else {
        Err(GitCommandError::Internal(format!("\"{name}\" não é um nome de remote válido")))
    }
}

pub fn add_remote(repo: &git2::Repository, name: &str, url: &str) -> Result<(), GitCommandError> {
    valid_remote_name(name)?;
    repo.remote(name, url.trim())?;
    Ok(())
}

/// Also drops its remote-tracking branches and upstream settings, like `git remote remove`.
pub fn remove_remote(repo: &git2::Repository, name: &str) -> Result<(), GitCommandError> {
    repo.remote_delete(name)?;
    Ok(())
}

/// Renames its remote-tracking branches and upstream settings too, like `git remote rename`.
pub fn rename_remote(repo: &git2::Repository, from: &str, to: &str) -> Result<(), GitCommandError> {
    valid_remote_name(to)?;
    let problems = repo.remote_rename(from, to)?;
    // Custom fetch refspecs libgit2 couldn't rewrite; the rename itself worked.
    let dir = repo.workdir().unwrap_or(repo.path());
    for problem in problems.iter().flatten().flatten() {
        activity::error(Some(dir), format!("renomear remote: o refspec {problem} ficou inalterado"));
    }
    Ok(())
}

pub fn set_remote_url(repo: &git2::Repository, name: &str, url: &str) -> Result<(), GitCommandError> {
    repo.find_remote(name)?;
    repo.remote_set_url(name, url.trim())?;
    Ok(())
}

/// Picks the remote `git push` would use for a branch: the first configured
/// candidate (`branch.<b>.pushRemote`, `remote.pushDefault`, the upstream's
/// remote) that exists, else `origin`, else the only remote. `None` when
/// that's ambiguous (or there are no remotes).
fn pick_push_remote(configured: &[Option<String>], remotes: &[String]) -> Option<String> {
    configured
        .iter()
        .flatten()
        .find(|name| remotes.contains(name))
        .cloned()
        .or_else(|| remotes.iter().find(|n| *n == "origin").cloned())
        .or_else(|| (remotes.len() == 1).then(|| remotes[0].clone()))
}

pub fn push_remote_for(repo: &git2::Repository, branch: &str) -> Result<String, GitCommandError> {
    let config = repo.config()?;
    let get = |key: &str| config.get_string(key).ok();
    let upstream = repo.branch_upstream_remote(&format!("refs/heads/{branch}")).ok().and_then(|buf| buf.as_str().ok().map(String::from));
    let configured = [get(&format!("branch.{branch}.pushRemote")), get("remote.pushDefault"), upstream];
    let remotes = remote_names(repo)?;
    pick_push_remote(&configured, &remotes).ok_or(GitCommandError::RemoteRequired { remotes })
}

/// Raw URL configured for a remote (e.g. `git@host:owner/repo.git` or
/// `https://host/owner/repo.git`), used to identify which code-hosting
/// provider/instance a repo's `origin` points at. `None` if the remote has no
/// URL configured (unusual, but `git2` allows it).
pub fn get_remote_url(repo: &git2::Repository, remote_name: &str) -> Result<Option<String>, GitCommandError> {
    let remote = repo.find_remote(remote_name)?;
    let url = remote.url()?;
    Ok(if url.is_empty() { None } else { Some(url.to_string()) })
}

pub fn push(
    repo: &git2::Repository,
    remote_name: &str,
    branch: &str,
    force: bool,
    mut on_progress: impl FnMut(/* current */ u32, /* total */ u32, /* bytes */ usize) + 'static,
) -> Result<PushSummary, GitCommandError> {
    let mut remote = repo.find_remote(remote_name)?;
    let Auth { mut callbacks, .. } = Auth::new(repo, AuthMode::Interactive);
    callbacks.push_transfer_progress(move |current, total, bytes| {
        on_progress(current as u32, total as u32, bytes);
    });

    let rejected: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let rejected_cb = rejected.clone();
    callbacks.push_update_reference(move |refname, status| {
        if let Some(message) = status {
            *rejected_cb.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(format!("{refname}: {message}"));
        }
        Ok(())
    });

    let mut opts = git2::PushOptions::new();
    opts.remote_callbacks(callbacks);

    let refspec =
        if force { format!("+refs/heads/{branch}:refs/heads/{branch}") } else { format!("refs/heads/{branch}:refs/heads/{branch}") };
    log_network(repo, &remote, &format!("push {remote_name} {refspec}"), AuthMode::Interactive);
    // Local/file transports (used by our own tests, and by anyone pushing to
    // a bare repo on disk) reject non-fast-forwards synchronously here; smart
    // HTTP/SSH remotes instead report it asynchronously via the
    // `push_update_reference` callback below, hence checking both spots.
    if let Err(e) = remote.push(&[refspec.as_str()], Some(&mut opts)) {
        if e.code() == git2::ErrorCode::NotFastForward {
            return Err(GitCommandError::PushRejectedNonFastForward { message: e.message().to_string() });
        }
        return Err(e.into());
    }

    if let Some(message) = rejected.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take() {
        let is_non_fast_forward = ["non-fast-forward", "fetch first", "stale info"].iter().any(|needle| message.contains(needle));
        if is_non_fast_forward {
            return Err(GitCommandError::PushRejectedNonFastForward { message });
        }
        return Err(GitCommandError::Internal(format!("push rejeitado pelo servidor: {message}")));
    }

    set_upstream_if_missing(repo, remote_name, branch);
    Ok(PushSummary { remote_name: remote_name.to_string(), branch: branch.to_string() })
}

/// Like `git push -u` on a branch's first push, so later pushes (and
/// ahead/behind) know which remote it belongs to. Best-effort.
fn set_upstream_if_missing(repo: &git2::Repository, remote_name: &str, branch: &str) {
    let Ok(mut local) = repo.find_branch(branch, git2::BranchType::Local) else {
        return;
    };
    if local.upstream().is_ok() {
        return;
    }
    let _ = local.set_upstream(Some(&format!("{remote_name}/{branch}")));
}

/// Deletes `branch` on `remote_name` via an empty-source refspec (the git
/// protocol's delete form), then best-effort drops the local remote-tracking
/// ref (`refs/remotes/{remote}/{branch}`) too, so the UI stops showing this
/// branch as present on the remote without needing a fetch first.
pub fn delete_remote_branch(repo: &git2::Repository, remote_name: &str, branch: &str) -> Result<(), GitCommandError> {
    push_refspec(repo, remote_name, &format!(":refs/heads/{branch}"))?;
    if let Ok(mut tracking_ref) = repo.find_reference(&format!("refs/remotes/{remote_name}/{branch}")) {
        let _ = tracking_ref.delete();
    }
    Ok(())
}

/// Deletes `branch` on every remote in `remotes`, then locally. The remotes
/// go first: if one refuses (offline, protected), the local branch is still
/// there, instead of being left as the only thing that got deleted. The
/// checked-out branch is refused up front, before anything is touched.
pub fn delete_branch_everywhere(repo: &git2::Repository, remotes: &[String], branch: &str) -> Result<(), GitCommandError> {
    let local = repo.find_branch(branch, git2::BranchType::Local)?;
    if local.is_head() {
        return Err(GitCommandError::Internal(format!("não é possível excluir a branch em checkout \"{branch}\"")));
    }
    for remote in remotes {
        delete_remote_branch(repo, remote, branch)?;
    }
    crate::git::branch::delete_local_branch(repo, branch)
}

/// Pushes a single refspec, surfacing server-side rejections as errors.
fn push_refspec(repo: &git2::Repository, remote_name: &str, refspec: &str) -> Result<(), GitCommandError> {
    let mut remote = repo.find_remote(remote_name)?;
    log_network(repo, &remote, &format!("push {remote_name} {refspec}"), AuthMode::Interactive);
    let Auth { mut callbacks, .. } = Auth::new(repo, AuthMode::Interactive);

    let rejected: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let rejected_cb = rejected.clone();
    callbacks.push_update_reference(move |refname, status| {
        if let Some(message) = status {
            *rejected_cb.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(format!("{refname}: {message}"));
        }
        Ok(())
    });

    let mut opts = git2::PushOptions::new();
    opts.remote_callbacks(callbacks);
    remote.push(&[refspec], Some(&mut opts))?;

    if let Some(message) = rejected.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take() {
        return Err(GitCommandError::Internal(format!("push rejeitado pelo servidor: {message}")));
    }
    Ok(())
}

pub fn push_tag(repo: &git2::Repository, remote_name: &str, tag: &str) -> Result<(), GitCommandError> {
    push_refspec(repo, remote_name, &format!("refs/tags/{tag}:refs/tags/{tag}"))
}

pub fn delete_remote_tag(repo: &git2::Repository, remote_name: &str, tag: &str) -> Result<(), GitCommandError> {
    push_refspec(repo, remote_name, &format!(":refs/tags/{tag}"))
}

/// Names of the tags that exist on the remote (like `git ls-remote --tags`).
/// Looked up on its own while the UI renders, so it never prompts.
pub fn list_remote_tags(repo: &git2::Repository, remote_name: &str) -> Result<Vec<String>, GitCommandError> {
    let mut remote = repo.find_remote(remote_name)?;
    let what = format!("ls-remote --tags {remote_name}");
    log_network(repo, &remote, &what, AuthMode::Background);
    let Auth { callbacks, locked } = Auth::new(repo, AuthMode::Background);
    let conn = remote.connect_auth(git2::Direction::Fetch, Some(callbacks), None).map_err(|e| Auth::error(&locked, e))?;
    let tags =
        conn.list()?.iter().filter_map(|h| h.name().strip_prefix("refs/tags/")).filter(|n| !n.ends_with("^{}")).map(String::from).collect();
    Ok(tags)
}

/// Fetch options with this repo's credentials, for fetches that go through
/// another libgit2 API (submodule clone/update). Map their failure with
/// `auth_error` so a locked key still reads as `SshPassphraseRequired`
/// (or missing HTTPS credentials as `HttpsCredentialsRequired`).
pub fn fetch_options(repo: &git2::Repository, mode: AuthMode) -> (git2::FetchOptions<'static>, Arc<Mutex<Option<Locked>>>) {
    let Auth { callbacks, locked } = Auth::new(repo, mode);
    let mut opts = git2::FetchOptions::new();
    opts.remote_callbacks(callbacks);
    (opts, locked)
}

pub fn auth_error(locked: &Mutex<Option<Locked>>, e: git2::Error) -> GitCommandError {
    Auth::error(locked, e)
}

/// Activity line for a network round trip, in `git` CLI terms.
fn log_network(repo: &git2::Repository, remote: &git2::Remote, what: &str, mode: AuthMode) {
    let url = https_credentials::redact(remote.url().unwrap_or("?"));
    let background = if mode == AuthMode::Background { " (em segundo plano)" } else { "" };
    let dir = repo.workdir().unwrap_or(repo.path());
    activity::info(Some(dir), format!("$ git {what}  → {url}{background}"));
}

/// Whether a remote operation may stop to ask the user for an SSH key
/// passphrase or HTTPS credentials. Background ones (launch / on-open /
/// periodic fetches, remote tag lookups) only use what was already given this
/// session and otherwise fail with `SshPassphraseRequired` /
/// `HttpsCredentialsRequired`, waiting for an operation the user asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMode {
    Interactive,
    Background,
}

/// Steps through the candidate keys, one per credentials callback — libgit2
/// re-invokes it after the server rejects the previous credential.
struct KeyPicker {
    keys: Vec<(PathBuf, Option<PathBuf>)>,
    next: usize,
    mode: AuthMode,
    /// Encrypted key skipped because Background mode can't ask for its passphrase.
    locked: Option<PathBuf>,
}

impl KeyPicker {
    fn new(keys: Vec<(PathBuf, Option<PathBuf>)>, mode: AuthMode) -> Self {
        Self { keys, next: 0, mode, locked: None }
    }

    /// Next (private key, public key, passphrase) to offer, if any.
    fn next(
        &mut self,
        is_encrypted: impl Fn(&Path) -> bool,
        passphrase: impl Fn(&Path, AuthMode) -> Option<String>,
    ) -> Option<(PathBuf, Option<PathBuf>, Option<String>)> {
        while let Some((private_key, public_key)) = self.keys.get(self.next).cloned() {
            self.next += 1;
            if !is_encrypted(&private_key) {
                return Some((private_key, public_key, None));
            }
            match passphrase(&private_key, self.mode) {
                Some(p) => return Some((private_key, public_key, Some(p))),
                None if self.mode == AuthMode::Background => self.locked = Some(private_key),
                None => {} // the user cancelled the prompt
            }
        }
        None
    }
}

fn session_passphrase(key: &Path, mode: AuthMode) -> Option<String> {
    match mode {
        AuthMode::Interactive => passphrase::get(key),
        AuthMode::Background => passphrase::cached(key),
    }
}

/// Why a background operation gave up without asking the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Locked {
    /// Encrypted key whose passphrase wasn't given this session.
    SshKey(PathBuf),
    /// HTTPS remote with no credentials from the helper or this session.
    Https(String),
}

/// Where the HTTPS credential offered last came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CredSource {
    Helper,
    Session,
    Prompt,
}

/// A wrong password is asked again up to this many prompts in total.
const MAX_HTTPS_PROMPTS: u32 = 3;

/// Steps through HTTPS username/password sources, one per credentials
/// callback (libgit2 re-invokes it after the server rejects one): git's own
/// credential helper, then what was typed earlier this session, then —
/// interactive only — a prompt.
struct HttpsPicker {
    mode: AuthMode,
    helper_tried: bool,
    session_tried: bool,
    prompts: u32,
    last: Option<CredSource>,
    /// Background mode ran out of sources and may not ask.
    locked: bool,
}

type Creds = https_credentials::Credentials;

impl HttpsPicker {
    fn new(mode: AuthMode) -> Self {
        Self { mode, helper_tried: false, session_tried: false, prompts: 0, last: None, locked: false }
    }

    fn next(
        &mut self,
        helper: impl FnOnce() -> Option<Creds>,
        session: impl FnOnce() -> Option<Creds>,
        prompt: impl FnOnce(/* retry */ bool) -> Option<Creds>,
        forget: impl FnOnce(),
    ) -> Option<(Creds, CredSource)> {
        // Re-invoked: whatever we offered last was rejected.
        let rejected = self.last.take();
        if matches!(rejected, Some(CredSource::Session | CredSource::Prompt)) {
            forget();
        }
        let mut found = None;
        if !self.helper_tried {
            self.helper_tried = true;
            found = helper().map(|c| (c, CredSource::Helper));
        }
        if found.is_none() && !self.session_tried {
            self.session_tried = true;
            found = session().map(|c| (c, CredSource::Session));
        }
        if found.is_none() {
            match self.mode {
                AuthMode::Background => self.locked = true,
                AuthMode::Interactive if self.prompts < MAX_HTTPS_PROMPTS => {
                    self.prompts += 1;
                    found = prompt(rejected.is_some()).map(|c| (c, CredSource::Prompt));
                }
                AuthMode::Interactive => {}
            }
        }
        self.last = found.as_ref().map(|(_, source)| *source);
        found
    }
}

/// Username + password from git's configured credential helper(s), by
/// `git credential fill` itself: it runs every kind of helper (libgit2's
/// own runner needs `sh`, missing on Windows). It may never prompt on a
/// terminal or an askpass window (the app asks itself), and in background
/// mode tells Git Credential Manager not to pop up its sign-in UI either.
fn helper_credentials(repo_dir: &Path, url: &str, username: Option<&str>, mode: AuthMode) -> Option<Creds> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_dir)
        .args(["credential", "fill"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "false")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if mode == AuthMode::Background {
        cmd.env("GCM_INTERACTIVE", "never");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().ok()?;
    let mut input = format!("url={url}\n");
    if let Some(user) = username {
        input.push_str(&format!("username={user}\n"));
    }
    input.push('\n');
    // Small enough for the pipe buffer: written before reading the answer.
    child.stdin.take()?.write_all(input.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_credential_answer(&String::from_utf8_lossy(&out.stdout))
}

/// `key=value` lines of a `git credential fill` answer.
fn parse_credential_answer(out: &str) -> Option<Creds> {
    let value = |key: &str| out.lines().find_map(|l| l.strip_prefix(key).and_then(|rest| rest.strip_prefix('='))).map(String::from);
    Some(Creds { username: value("username")?, password: value("password")? })
}

/// Credential callbacks plus what they learned, to explain a failure.
struct Auth {
    callbacks: git2::RemoteCallbacks<'static>,
    locked: Arc<Mutex<Option<Locked>>>,
}

impl Auth {
    /// SSH: ssh-agent, then the repo's SSH key (see `ssh_keys`). HTTPS: git's
    /// credential helper, then credentials typed this session, then a prompt.
    fn new(repo: &git2::Repository, mode: AuthMode) -> Self {
        let repo_dir = repo.workdir().unwrap_or(repo.path()).to_path_buf();
        let mut picker = KeyPicker::new(ssh_keys::keys_for(Some(&repo_dir)), mode);
        let mut https = HttpsPicker::new(mode);
        let locked = Arc::new(Mutex::new(None));
        let locked_cb = locked.clone();
        let mut agent_tried = false;
        let mut default_tried = false;
        let mut last_encrypted: Option<PathBuf> = None;

        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(move |url, username_from_url, allowed| {
            let username = username_from_url.unwrap_or("git");
            let log = |msg: String| activity::info(Some(&repo_dir), msg);

            // SSH URLs without a user ask for one before the key.
            if allowed.contains(git2::CredentialType::USERNAME) {
                return git2::Cred::username(username);
            }

            if allowed.contains(git2::CredentialType::SSH_KEY) {
                // Re-invoked means the previous credential failed the handshake;
                // if it used a passphrase, that one may be wrong — ask again next time.
                if let Some(key) = last_encrypted.take() {
                    passphrase::forget(&key);
                }
                if !agent_tried {
                    agent_tried = true;
                    if let Ok(cred) = git2::Cred::ssh_key_from_agent(username) {
                        log(format!("ssh: oferecendo as identidades do ssh-agent como {username}"));
                        return Ok(cred);
                    }
                }
                while let Some((private_key, public_key, pass)) = picker.next(passphrase::is_encrypted, session_passphrase) {
                    if let Ok(cred) = git2::Cred::ssh_key(username, public_key.as_deref(), &private_key, pass.as_deref()) {
                        let unlocked = if pass.is_some() { " (desbloqueada)" } else { "" };
                        log(format!("ssh: oferecendo a chave {}{unlocked}", private_key.display()));
                        if pass.is_some() {
                            last_encrypted = Some(private_key);
                        }
                        return Ok(cred);
                    }
                }
                if let Some(key) = &picker.locked {
                    log(format!("ssh: a chave {} precisa da senha; operações em segundo plano não pedem", key.display()));
                    *locked_cb.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Locked::SshKey(key.clone()));
                }
            }

            if allowed.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
                let host = https_credentials::host_key(url);
                let found = https.next(
                    || {
                        log(format!("$ git credential fill  → {host}"));
                        helper_credentials(&repo_dir, url, username_from_url, mode)
                    },
                    || https_credentials::cached(url),
                    |retry| {
                        log(format!("https: pedindo credenciais para {host}"));
                        https_credentials::get(url, username_from_url, retry)
                    },
                    || {
                        log(format!("https: as credenciais para {host} foram rejeitadas"));
                        https_credentials::forget(url);
                    },
                );
                if let Some((creds, source)) = found {
                    let from = match source {
                        CredSource::Helper => "credential helper do git",
                        CredSource::Session => "esta sessão",
                        CredSource::Prompt => "digitadas pelo usuário",
                    };
                    // The username only: never the password/token.
                    log(format!("https: oferecendo o usuário {} para {host} (origem: {from})", creds.username));
                    return git2::Cred::userpass_plaintext(&creds.username, &creds.password);
                }
                if https.locked {
                    log(format!("https: {host} precisa de credenciais; operações em segundo plano não as pedem"));
                    // A locked SSH key (same connection offering both) explains it better.
                    locked_cb
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .get_or_insert(Locked::Https(https_credentials::redact(url)));
                }
            }

            // Once only: it's a marker (NTLM/Negotiate), so a rejection would loop.
            if allowed.contains(git2::CredentialType::DEFAULT) && !default_tried {
                default_tried = true;
                if let Ok(cred) = git2::Cred::default() {
                    return Ok(cred);
                }
            }

            let url = https_credentials::redact(url);
            activity::error(Some(&repo_dir), format!("nenhuma credencial disponível para {url}"));
            Err(git2::Error::from_str(&format!("nenhuma credencial disponível para {url}")))
        });
        Self { callbacks, locked }
    }

    fn error(locked: &Mutex<Option<Locked>>, e: git2::Error) -> GitCommandError {
        match locked.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take() {
            Some(Locked::SshKey(key)) => GitCommandError::SshPassphraseRequired { key: key.display().to_string() },
            Some(Locked::Https(url)) => GitCommandError::HttpsCredentialsRequired { url },
            None => e.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn commit_file(repo: &git2::Repository, file: &str, content: &str, msg: &str) -> git2::Oid {
        std::fs::write(repo.workdir().unwrap().join(file), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(file)).unwrap();
        let tree_id = index.write_tree().unwrap();
        index.write().unwrap();

        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().ok().and_then(|h| h.target()).and_then(|oid| repo.find_commit(oid).ok());
        let parents: Vec<_> = parent.into_iter().collect();
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parent_refs).unwrap()
    }

    #[test]
    fn pushes_to_and_fetches_from_a_local_bare_remote() {
        let bare_dir = tempfile::tempdir().unwrap();
        git2::Repository::init_bare(bare_dir.path()).unwrap();

        let local_dir = tempfile::tempdir().unwrap();
        let local = git2::Repository::init(local_dir.path()).unwrap();
        commit_file(&local, "a.txt", "hello", "initial");
        let branch_name = local.head().unwrap().shorthand().unwrap().to_string();
        local.remote("origin", bare_dir.path().to_str().unwrap()).unwrap();

        // push: local -> bare "remote"
        let summary = push(&local, "origin", &branch_name, false, |_, _, _| {}).unwrap();
        assert_eq!(summary.branch, branch_name);

        // a second clone fetching from the same bare remote should see the pushed commit
        let other_dir = tempfile::tempdir().unwrap();
        let other = git2::Repository::init(other_dir.path()).unwrap();
        other.remote("origin", bare_dir.path().to_str().unwrap()).unwrap();
        let fetch_summary = fetch(&other, "origin", AuthMode::Interactive, |_, _, _| {}).unwrap();
        assert!(fetch_summary.received_objects > 0);
        assert!(other.find_reference(&format!("refs/remotes/origin/{branch_name}")).is_ok());
    }

    #[test]
    fn non_fast_forward_push_is_rejected_unless_forced() {
        let bare_dir = tempfile::tempdir().unwrap();
        git2::Repository::init_bare(bare_dir.path()).unwrap();

        let local_dir = tempfile::tempdir().unwrap();
        let local = git2::Repository::init(local_dir.path()).unwrap();
        commit_file(&local, "a.txt", "hello", "initial");
        let branch_name = local.head().unwrap().shorthand().unwrap().to_string();
        local.remote("origin", bare_dir.path().to_str().unwrap()).unwrap();
        push(&local, "origin", &branch_name, false, |_, _, _| {}).unwrap();

        // a second clone fetches, then pushes a commit of its own on top —
        // the bare remote's ref is now ahead of what `local` knows about.
        let other_dir = tempfile::tempdir().unwrap();
        let other = git2::Repository::init(other_dir.path()).unwrap();
        other.remote("origin", bare_dir.path().to_str().unwrap()).unwrap();
        fetch(&other, "origin", AuthMode::Interactive, |_, _, _| {}).unwrap();
        let remote_commit = other.find_reference(&format!("refs/remotes/origin/{branch_name}")).unwrap().peel_to_commit().unwrap();
        other.branch(&branch_name, &remote_commit, false).unwrap();
        other.set_head(&format!("refs/heads/{branch_name}")).unwrap();
        other.checkout_head(None).unwrap();
        commit_file(&other, "b.txt", "from-other", "other change");
        push(&other, "origin", &branch_name, false, |_, _, _| {}).unwrap();

        // `local` makes a diverging commit and tries to push without force
        commit_file(&local, "c.txt", "from-local", "diverging change");
        let result = push(&local, "origin", &branch_name, false, |_, _, _| {});
        assert!(
            matches!(result, Err(GitCommandError::PushRejectedNonFastForward { .. })),
            "expected PushRejectedNonFastForward, got {result:?}"
        );

        // forcing it through succeeds and overwrites the remote's ref
        push(&local, "origin", &branch_name, true, |_, _, _| {}).unwrap();
    }

    fn keys(names: &[&str]) -> Vec<(PathBuf, Option<PathBuf>)> {
        names.iter().map(|n| (PathBuf::from(n), None)).collect()
    }

    #[test]
    fn background_never_prompts_and_reports_the_locked_key() {
        let mut picker = KeyPicker::new(keys(&["/k/locked"]), AuthMode::Background);
        let asked = std::cell::Cell::new(None);
        let next = picker.next(
            |_| true,
            |_, mode| {
                asked.set(Some(mode));
                None
            },
        );
        assert_eq!(next, None);
        assert_eq!(asked.get(), Some(AuthMode::Background));
        assert_eq!(picker.locked, Some(PathBuf::from("/k/locked")));
    }

    #[test]
    fn background_uses_a_passphrase_given_earlier() {
        let mut picker = KeyPicker::new(keys(&["/k/locked"]), AuthMode::Background);
        let next = picker.next(|_| true, |_, _| Some("secret".into()));
        assert_eq!(next, Some(("/k/locked".into(), None, Some("secret".into()))));
        assert_eq!(picker.locked, None);
    }

    #[test]
    fn interactive_asks_and_a_cancel_is_not_reported_as_locked() {
        let mut picker = KeyPicker::new(keys(&["/k/locked", "/k/plain"]), AuthMode::Interactive);
        let encrypted = |p: &Path| p == Path::new("/k/locked");
        let next = picker.next(encrypted, |_, mode| {
            assert_eq!(mode, AuthMode::Interactive);
            None
        });
        assert_eq!(next, Some(("/k/plain".into(), None, None)));
        assert_eq!(picker.locked, None);
    }

    #[test]
    fn each_key_is_offered_once() {
        let mut picker = KeyPicker::new(keys(&["/k/a", "/k/b"]), AuthMode::Interactive);
        let mut offered = Vec::new();
        while let Some((key, _, _)) = picker.next(|_| false, |_, _| None) {
            offered.push(key);
        }
        assert_eq!(offered, [PathBuf::from("/k/a"), PathBuf::from("/k/b")]);
    }

    #[test]
    fn background_fetch_works_without_credentials() {
        let bare_dir = tempfile::tempdir().unwrap();
        git2::Repository::init_bare(bare_dir.path()).unwrap();
        let local_dir = tempfile::tempdir().unwrap();
        let local = git2::Repository::init(local_dir.path()).unwrap();
        local.remote("origin", bare_dir.path().to_str().unwrap()).unwrap();
        commit_file(&local, "a.txt", "a", "first");
        let branch = local.head().unwrap().shorthand().unwrap().to_string();
        push(&local, "origin", &branch, false, |_, _, _| {}).unwrap();

        let other_dir = tempfile::tempdir().unwrap();
        let other = git2::Repository::init(other_dir.path()).unwrap();
        other.remote("origin", bare_dir.path().to_str().unwrap()).unwrap();
        fetch(&other, "origin", AuthMode::Background, |_, _, _| {}).unwrap();
        assert!(other.find_reference(&format!("refs/remotes/origin/{branch}")).is_ok());
    }

    #[test]
    fn locked_maps_to_the_matching_error() {
        let locked = Mutex::new(Some(Locked::SshKey(PathBuf::from("/k/locked"))));
        let err = Auth::error(&locked, git2::Error::from_str("no credentials"));
        assert!(matches!(err, GitCommandError::SshPassphraseRequired { key } if key == "/k/locked"));
        let locked = Mutex::new(Some(Locked::Https("https://h/r.git".into())));
        let err = Auth::error(&locked, git2::Error::from_str("no credentials"));
        assert!(matches!(err, GitCommandError::HttpsCredentialsRequired { url } if url == "https://h/r.git"));
        let unlocked = Mutex::new(None);
        let err = Auth::error(&unlocked, git2::Error::from_str("no credentials"));
        assert!(matches!(err, GitCommandError::Libgit2 { .. }));
    }

    fn creds(user: &str) -> Creds {
        Creds { username: user.into(), password: "pw".into() }
    }

    #[test]
    fn https_tries_helper_then_session_then_prompt() {
        let mut picker = HttpsPicker::new(AuthMode::Interactive);
        let forgot = std::cell::Cell::new(0);
        let step = |p: &mut HttpsPicker| {
            p.next(|| Some(creds("helper")), || Some(creds("session")), |_| Some(creds("typed")), || forgot.set(forgot.get() + 1))
        };
        assert_eq!(step(&mut picker).unwrap().1, CredSource::Helper);
        assert_eq!(step(&mut picker).unwrap().1, CredSource::Session);
        // A rejected helper answer isn't ours to forget; a rejected session one is.
        assert_eq!(forgot.get(), 0);
        assert_eq!(step(&mut picker).unwrap().1, CredSource::Prompt);
        assert_eq!(forgot.get(), 1);
    }

    #[test]
    fn https_reprompts_a_bounded_number_of_times_with_retry_set() {
        let mut picker = HttpsPicker::new(AuthMode::Interactive);
        let mut retries = Vec::new();
        while picker
            .next(
                || None,
                || None,
                |retry| {
                    retries.push(retry);
                    Some(creds("typed"))
                },
                || {},
            )
            .is_some()
        {}
        assert_eq!(retries, [false, true, true]);
    }

    #[test]
    fn https_cancelled_prompt_gives_up_without_locking() {
        let mut picker = HttpsPicker::new(AuthMode::Interactive);
        assert!(picker.next(|| None, || None, |_| None, || {}).is_none());
        assert!(!picker.locked);
    }

    #[test]
    fn https_background_never_prompts_and_reports_locked() {
        let mut picker = HttpsPicker::new(AuthMode::Background);
        let found = picker.next(|| None, || None, |_| panic!("background must not prompt"), || {});
        assert!(found.is_none());
        assert!(picker.locked);

        // ...but uses what was typed earlier this session.
        let mut picker = HttpsPicker::new(AuthMode::Background);
        let found = picker.next(|| None, || Some(creds("session")), |_| panic!("no prompt"), || {});
        assert_eq!(found.unwrap().1, CredSource::Session);
    }

    /// Like `git credential fill`: a configured helper's answer is used.
    #[test]
    fn reads_credentials_from_the_configured_helper() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let mut config = repo.config().unwrap().open_level(git2::ConfigLevel::Local).unwrap();
        config.set_str("credential.helper", "!f() { echo username=helper-user; echo password=helper-pw; }; f").unwrap();
        let url = "https://example.invalid/r.git";
        let found = helper_credentials(dir.path(), url, None, AuthMode::Interactive);
        assert_eq!(found, Some(Creds { username: "helper-user".into(), password: "helper-pw".into() }));

        // Background lookups tell Git Credential Manager not to show its UI.
        config.set_str("credential.helper", "!f() { echo username=u; echo password=gcm-$GCM_INTERACTIVE; }; f").unwrap();
        let password = |mode| helper_credentials(dir.path(), url, Some("u"), mode).unwrap().password;
        assert_eq!(password(AuthMode::Background), "gcm-never");
        // Interactive keeps whatever the environment already says (the Windows runner sets `Never`).
        let inherited = std::env::var("GCM_INTERACTIVE").unwrap_or_default();
        assert_eq!(password(AuthMode::Interactive), format!("gcm-{inherited}"));

        // No answer: git would prompt, but may not.
        config.set_str("credential.helper", "!true").unwrap();
        assert_eq!(helper_credentials(dir.path(), url, None, AuthMode::Interactive), None);
    }

    #[test]
    fn parses_a_credential_answer() {
        let out = "protocol=https\nhost=h\nusername=me\npassword=p=w\n";
        assert_eq!(parse_credential_answer(out), Some(Creds { username: "me".into(), password: "p=w".into() }));
        assert_eq!(parse_credential_answer("username=me\n"), None);
    }

    fn init_with_remotes(names: &[&str]) -> (tempfile::TempDir, git2::Repository) {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        for name in names {
            repo.remote(name, &format!("https://example.invalid/{name}.git")).unwrap();
        }
        (dir, repo)
    }

    #[test]
    fn lists_remotes_origin_first() {
        let (_dir, repo) = init_with_remotes(&["upstream", "origin", "fork"]);
        repo.remote_set_pushurl("fork", Some("git@example.invalid:fork.git")).unwrap();
        let remotes = list_remotes(&repo).unwrap();
        let names: Vec<_> = remotes.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["origin", "fork", "upstream"]);
        assert_eq!(remotes[0].fetch_url.as_deref(), Some("https://example.invalid/origin.git"));
        assert_eq!(remotes[0].push_url, None);
        assert_eq!(remotes[1].push_url.as_deref(), Some("git@example.invalid:fork.git"));
    }

    #[test]
    fn adds_renames_edits_and_removes_remotes() {
        let (_dir, repo) = init_with_remotes(&[]);
        add_remote(&repo, "origin", " https://example.invalid/a.git ").unwrap();
        assert!(add_remote(&repo, "bad name", "x").is_err());
        assert!(add_remote(&repo, "origin", "https://dup").is_err());
        rename_remote(&repo, "origin", "upstream").unwrap();
        set_remote_url(&repo, "upstream", "https://example.invalid/b.git").unwrap();
        let remotes = list_remotes(&repo).unwrap();
        assert_eq!(
            remotes,
            [RemoteInfo { name: "upstream".into(), fetch_url: Some("https://example.invalid/b.git".into()), push_url: None }]
        );
        remove_remote(&repo, "upstream").unwrap();
        assert!(list_remotes(&repo).unwrap().is_empty());
    }

    #[test]
    fn rename_carries_remote_tracking_branches_along() {
        let bare_dir = tempfile::tempdir().unwrap();
        git2::Repository::init_bare(bare_dir.path()).unwrap();
        let (_dir, repo) = init_with_remotes(&[]);
        // forward slashes: a `\` in a remote URL is an escape in the config file
        repo.remote("origin", &bare_dir.path().to_str().unwrap().replace('\\', "/")).unwrap();
        commit_file(&repo, "a.txt", "a", "first");
        let branch = repo.head().unwrap().shorthand().unwrap().to_string();
        push(&repo, "origin", &branch, false, |_, _, _| {}).unwrap();
        rename_remote(&repo, "origin", "hub").unwrap();
        assert!(repo.find_reference(&format!("refs/remotes/hub/{branch}")).is_ok());
        assert!(repo.find_reference(&format!("refs/remotes/origin/{branch}")).is_err());
    }

    #[test]
    fn push_remote_prefers_configured_then_origin_then_the_only_one() {
        let s = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let remotes = s(&["origin", "fork"]);
        assert_eq!(pick_push_remote(&[None, None, Some("fork".into())], &remotes), Some("fork".into()));
        assert_eq!(pick_push_remote(&[Some("fork".into()), None, Some("origin".into())], &remotes), Some("fork".into()));
        // A configured remote that no longer exists is skipped.
        assert_eq!(pick_push_remote(&[Some("gone".into())], &remotes), Some("origin".into()));
        assert_eq!(pick_push_remote(&[None], &s(&["fork"])), Some("fork".into()));
        assert_eq!(pick_push_remote(&[None], &s(&["fork", "upstream"])), None);
        assert_eq!(pick_push_remote(&[None], &[]), None);
    }

    #[test]
    fn first_push_sets_the_upstream_so_the_next_one_needs_no_choice() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        git2::Repository::init_bare(a.path()).unwrap();
        git2::Repository::init_bare(b.path()).unwrap();
        let (_dir, repo) = init_with_remotes(&[]);
        repo.remote("fork", a.path().to_str().unwrap()).unwrap();
        repo.remote("upstream", b.path().to_str().unwrap()).unwrap();
        commit_file(&repo, "a.txt", "a", "first");
        let branch = repo.head().unwrap().shorthand().unwrap().to_string();

        let err = push_remote_for(&repo, &branch).unwrap_err();
        assert!(matches!(err, GitCommandError::RemoteRequired { remotes } if remotes == ["fork", "upstream"]));
        push(&repo, "upstream", &branch, false, |_, _, _| {}).unwrap();
        assert_eq!(push_remote_for(&repo, &branch).unwrap(), "upstream");
    }

    #[test]
    fn fetches_every_remote_and_reports_the_first_failure() {
        let bare_a = tempfile::tempdir().unwrap();
        let bare_b = tempfile::tempdir().unwrap();
        let mut branch = String::new();
        for (dir, file) in [(&bare_a, "a.txt"), (&bare_b, "b.txt")] {
            git2::Repository::init_bare(dir.path()).unwrap();
            let src_dir = tempfile::tempdir().unwrap();
            let src = git2::Repository::init(src_dir.path()).unwrap();
            src.remote("origin", dir.path().to_str().unwrap()).unwrap();
            commit_file(&src, file, file, file);
            branch = src.head().unwrap().shorthand().unwrap().to_string();
            push(&src, "origin", &branch, false, |_, _, _| {}).unwrap();
        }
        let (_dir, repo) = init_with_remotes(&[]);
        repo.remote("origin", bare_a.path().to_str().unwrap()).unwrap();
        repo.remote("other", bare_b.path().to_str().unwrap()).unwrap();

        let summary = fetch_remotes(&repo, None, AuthMode::Background, |_, _, _| {}).unwrap();
        assert_eq!(summary.remote_name, "origin, other");
        assert!(summary.received_objects > 0);
        let tracking: Vec<String> = repo
            .branches(Some(git2::BranchType::Remote))
            .unwrap()
            .map(|b| b.unwrap().0.name().unwrap().unwrap().split('/').next().unwrap().to_string())
            .collect();
        assert!(tracking.contains(&"origin".to_string()) && tracking.contains(&"other".to_string()));

        // A broken remote fails the call, but the others still got fetched.
        repo.remote("broken", "/definitely/not/a/repo").unwrap();
        let other_ref = format!("refs/remotes/other/{branch}");
        repo.find_reference(&other_ref).unwrap().delete().unwrap();
        assert!(fetch_remotes(&repo, None, AuthMode::Background, |_, _, _| {}).is_err());
        assert!(repo.find_reference(&other_ref).is_ok());
        // Like `git fetch --all`, skipFetchAll remotes are left out (named, they're fetched).
        repo.config().unwrap().set_bool("remote.broken.skipFetchAll", true).unwrap();
        assert_eq!(fetch_remotes(&repo, None, AuthMode::Background, |_, _, _| {}).unwrap().remote_name, "origin, other");
        assert!(fetch_remotes(&repo, Some("broken"), AuthMode::Background, |_, _, _| {}).is_err());

        // No remotes: nothing to do, not an error.
        let (_empty_dir, empty) = init_with_remotes(&[]);
        assert_eq!(fetch_remotes(&empty, None, AuthMode::Background, |_, _, _| {}).unwrap().remote_name, "");
    }

    #[test]
    fn counts_updated_refs_so_a_no_op_fetch_reports_zero() {
        let bare = tempfile::tempdir().unwrap();
        git2::Repository::init_bare(bare.path()).unwrap();
        let (_src_dir, src) = init_with_remotes(&[]);
        src.remote("origin", bare.path().to_str().unwrap()).unwrap();
        commit_file(&src, "a.txt", "a", "first");
        let branch = src.head().unwrap().shorthand().unwrap().to_string();
        push(&src, "origin", &branch, false, |_, _, _| {}).unwrap();

        let (_dir, repo) = init_with_remotes(&[]);
        repo.remote("origin", bare.path().to_str().unwrap()).unwrap();
        let first = fetch_remotes(&repo, None, AuthMode::Background, |_, _, _| {}).unwrap();
        assert_eq!(first.updated_refs, 1);
        let again = fetch_remotes(&repo, None, AuthMode::Background, |_, _, _| {}).unwrap();
        assert_eq!(again.updated_refs, 0);
        commit_file(&src, "b.txt", "b", "second");
        push(&src, "origin", &branch, false, |_, _, _| {}).unwrap();
        assert_eq!(fetch_remotes(&repo, None, AuthMode::Background, |_, _, _| {}).unwrap().updated_refs, 1);
    }

    fn base64(input: &str) -> String {
        const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in input.as_bytes().chunks(3) {
            let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
            let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    /// HTTP server that rejects every request with `401 Basic`, recording the
    /// `Authorization` header (if any) of each — drives libgit2's real HTTP
    /// transport through the credentials callback.
    fn rejecting_http_server() -> (u16, Arc<Mutex<Vec<Option<String>>>>) {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_srv = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut auth = None;
                let mut line = String::new();
                while reader.read_line(&mut line).map(|n| n > 0).unwrap_or(false) && line != "\r\n" {
                    if let Some(v) = line.strip_prefix("Authorization: ") {
                        auth = Some(v.trim().to_string());
                    }
                    line.clear();
                }
                seen_srv.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(auth);
                let _ = stream.write_all(
                    b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"t\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
        });
        (port, seen)
    }

    #[test]
    fn https_background_fetch_tries_helper_then_session_then_reports_required() {
        let (port, seen) = rejecting_http_server();
        let url = format!("http://127.0.0.1:{port}/r.git");
        let (_dir, repo) = init_with_remotes(&[]);
        repo.remote("origin", &url).unwrap();
        let mut local = repo.config().unwrap().open_level(git2::ConfigLevel::Local).unwrap();
        local.set_str("credential.helper", "!f() { echo username=helper-user; echo password=helper-pw; }; f").unwrap();
        let session = Creds { username: "typed-user".into(), password: "typed-pw".into() };
        https_credentials::remember(&url, session);

        let err = fetch(&repo, "origin", AuthMode::Background, |_, _, _| {}).unwrap_err();
        assert!(matches!(&err, GitCommandError::HttpsCredentialsRequired { url: u } if *u == url), "{err:?}");
        let sent: Vec<_> = seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner).iter().flatten().cloned().collect();
        assert_eq!(sent, [format!("Basic {}", base64("helper-user:helper-pw")), format!("Basic {}", base64("typed-user:typed-pw")),]);
        // Rejected session credentials are forgotten, so the next user-started op asks.
        assert_eq!(https_credentials::cached(&url), None);
    }

    #[test]
    fn https_required_error_never_carries_a_token_from_the_url() {
        let (port, _seen) = rejecting_http_server();
        let (_dir, repo) = init_with_remotes(&[]);
        repo.remote("origin", &format!("http://bot:s3cret-token@127.0.0.1:{port}/r.git")).unwrap();
        // No helper answer (and none inherited from the machine's global config).
        let mut local = repo.config().unwrap().open_level(git2::ConfigLevel::Local).unwrap();
        local.set_str("credential.helper", "!true").unwrap();

        let err = fetch(&repo, "origin", AuthMode::Background, |_, _, _| {}).unwrap_err();
        assert!(matches!(&err, GitCommandError::HttpsCredentialsRequired { .. }), "{err:?}");
        assert!(!format!("{err} {err:?}").contains("s3cret-token"));
    }

    #[test]
    fn https_interactive_with_the_prompt_cancelled_stops_without_looping() {
        let (port, seen) = rejecting_http_server();
        let (_dir, repo) = init_with_remotes(&[]);
        repo.remote("origin", &format!("http://127.0.0.1:{port}/r.git")).unwrap();
        let mut local = repo.config().unwrap().open_level(git2::ConfigLevel::Local).unwrap();
        local.set_str("credential.helper", "!true").unwrap();

        // No UI in tests: the prompt answers like a cancel.
        let err = fetch(&repo, "origin", AuthMode::Interactive, |_, _, _| {}).unwrap_err();
        assert!(matches!(err, GitCommandError::Libgit2 { .. }), "{err:?}");
        assert!(seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner).iter().all(Option::is_none));
    }

    #[test]
    fn delete_everywhere_removes_remote_and_local_or_neither() {
        let bare_dir = tempfile::tempdir().unwrap();
        git2::Repository::init_bare(bare_dir.path()).unwrap();
        let local_dir = tempfile::tempdir().unwrap();
        let local = git2::Repository::init(local_dir.path()).unwrap();
        commit_file(&local, "a.txt", "hello", "initial");
        let main = local.head().unwrap().shorthand().unwrap().to_string();
        local.remote("origin", bare_dir.path().to_str().unwrap()).unwrap();
        let head = local.head().unwrap().peel_to_commit().unwrap();
        local.branch("feat", &head, false).unwrap();
        push(&local, "origin", "feat", false, |_, _, _| {}).unwrap();

        // The checked-out branch is refused before any remote is touched.
        assert!(delete_branch_everywhere(&local, &["origin".into()], &main).is_err());

        // A remote that can't be reached leaves the local branch alone.
        local.remote("dead", "/nonexistent/path/to/repo").unwrap();
        assert!(delete_branch_everywhere(&local, &["origin".into(), "dead".into()], "feat").is_err());
        assert!(local.find_branch("feat", git2::BranchType::Local).is_ok());

        delete_branch_everywhere(&local, &["origin".into()], "feat").unwrap();
        assert!(local.find_branch("feat", git2::BranchType::Local).is_err());
        let bare = git2::Repository::open_bare(bare_dir.path()).unwrap();
        assert!(bare.find_reference("refs/heads/feat").is_err());
    }
}

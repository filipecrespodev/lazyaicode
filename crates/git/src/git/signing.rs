//! Commit signing, which libgit2 leaves to the caller: the commit buffer is
//! signed by `ssh-keygen -Y sign` (gpg.format=ssh) or `gpg -bsa` (openpgp),
//! exactly what git runs, and stored with `commit_signed`.
//!
//! Signing happens when the repo asks for it (`commit.gpgsign=true`, using
//! `gpg.format` / `user.signingkey` like git) or when the app setting "Sign
//! commits" is on, which signs with the repo's SSH key from [`ssh_keys`]
//! without touching git config.
//!
//! Passphrase-protected SSH keys: ssh-keygen tries ssh-agent first on its
//! own. If the agent lacks the key, we ask through the same prompt as
//! fetch/push ([`passphrase`]) and hand the answer to ssh-keygen via
//! `SSH_ASKPASS` pointing at this very executable, which in that mode just
//! prints the passphrase from its environment (see [`answer_askpass`]). No
//! temp files with secrets, nothing on the command line.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::error::GitCommandError;
use crate::{activity, passphrase, ssh_keys};

/// App setting (bool): sign every commit with the repo's SSH key.
const STORE_KEY: &str = "signCommits";
/// Set on ssh-keygen when it should get a passphrase from us.
const ASKPASS_ENV: &str = "GITHYDRA_ASKPASS_REPLY";

/// When this binary was started as ssh-keygen's `SSH_ASKPASS` helper,
/// prints the passphrase it was given and returns true (the caller exits).
pub fn answer_askpass() -> bool {
    match std::env::var(ASKPASS_ENV) {
        Ok(reply) => {
            println!("{reply}");
            true
        }
        Err(_) => false,
    }
}

fn app_forces_signing() -> bool {
    crate::settings::get(STORE_KEY).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn signing_error(message: impl Into<String>) -> GitCommandError {
    GitCommandError::SigningFailed { message: message.into() }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SshKey {
    /// Private (or public, with the private half in the agent) key file.
    File(PathBuf),
    /// `key::ssh-ed25519 AAAA…` / `ssh-ed25519 AAAA…`: a public key whose
    /// private half lives in ssh-agent.
    Literal(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Method {
    Ssh(SshKey),
    /// `None`: gpg picks the key from the committer identity.
    OpenPgp(Option<String>),
}

#[derive(Debug, Clone)]
pub struct Signer {
    pub method: Method,
    pub program: String,
    /// Program ssh-keygen runs to read a passphrase (this executable).
    pub askpass: PathBuf,
    /// Repo the Activity Logs entries belong to.
    pub log_dir: Option<PathBuf>,
}

/// `user.signingkey` for SSH: a literal public key or a key file (`~/` ok).
fn parse_ssh_key(value: &str) -> SshKey {
    let value = value.trim();
    if let Some(literal) = value.strip_prefix("key::") {
        return SshKey::Literal(literal.trim().to_string());
    }
    if ["ssh-", "ecdsa-", "sk-"].iter().any(|p| value.starts_with(p)) {
        return SshKey::Literal(value.to_string());
    }
    SshKey::File(expand_home(value))
}

fn expand_home(path: &str) -> PathBuf {
    match path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        Some(rest) => std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(|home| PathBuf::from(home).join(rest))
            .unwrap_or_else(|| PathBuf::from(path)),
        None => PathBuf::from(path),
    }
}

/// The repo's own SSH key (repo > workspace > default), for signing when
/// git config names none.
fn repo_ssh_key(repo: &git2::Repository) -> Result<SshKey, GitCommandError> {
    ssh_keys::keys_for(repo.workdir())
        .into_iter()
        .next()
        .map(|(private, _)| SshKey::File(private))
        .ok_or_else(|| signing_error("nenhuma chave SSH para assinar: escolha uma em Configurações › Chaves SSH ou defina user.signingkey"))
}

/// How commits in `repo` must be signed, or `None` when they aren't.
pub fn signer_for(repo: &git2::Repository) -> Result<Option<Signer>, GitCommandError> {
    let config = repo.config()?.snapshot()?;
    let gpgsign = config.get_bool("commit.gpgsign").unwrap_or(false);
    if !gpgsign && !app_forces_signing() {
        return Ok(None);
    }
    let get = |key: &str| config.get_string(key).ok().filter(|v| !v.trim().is_empty());
    let signing_key = get("user.signingkey");
    let format = if gpgsign { get("gpg.format").unwrap_or_else(|| "openpgp".into()) } else { "ssh".into() };
    let (method, program) = match format.as_str() {
        "ssh" => {
            let key = match (gpgsign, signing_key) {
                (true, Some(key)) => parse_ssh_key(&key),
                _ => repo_ssh_key(repo)?,
            };
            (Method::Ssh(key), get("gpg.ssh.program").unwrap_or_else(|| "ssh-keygen".into()))
        }
        "openpgp" => {
            let program = get("gpg.openpgp.program").or_else(|| get("gpg.program")).unwrap_or_else(|| "gpg".into());
            (Method::OpenPgp(signing_key), program)
        }
        other => return Err(signing_error(format!("gpg.format={other} não é suportado"))),
    };
    Ok(Some(Signer {
        method,
        program,
        askpass: std::env::current_exe().unwrap_or_default(),
        log_dir: repo.workdir().map(Path::to_path_buf),
    }))
}

struct Output {
    ok: bool,
    stdout: String,
    stderr: String,
}

impl Signer {
    /// Detached ASCII-armored signature of `buffer`. `ask` supplies an SSH
    /// key's passphrase when ssh-agent doesn't hold the key.
    pub fn sign(&self, buffer: &str, ask: impl Fn(&Path) -> Option<String>) -> Result<String, GitCommandError> {
        match &self.method {
            Method::Ssh(SshKey::Literal(key)) => self.sign_with_literal_key(buffer, key),
            Method::Ssh(SshKey::File(key)) => self.sign_with_key_file(buffer, key, ask),
            Method::OpenPgp(key) => self.sign_gpg(buffer, key.as_deref()),
        }
    }

    fn ssh_command(&self, key: &Path, extra: &[&str]) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(["-Y", "sign", "-n", "git", "-f"]).arg(key).args(extra);
        activity::info(
            self.log_dir.as_deref(),
            format!("$ {} -Y sign -n git -f {} {}", self.program, key.display(), extra.join(" ")).trim_end().to_string(),
        );
        cmd
    }

    fn sign_with_literal_key(&self, buffer: &str, key: &str) -> Result<String, GitCommandError> {
        // ssh-keygen only takes keys as files; the public half is no secret.
        let file = std::env::temp_dir().join(format!("githydra-signing-{}.pub", uuid::Uuid::new_v4()));
        std::fs::write(&file, format!("{key}\n")).map_err(|e| signing_error(e.to_string()))?;
        let mut cmd = self.ssh_command(&file, &["-U"]);
        cmd.env("SSH_ASKPASS_REQUIRE", "never");
        let result = self.run(cmd, buffer);
        let _ = std::fs::remove_file(&file);
        let out = result?;
        if out.ok {
            return Ok(out.stdout);
        }
        Err(signing_error(format!("{} (a chave está carregada no ssh-agent?)", out.stderr.trim())))
    }

    fn sign_with_key_file(&self, buffer: &str, key: &Path, ask: impl Fn(&Path) -> Option<String>) -> Result<String, GitCommandError> {
        let encrypted = passphrase::is_encrypted(key);
        // ssh-keygen tries ssh-agent first, then asks for the passphrase. It
        // always gets it from our askpass helper (an empty one when we have
        // none yet): with `SSH_ASKPASS_REQUIRE=never` it would instead prompt
        // on the terminal the app was started from (blocking the actor) or
        // read the commit buffer on stdin as the passphrase.
        let attempt = |pass: Option<&str>| {
            let mut cmd = self.ssh_command(key, &[]);
            cmd.env("SSH_ASKPASS", &self.askpass).env("SSH_ASKPASS_REQUIRE", "force").env(ASKPASS_ENV, pass.unwrap_or_default());
            self.run(cmd, buffer)
        };

        let cached = if encrypted { passphrase::cached(key) } else { None };
        let first = attempt(cached.as_deref())?;
        if first.ok {
            return Ok(first.stdout);
        }
        if !encrypted {
            return Err(signing_error(first.stderr.trim()));
        }
        if cached.is_some() {
            passphrase::forget(key);
        }
        let pass =
            ask(key).ok_or_else(|| signing_error(format!("assinatura cancelada: nenhuma senha informada para {}", key.display())))?;
        let second = attempt(Some(&pass))?;
        if second.ok {
            return Ok(second.stdout);
        }
        passphrase::forget(key);
        Err(signing_error(second.stderr.trim()))
    }

    fn sign_gpg(&self, buffer: &str, key: Option<&str>) -> Result<String, GitCommandError> {
        let key = match key {
            Some(key) => key.to_string(),
            None => {
                // git's default: the committer identity.
                let log_dir = self.log_dir.as_deref().ok_or_else(|| signing_error("nenhum repositório"))?;
                let sig = git2::Repository::open(log_dir)?.signature()?;
                format!("{} <{}>", sig.name().unwrap_or_default(), sig.email().unwrap_or_default())
            }
        };
        activity::info(self.log_dir.as_deref(), format!("$ {} --status-fd=2 -bsau {key}", self.program));
        let mut cmd = Command::new(&self.program);
        cmd.args(["--status-fd=2", "-bsau", &key]);
        let out = self.run(cmd, buffer)?;
        if out.ok && out.stderr.contains("[GNUPG:] SIG_CREATED ") {
            return Ok(out.stdout);
        }
        Err(signing_error(out.stderr.trim()))
    }

    /// Feeds `input` on stdin; ssh-keygen and gpg both sign stdin to stdout.
    fn run(&self, mut cmd: Command, input: &str) -> Result<Output, GitCommandError> {
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                signing_error(format!("{} não foi encontrado: instale-o ou configure-o no git config", self.program))
            } else {
                signing_error(format!("falha ao executar {}: {e}", self.program))
            }
        })?;
        // Fed from its own thread while the outputs are drained: gpg may write
        // status lines before reading a large commit, which could otherwise
        // deadlock on full pipes.
        let stdin = child.stdin.take();
        let output = std::thread::scope(|scope| {
            if let Some(mut stdin) = stdin {
                scope.spawn(move || {
                    let _ = stdin.write_all(input.as_bytes());
                });
            }
            child.wait_with_output()
        })
        .map_err(|e| signing_error(e.to_string()))?;
        let result = Output {
            ok: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        };
        if !result.ok {
            activity::error(self.log_dir.as_deref(), format!("{} falhou: {}", self.program, result.stderr.trim()));
        }
        Ok(result)
    }
}

/// Environment for a git CLI run that makes commits (interactive rebase),
/// so they're signed like the app's own: the "Sign commits" setting becomes
/// git config for that one run (`GIT_CONFIG_*`), and ssh-keygen gets a
/// locked key's passphrase from our askpass helper — asked up front, since
/// git would stop on a prompt nobody sees. As for the app's own ssh-keygen
/// runs, the passphrase only ever travels in the environment.
pub fn git_cli_env(repo: &git2::Repository) -> Result<Vec<(String, String)>, GitCommandError> {
    let Some(signer) = signer_for(repo)? else {
        return Ok(Vec::new());
    };
    let forced = !repo.config()?.get_bool("commit.gpgsign").unwrap_or(false);
    let reply = match &signer.method {
        Method::Ssh(SshKey::File(key)) if passphrase::is_encrypted(key) => {
            // A throwaway signature: ssh-agent first, else the prompt (cached).
            signer.sign("githydra signing check\n", passphrase::get)?;
            passphrase::cached(key)
        }
        _ => None,
    };
    Ok(cli_env(&signer, forced, reply))
}

/// `forced`: signing comes from the app setting, not from git config.
pub(crate) fn cli_env(signer: &Signer, forced: bool, reply: Option<String>) -> Vec<(String, String)> {
    let Method::Ssh(key) = &signer.method else {
        // gpg asks through its own agent/pinentry, as it does for git.
        return Vec::new();
    };
    let mut env = Vec::new();
    if forced {
        let key = match key {
            SshKey::File(path) => path.display().to_string(),
            SshKey::Literal(literal) => format!("key::{literal}"),
        };
        let config = [
            ("commit.gpgsign", "true".to_string()),
            ("gpg.format", "ssh".to_string()),
            ("gpg.ssh.program", signer.program.clone()),
            ("user.signingkey", key),
        ];
        env.push(("GIT_CONFIG_COUNT".into(), config.len().to_string()));
        for (i, (k, v)) in config.into_iter().enumerate() {
            env.push((format!("GIT_CONFIG_KEY_{i}"), k.to_string()));
            env.push((format!("GIT_CONFIG_VALUE_{i}"), v));
        }
    }
    env.push(("SSH_ASKPASS".into(), signer.askpass.display().to_string()));
    env.push(("SSH_ASKPASS_REQUIRE".into(), "force".into()));
    env.push((ASKPASS_ENV.into(), reply.unwrap_or_default()));
    env
}

/// Creates a commit object (no ref update), signed when [`signer_for`] says
/// so. Every commit the app makes goes through here.
pub fn create_commit(
    repo: &git2::Repository,
    author: &git2::Signature,
    committer: &git2::Signature,
    message: &str,
    tree: &git2::Tree,
    parents: &[&git2::Commit],
) -> Result<git2::Oid, GitCommandError> {
    let signer = signer_for(repo)?;
    create_commit_with(repo, signer.as_ref(), author, committer, message, tree, parents)
}

/// [`create_commit`] with the signer resolved once by the caller, for paths
/// that create many commits in a row (reword rebuilding a chain).
pub fn create_commit_with(
    repo: &git2::Repository,
    signer: Option<&Signer>,
    author: &git2::Signature,
    committer: &git2::Signature,
    message: &str,
    tree: &git2::Tree,
    parents: &[&git2::Commit],
) -> Result<git2::Oid, GitCommandError> {
    let Some(signer) = signer else {
        return Ok(repo.commit(None, author, committer, message, tree, parents)?);
    };
    let buffer = repo.commit_create_buffer(author, committer, message, tree, parents)?;
    let buffer = buffer.as_str().map_err(|_| signing_error("o commit não é UTF-8 válido"))?;
    let signature = signer.sign(buffer, passphrase::get)?;
    Ok(repo.commit_signed(buffer, &signature, None)?)
}

/// Moves HEAD's branch (or HEAD itself when detached) to `oid`, like the
/// final step of `git commit`, without libgit2's "tip must be the first
/// parent" check (amend and signed commits don't satisfy it).
pub fn advance_head(repo: &git2::Repository, oid: git2::Oid, reflog: &str) -> Result<(), GitCommandError> {
    let head = repo.find_reference("HEAD")?;
    let target = head.symbolic_target()?.unwrap_or("HEAD").to_string();
    repo.reference(&target, oid, true, reflog)?;
    Ok(())
}

#[derive(Debug, Serialize, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSignature {
    /// "ssh", "gpg", "x509" or "unknown".
    pub format: String,
}

/// Whether `sha` carries a signature, and of which kind. Not verified:
/// that would need the user's trust setup (allowed signers / keyring).
pub fn commit_signature(repo: &git2::Repository, sha: &str) -> Result<Option<CommitSignature>, GitCommandError> {
    let oid = git2::Oid::from_str(sha)?;
    let signature = match repo.extract_signature(&oid, None) {
        Ok((signature, _)) => signature,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let text = String::from_utf8_lossy(&signature);
    let format = if text.starts_with("-----BEGIN SSH SIGNATURE") {
        "ssh"
    } else if text.starts_with("-----BEGIN PGP SIGNATURE") {
        "gpg"
    } else if text.starts_with("-----BEGIN SIGNED MESSAGE") {
        "x509"
    } else {
        "unknown"
    };
    Ok(Some(CommitSignature { format: format.into() }))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn parses_signing_key_forms() {
        assert_eq!(parse_ssh_key("key::ssh-ed25519 AAAA x"), SshKey::Literal("ssh-ed25519 AAAA x".into()));
        assert_eq!(parse_ssh_key("ssh-rsa AAAA"), SshKey::Literal("ssh-rsa AAAA".into()));
        assert_eq!(parse_ssh_key("/k/id"), SshKey::File("/k/id".into()));
        if let Some(home) = std::env::var_os("HOME") {
            assert_eq!(parse_ssh_key("~/.ssh/id"), SshKey::File(PathBuf::from(home).join(".ssh/id")));
        }
    }

    #[test]
    fn no_signing_unless_configured() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        repo.config().unwrap().set_bool("commit.gpgsign", false).unwrap();
        assert!(signer_for(&repo).unwrap().is_none());
    }

    #[test]
    fn reads_format_key_and_program_from_git_config() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_bool("commit.gpgsign", true).unwrap();
        config.set_str("gpg.format", "ssh").unwrap();
        config.set_str("user.signingkey", "/k/id").unwrap();
        config.set_str("gpg.ssh.program", "my-keygen").unwrap();
        let signer = signer_for(&repo).unwrap().unwrap();
        assert_eq!(signer.method, Method::Ssh(SshKey::File("/k/id".into())));
        assert_eq!(signer.program, "my-keygen");

        config.set_str("gpg.format", "openpgp").unwrap();
        config.set_str("user.signingkey", "ABCD").unwrap();
        let signer = signer_for(&repo).unwrap().unwrap();
        assert_eq!(signer.method, Method::OpenPgp(Some("ABCD".into())));

        config.set_str("gpg.format", "x509").unwrap();
        assert!(matches!(signer_for(&repo), Err(GitCommandError::SigningFailed { .. })));
    }

    #[test]
    fn missing_program_is_a_clear_error() {
        let signer = Signer {
            method: Method::OpenPgp(Some("ABCD".into())),
            program: "githydra-no-such-gpg".into(),
            askpass: PathBuf::new(),
            log_dir: None,
        };
        match signer.sign("data", |_| None) {
            Err(GitCommandError::SigningFailed { message }) => assert!(message.contains("não foi encontrado"), "{message}"),
            other => panic!("expected SigningFailed, got {other:?}"),
        }
    }

    #[test]
    fn cli_env_turns_the_app_setting_into_git_config() {
        let signer = Signer {
            method: Method::Ssh(SshKey::File(PathBuf::from("/k/id"))),
            program: "ssh-keygen".into(),
            askpass: PathBuf::from("/app"),
            log_dir: None,
        };
        let env = |forced, reply: Option<&str>| {
            cli_env(&signer, forced, reply.map(String::from)).into_iter().collect::<std::collections::HashMap<_, _>>()
        };
        let forced = env(true, Some("pw"));
        assert_eq!(forced["GIT_CONFIG_COUNT"], "4");
        let config: Vec<_> =
            (0..4).map(|i| (forced[&format!("GIT_CONFIG_KEY_{i}")].as_str(), forced[&format!("GIT_CONFIG_VALUE_{i}")].as_str())).collect();
        assert!(config.contains(&("commit.gpgsign", "true")));
        assert!(config.contains(&("gpg.format", "ssh")));
        assert!(config.contains(&("user.signingkey", "/k/id")));
        assert_eq!(forced["SSH_ASKPASS"], "/app");
        assert_eq!(forced["SSH_ASKPASS_REQUIRE"], "force");
        assert_eq!(forced[ASKPASS_ENV], "pw");

        // git config already signs: only the askpass part, never an empty prompt on a terminal.
        let configured = env(false, None);
        assert!(!configured.contains_key("GIT_CONFIG_COUNT"));
        assert_eq!(configured[ASKPASS_ENV], "");

        let gpg = Signer { method: Method::OpenPgp(None), ..signer.clone() };
        assert!(cli_env(&gpg, true, None).is_empty());
    }

    #[test]
    fn unsigned_commits_have_no_signature() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let sig = git2::Signature::now("T", "t@example.com").unwrap();
        let tree = repo.find_tree(repo.index().unwrap().write_tree().unwrap()).unwrap();
        let oid = repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();
        assert_eq!(commit_signature(&repo, &oid.to_string()).unwrap(), None);
    }

    /// Runs the real tools; skipped when ssh-keygen or git are missing.
    #[cfg(unix)]
    mod with_ssh_keygen {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        pub(crate) fn keygen(dir: &Path, name: &str, pass: &str) -> Option<PathBuf> {
            let path = dir.join(name);
            let ok = Command::new("ssh-keygen")
                .args(["-q", "-t", "ed25519", "-C", "t@example.com", "-N", pass, "-f"])
                .arg(&path)
                .status()
                .is_ok_and(|s| s.success());
            ok.then_some(path)
        }

        /// `git verify-commit` against an allowed_signers file for `key`;
        /// `None` only when git itself can't be run.
        pub(crate) fn git_verifies(repo_dir: &Path, key: &Path, sha: &str) -> Option<bool> {
            let public = std::fs::read_to_string(key.with_extension("pub")).unwrap();
            let allowed = repo_dir.join("allowed_signers");
            std::fs::write(&allowed, format!("t@example.com namespaces=\"git\" {public}")).unwrap();
            let out = Command::new("git")
                .current_dir(repo_dir)
                .arg("-c")
                .arg(format!("gpg.ssh.allowedSignersFile={}", allowed.display()))
                .args(["verify-commit", sha])
                .output()
                .ok()?;
            Some(out.status.success())
        }

        fn test_repo(dir: &Path, key: &Path) -> git2::Repository {
            let repo = git2::Repository::init(dir.join("repo")).unwrap();
            let mut config = repo.config().unwrap();
            config.set_bool("commit.gpgsign", true).unwrap();
            config.set_str("gpg.format", "ssh").unwrap();
            config.set_str("user.signingkey", &key.display().to_string()).unwrap();
            config.set_str("user.name", "T").unwrap();
            config.set_str("user.email", "t@example.com").unwrap();
            repo
        }

        #[test]
        fn signed_commit_verifies_with_git() {
            let tmp = tempfile::tempdir().unwrap();
            let Some(key) = keygen(tmp.path(), "id", "") else { return };
            let repo = test_repo(tmp.path(), &key);
            let sig = repo.signature().unwrap();
            let tree = repo.find_tree(repo.index().unwrap().write_tree().unwrap()).unwrap();
            let oid = create_commit(&repo, &sig, &sig, "signed\n", &tree, &[]).unwrap();
            advance_head(&repo, oid, "commit (initial): signed").unwrap();
            assert_eq!(repo.head().unwrap().target(), Some(oid));
            assert_eq!(commit_signature(&repo, &oid.to_string()).unwrap(), Some(CommitSignature { format: "ssh".into() }));
            if let Some(ok) = git_verifies(repo.workdir().unwrap(), &key, &oid.to_string()) {
                assert!(ok, "git verify-commit rejected the signature");
                // And it really checks: another key isn't accepted.
                let other = keygen(tmp.path(), "other", "").unwrap();
                assert_eq!(git_verifies(repo.workdir().unwrap(), &other, &oid.to_string()), Some(false));
            }
        }

        #[test]
        fn locked_key_gets_its_passphrase_through_askpass() {
            let tmp = tempfile::tempdir().unwrap();
            let Some(key) = keygen(tmp.path(), "locked", "pw") else { return };
            // Stands in for this executable in askpass mode, logging each reply.
            let askpass = tmp.path().join("askpass.sh");
            let replies = tmp.path().join("replies.log");
            std::fs::write(
                &askpass,
                format!("#!/bin/sh\nprintf '%s|' \"${ASKPASS_ENV}\" >> '{}'\necho \"${ASKPASS_ENV}\"\n", replies.display()),
            )
            .unwrap();
            std::fs::set_permissions(&askpass, std::fs::Permissions::from_mode(0o755)).unwrap();
            let signer = Signer { method: Method::Ssh(SshKey::File(key.clone())), program: "ssh-keygen".into(), askpass, log_dir: None };
            let asked = std::cell::Cell::new(0);
            let signature = signer
                .sign("payload\n", |k| {
                    assert_eq!(k, key.as_path());
                    asked.set(asked.get() + 1);
                    Some("pw".into())
                })
                .unwrap();
            assert!(signature.starts_with("-----BEGIN SSH SIGNATURE-----"));
            assert_eq!(asked.get(), 1);
            // The agent-only first try still went through askpass (empty), never
            // a terminal prompt or the commit buffer on stdin.
            assert_eq!(std::fs::read_to_string(&replies).unwrap(), "|pw|");

            passphrase::forget(&key);
            let wrong = signer.sign("payload\n", |_| Some("nope".into()));
            assert!(matches!(wrong, Err(GitCommandError::SigningFailed { .. })));
            let cancelled = signer.sign("payload\n", |_| None);
            assert!(matches!(cancelled, Err(GitCommandError::SigningFailed { .. })));
        }
    }
    #[cfg(unix)]
    pub(crate) use with_ssh_keygen::{git_verifies, keygen};
}

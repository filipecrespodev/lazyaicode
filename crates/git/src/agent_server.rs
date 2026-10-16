//! `lazyaicode-agent`: the git client backend, headless, serving the app over
//! stdio from inside a WSL distro (see `proto`). Not to be confused with
//! `crate::agent` (the Claude Code threads). Everything a repo needs — libgit2, hooks,
//! ssh, gpg, git-lfs, credential helpers, `~/.ssh` — then runs natively on
//! the Linux side; only small JSON frames cross to Windows.

use std::io::{stdin, stdout, BufReader, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::GitCommandError;
use crate::git::actor::RepoCommand;
use crate::git::remote::AuthMode;
use crate::git::{external_git, fingerprint, lfs};
use crate::proto::{decode, read_frame, write_frame, FromAgent, ToAgent, PROTOCOL_VERSION};
use crate::state::AppState;
use crate::{host as workspace, https_credentials, passphrase, settings, ssh_keys};

fn args<T: DeserializeOwned>(v: &Value, key: &str) -> Result<T, GitCommandError> {
    serde_json::from_value(v.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|e| GitCommandError::Internal(format!("argumento inválido {key}: {e}")))
}

fn json_of(v: impl serde::Serialize) -> Result<Value, GitCommandError> {
    serde_json::to_value(v).map_err(|e| GitCommandError::Internal(e.to_string()))
}

/// Everything the app can ask of the agent; blocking.
pub fn handle(state: &AppState, op: &str, a: Value) -> Result<Value, GitCommandError> {
    match op {
        "repo" => {
            let repo_id: String = args(&a, "repoId")?;
            let (cmd, waiter) = decode::<RepoCommand>(a.get("cmd").cloned().unwrap_or(Value::Null))?;
            state.actor(&repo_id)?.send(cmd)?;
            Ok(waiter())
        }
        "open_repository" => json_of(state.open_local(args(&a, "repoId")?, args(&a, "path")?)?),
        "close_repository" => {
            state.repos.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&args::<String>(&a, "repoId")?);
            Ok(Value::Null)
        }
        "fingerprint" => {
            let path = state.repo_path(&args::<String>(&a, "repoId")?)?;
            json_of(fingerprint::fingerprint(Path::new(&path))?)
        }
        "lfs_status" => {
            let path = state.repo_path(&args::<String>(&a, "repoId")?)?;
            json_of(lfs::status(Path::new(&path)))
        }
        "lfs_pull" => {
            let path = state.repo_path(&args::<String>(&a, "repoId")?)?;
            external_git::lfs_pull(Path::new(&path))?;
            Ok(Value::Null)
        }
        "scan_workspace" => json_of(workspace::scan(Path::new(&args::<String>(&a, "path")?))?),
        "list_dirs" => json_of(workspace::list_dirs(&args::<String>(&a, "path")?)?),
        "fetch_repo_at_path" => {
            let background: Option<bool> = args(&a, "background")?;
            let mode = if background.unwrap_or(false) { AuthMode::Background } else { AuthMode::Interactive };
            workspace::fetch_at_path(&args::<String>(&a, "path")?, mode)?;
            Ok(Value::Null)
        }
        "list_ssh_keys" => json_of(ssh_keys::list_ssh_keys()),
        "global_identity" => json_of(crate::identity::global()?),
        "submit_ssh_passphrase" => {
            passphrase::submit_ssh_passphrase(args(&a, "passphrase")?);
            Ok(Value::Null)
        }
        "pending_ssh_passphrase" => json_of(passphrase::pending_ssh_passphrase()),
        "submit_https_credentials" => {
            https_credentials::submit_https_credentials(args(&a, "username")?, args(&a, "password")?);
            Ok(Value::Null)
        }
        "pending_https_credentials" => json_of(https_credentials::pending_https_credentials()),
        other => Err(GitCommandError::Internal(format!("operação desconhecida do agente: {other}"))),
    }
}

/// Serves requests on stdin/stdout until the app hangs up.
pub fn serve_stdio() {
    let out = Arc::new(Mutex::new(stdout()));
    let send = {
        let out = out.clone();
        move |frame: FromAgent| {
            let mut out = out.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let _ = write_frame(&mut *out, &frame).and_then(|_| out.flush());
        }
    };
    let event_send = send.clone();
    crate::events::set_sink(move |name, payload| {
        event_send(FromAgent::Event { name: name.to_string(), payload });
    });

    let home = std::env::var("HOME").unwrap_or_default();
    send(FromAgent::Hello { version: PROTOCOL_VERSION, home });

    let state = Arc::new(AppState::default());
    let mut input = BufReader::new(stdin().lock());
    while let Ok(Some(ToAgent::Req { id, op, args, settings: pushed })) = read_frame::<ToAgent>(&mut input) {
        let (state, send) = (state.clone(), send.clone());
        // One thread per request: ops block (network, prompts waiting on
        // the user) and must not queue behind each other.
        std::thread::spawn(move || {
            settings::apply(pushed.into_iter().collect());
            let result = handle(&state, &op, args);
            send(FromAgent::Res { id, result });
        });
    }
}

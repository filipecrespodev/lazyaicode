//! App side of the WSL bridge: one persistent `lazyaicode-agent` process per
//! distro, spoken to over its stdio (see `proto`). Requests are multiplexed
//! by id, so many repos and many calls share the single pipe.

use std::collections::HashMap;
use std::io::BufReader;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::git_client::error::GitCommandError;
use crate::git_client::proto::{read_frame, write_frame, FromAgent, ToAgent, PROTOCOL_VERSION};

type Pending = HashMap<u64, oneshot::Sender<Result<Value, GitCommandError>>>;

fn internal(message: impl Into<String>) -> GitCommandError {
    GitCommandError::Internal(message.into())
}

/// Events name repos by their Linux path; the app knows them by UNC path.
fn localize(distro: &str, name: &str, mut payload: Value) -> Value {
    if name == "activity://log" {
        if let Some(path) = payload.get("repoPath").and_then(Value::as_str).map(|p| crate::git_client::wslpath::to_unc(distro, p)) {
            payload["repoPath"] = Value::String(path);
        }
    }
    payload
}

/// The app's settings as this distro's agent should see them: SSH key
/// choices are stored with Windows/UNC paths, but only Linux paths mean
/// anything inside the distro.
fn settings_for(distro: &str) -> HashMap<String, Value> {
    let mut settings = crate::git_client::settings::snapshot();
    if let Some(keys) = settings.get_mut("sshKeys").and_then(Value::as_object_mut) {
        if keys.get("defaultKey").and_then(Value::as_str).is_some_and(|k| !k.starts_with('/')) {
            keys.remove("defaultKey");
        }
        if let Some(overrides) = keys.get_mut("overrides").and_then(Value::as_object_mut) {
            let own: serde_json::Map<String, Value> = overrides
                .iter()
                .filter_map(|(dir, key)| {
                    let (d, linux) = crate::git_client::wslpath::parse_unc(dir)?;
                    (d.eq_ignore_ascii_case(distro) && key.as_str()?.starts_with('/')).then(|| (linux, key.clone()))
                })
                .collect();
            *overrides = own;
        }
    }
    settings
}

pub struct AgentConn {
    stdin: Mutex<ChildStdin>,
    pending: Arc<Mutex<Pending>>,
    next_id: AtomicU64,
    child: Mutex<Child>,
    /// The agent's `$HOME` (where its `~/.ssh` lives), from its hello.
    pub home: String,
    pub distro: String,
}

impl AgentConn {
    /// Starts `cmd` (an agent speaking `proto` on stdio) and waits for its
    /// hello. Blocking: wsl.exe may have to boot the distro first.
    pub fn connect(mut cmd: Command, distro: String) -> Result<Arc<Self>, GitCommandError> {
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
        let mut child = cmd.spawn().map_err(|e| internal(format!("não foi possível iniciar o agente do WSL: {e}")))?;
        let stdin = child.stdin.take().ok_or_else(|| internal("o agente não tem stdin"))?;
        let mut stdout = BufReader::new(child.stdout.take().ok_or_else(|| internal("o agente não tem stdout"))?);

        let home = match read_frame::<FromAgent>(&mut stdout) {
            Ok(Some(FromAgent::Hello { version, home })) if version == PROTOCOL_VERSION => home,
            Ok(Some(FromAgent::Hello { version, .. })) => {
                let _ = child.kill();
                return Err(internal(format!(
                    "o agente do WSL fala o protocolo {version}; este app precisa do {PROTOCOL_VERSION}"
                )));
            }
            other => {
                let _ = child.kill();
                return Err(internal(format!("o agente do WSL não se apresentou ({other:?})")));
            }
        };

        let pending: Arc<Mutex<Pending>> = Arc::default();
        let reader_pending = pending.clone();
        let reader_distro = distro.clone();
        std::thread::spawn(move || {
            while let Ok(Some(frame)) = read_frame::<FromAgent>(&mut stdout) {
                match frame {
                    FromAgent::Res { id, result } => {
                        if let Some(tx) = reader_pending.lock().unwrap().remove(&id) {
                            let _ = tx.send(result);
                        }
                    }
                    FromAgent::Event { name, payload } => crate::git_client::events::emit(&name, localize(&reader_distro, &name, payload)),
                    FromAgent::Hello { .. } => {}
                }
            }
            for (_, tx) in reader_pending.lock().unwrap().drain() {
                let _ = tx.send(Err(internal("o agente do WSL desconectou")));
            }
        });

        Ok(Arc::new(Self {
            stdin: Mutex::new(stdin),
            pending,
            next_id: AtomicU64::new(1),
            child: Mutex::new(child),
            home,
            distro,
        }))
    }

    pub fn is_alive(&self) -> bool {
        matches!(self.child.lock().unwrap().try_wait(), Ok(None))
    }

    pub async fn call(&self, op: &str, args: Value) -> Result<Value, GitCommandError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let req = ToAgent::Req { id, op: op.into(), args, settings: settings_for(&self.distro) };
        if let Err(e) = write_frame(&mut *self.stdin.lock().unwrap(), &req) {
            self.pending.lock().unwrap().remove(&id);
            return Err(internal(format!("o agente do WSL encerrou: {e}")));
        }
        rx.await.map_err(|_| internal("o agente do WSL desconectou"))?
    }

    pub async fn call_as<T: DeserializeOwned>(&self, op: &str, args: Value) -> Result<T, GitCommandError> {
        let value = self.call(op, args).await?;
        serde_json::from_value(value).map_err(|e| internal(format!("resposta inválida a {op}: {e}")))
    }
}

impl Drop for AgentConn {
    fn drop(&mut self) {
        let _ = self.child.lock().unwrap().kill();
    }
}

#[derive(Clone)]
pub struct RemoteRepo {
    pub conn: Arc<AgentConn>,
    pub repo_id: String,
}

/// Keys whose absolute Linux paths the app must see as UNC paths.
const PATH_KEYS: [&str; 4] = ["path", "absPath", "commonDir", "workdir"];

/// Rewrites paths crossing the pipe: UNC → Linux in what the app sends (every
/// string; nobody types a `\\wsl.localhost\...` for another purpose), Linux →
/// UNC in the known path fields of what comes back. Git-relative paths never
/// start with `/`, so those are left alone.
fn translate(value: &mut Value, distro: &str, to_linux: bool) {
    match value {
        Value::String(s) if to_linux => {
            if let Some((_, linux)) = crate::git_client::wslpath::parse_unc(s) {
                *s = linux;
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| translate(v, distro, to_linux)),
        Value::Object(map) => {
            for (key, v) in map.iter_mut() {
                match v {
                    Value::String(s) if !to_linux && s.starts_with('/') && PATH_KEYS.contains(&key.as_str()) => {
                        *s = crate::git_client::wslpath::to_unc(distro, s);
                    }
                    _ => translate(v, distro, to_linux),
                }
            }
        }
        _ => {}
    }
}

impl RemoteRepo {
    /// Runs a `RepoCommand` in the agent's actor for this repo.
    pub async fn call<T: DeserializeOwned>(&self, cmd: &impl Serialize) -> Result<T, GitCommandError> {
        let mut cmd = serde_json::to_value(cmd).map_err(|e| internal(e.to_string()))?;
        translate(&mut cmd, &self.conn.distro, true);
        let mut reply = self
            .conn
            .call("repo", json!({ "repoId": self.repo_id, "cmd": cmd }))
            .await?;
        translate(&mut reply, &self.conn.distro, false);
        serde_json::from_value::<Result<T, GitCommandError>>(reply)
            .map_err(|e| internal(format!("resposta inválida do agente do WSL: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_key_settings_keep_only_what_the_distro_can_use() {
        crate::git_client::settings::apply(HashMap::from([(
            "sshKeys".to_string(),
            json!({
                "defaultKey": r"C:\\Users\\me\\.ssh\\id_ed25519",
                "overrides": {
                    r"\\wsl.localhost\\U\\home\\me\\repo": "/home/me/.ssh/work",
                    r"\\wsl.localhost\\Other\\repo": "/home/x/.ssh/k",
                    r"C:\\win\\repo": r"C:\\k",
                }
            }),
        )]));
        let got = settings_for("U");
        assert_eq!(got["sshKeys"], json!({ "overrides": { "/home/me/repo": "/home/me/.ssh/work" } }));
    }

    #[test]
    fn translates_paths_both_ways() {
        let mut out = json!({ "Add": { "path": r"\\wsl.localhost\U\home\me\wt", "branch": "x" } });
        translate(&mut out, "U", true);
        assert_eq!(out["Add"]["path"], "/home/me/wt");

        let mut back = json!({ "Ok": [{ "path": "/home/me/wt", "files": [{ "path": "src/a.rs" }] }] });
        translate(&mut back, "U", false);
        assert_eq!(back["Ok"][0]["path"], r"\\wsl.localhost\U\home\me\wt");
        assert_eq!(back["Ok"][0]["files"][0]["path"], "src/a.rs");
    }
}

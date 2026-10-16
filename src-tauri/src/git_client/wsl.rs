//! Reaching a WSL distro from the app: starting its `lazyaicode-agent`,
//! listing distros, and routing paths to the right backend.
//!
//! Only the Windows app ever sees `\\wsl.localhost\...` paths; on Linux/macOS
//! `list_distros` is empty and routing only happens through the
//! `LAZYAICODE_AGENT_BIN` override (the loopback tests).

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, OnceLock};

use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::git_client::error::GitCommandError;
use crate::git_client::git::actor::RepoActor;
use crate::git_client::git::repo::RepoSummary;
use crate::git_client::remote_agent::{AgentConn, RemoteRepo};
use crate::git_client::state::{AppState, RepoHandle};
use crate::git_client::wslpath;

/// Dev/test override: a local `lazyaicode-agent` binary to run directly
/// instead of going through `wsl.exe`.
pub const AGENT_OVERRIDE_ENV: &str = "LAZYAICODE_AGENT_BIN";

/// Where the app installs the agent inside a distro (under `$HOME`).
pub const AGENT_DIR: &str = ".lazyaicode";
pub const AGENT_REL_PATH: &str = ".lazyaicode/lazyaicode-agent";

/// The Linux agent shipped inside the app, copied into a distro on first use
/// and whenever the app's version changes. Unset in dev builds.
static BUNDLED_AGENT: OnceLock<PathBuf> = OnceLock::new();

pub fn set_bundled_agent(path: PathBuf) {
    if path.is_file() {
        let _ = BUNDLED_AGENT.set(path);
    }
}

/// What `lazyaicode-agent --version` prints for this build.
pub fn agent_version() -> String {
    format!("{} proto{}", env!("CARGO_PKG_VERSION"), crate::git_client::proto::PROTOCOL_VERSION)
}

#[cfg(windows)]
fn wsl_sh(distro: &str, script: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut cmd = Command::new("wsl.exe");
    cmd.args(["-d", distro, "--exec", "sh", "-c", script]);
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

/// Puts the bundled agent into `distro` unless the installed one is current.
/// Blocking. No bundled agent (dev builds): leaves the distro as it is.
#[cfg(windows)]
pub fn ensure_agent_installed(distro: &str) -> Result<(), GitCommandError> {
    use std::io::Write;
    use std::process::Stdio;

    let Some(bundled) = BUNDLED_AGENT.get() else { return Ok(()) };
    let fail = |what: &str, e: &dyn std::fmt::Display| {
        GitCommandError::Internal(format!("não foi possível {what} o agente do LazyAICode no WSL ({distro}): {e}"))
    };
    let installed = wsl_sh(distro, &format!("\"$HOME/{AGENT_REL_PATH}\" --version"))
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    if installed.as_deref() == Some(agent_version().as_str()) {
        return Ok(());
    }
    let bytes = std::fs::read(bundled).map_err(|e| fail("ler", &e))?;
    // Written beside the target, then renamed: a running old agent keeps its
    // inode, and a half-copied file is never executed.
    let script = format!(
        "mkdir -p \"$HOME/{AGENT_DIR}\" && cat > \"$HOME/{AGENT_REL_PATH}.new\" && chmod +x \"$HOME/{AGENT_REL_PATH}.new\" && mv -f \"$HOME/{AGENT_REL_PATH}.new\" \"$HOME/{AGENT_REL_PATH}\""
    );
    let mut child = wsl_sh(distro, &script)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| fail("instalar", &e))?;
    child.stdin.take().expect("piped").write_all(&bytes).map_err(|e| fail("instalar", &e))?;
    let out = child.wait_with_output().map_err(|e| fail("instalar", &e))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(fail("instalar", &String::from_utf8_lossy(&out.stderr).trim()))
    }
}

/// Off Windows there is no WSL to install into.
#[cfg(not(windows))]
pub fn ensure_agent_installed(_distro: &str) -> Result<(), GitCommandError> {
    let _ = BUNDLED_AGENT.get();
    Ok(())
}

pub fn agent_command(distro: &str) -> Result<Command, GitCommandError> {
    if let Some(bin) = std::env::var_os(AGENT_OVERRIDE_ENV) {
        let mut cmd = Command::new(bin);
        cmd.arg("serve");
        return Ok(cmd);
    }
    if !cfg!(windows) {
        return Err(GitCommandError::Internal(format!(
            "o WSL só existe no Windows (distro {distro}); defina {AGENT_OVERRIDE_ENV} para testar o agente"
        )));
    }
    let mut cmd = Command::new("wsl.exe");
    cmd.args(["-d", distro, "--exec", "sh", "-c"])
        .arg(format!("exec \"$HOME/{AGENT_REL_PATH}\" serve"));
    Ok(cmd)
}

/// Runs blocking `f` on its own thread, awaitable from async commands.
pub async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, GitCommandError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.await.map_err(|_| GitCommandError::Internal("a thread de trabalho morreu".into()))
}

/// Distro names from `wsl.exe -l -q` (UTF-16LE output).
#[cfg(windows)]
pub fn list_distros() -> Vec<String> {
    use std::os::windows::process::CommandExt;
    let out = match Command::new("wsl.exe").args(["-l", "-q"]).creation_flags(0x0800_0000).output() {
        Ok(out) if out.status.success() => out.stdout,
        _ => return Vec::new(),
    };
    parse_distro_list(&out)
}

/// Empty off Windows.
#[cfg(not(windows))]
pub fn list_distros() -> Vec<String> {
    Vec::new()
}

/// `wsl.exe -l -q` prints UTF-16LE, one distro per line; Docker Desktop's
/// internal distros are left out.
pub fn parse_distro_list(out: &[u8]) -> Vec<String> {
    let units: Vec<u16> = out.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units)
        .lines()
        .map(|l| l.trim().trim_start_matches('\u{feff}').to_string())
        .filter(|l| !l.is_empty() && !l.starts_with("docker-desktop"))
        .collect()
}

impl AppState {
    /// The (lazily started) agent of `distro`; restarted if it died.
    pub async fn agent(&self, distro: &str) -> Result<Arc<AgentConn>, GitCommandError> {
        if let Some(conn) = self.agents.lock().unwrap().get(distro).filter(|c| c.is_alive()) {
            return Ok(conn.clone());
        }
        let name = distro.to_string();
        let conn = blocking(move || {
            if std::env::var_os(AGENT_OVERRIDE_ENV).is_none() {
                ensure_agent_installed(&name)?;
            }
            AgentConn::connect(agent_command(&name)?, name)
        })
        .await??;
        self.agents.lock().unwrap().insert(distro.to_string(), conn.clone());
        Ok(conn)
    }

    /// For a WSL UNC path: its agent and the Linux path. `None` for local paths.
    pub async fn route(&self, path: &str) -> Result<Option<(Arc<AgentConn>, String)>, GitCommandError> {
        match wslpath::parse_unc(path) {
            Some((distro, linux)) => Ok(Some((self.agent(&distro).await?, linux))),
            None => Ok(None),
        }
    }

    /// Opens a repo of a WSL distro through its agent; paths in the summary
    /// come back as UNC paths, which is how the app knows the repo from then on.
    pub async fn open_remote(&self, distro: &str, linux_path: String) -> Result<RepoSummary, GitCommandError> {
        let agent = self.agent(distro).await?;
        let repo_id = uuid::Uuid::new_v4().to_string();
        let mut summary: RepoSummary = agent
            .call_as("open_repository", json!({ "repoId": repo_id, "path": linux_path }))
            .await?;
        let unc = |p: &str| wslpath::to_unc(distro, p);
        summary.path = unc(&summary.path);
        summary.common_dir = unc(&summary.common_dir);
        let actor = RepoActor::Remote(RemoteRepo { conn: agent.clone(), repo_id: repo_id.clone() });
        self.repos
            .lock()
            .unwrap()
            .insert(repo_id, RepoHandle { actor, path: summary.path.clone(), agent: Some(agent) });
        Ok(summary)
    }

    /// Git repos in `path` and its immediate children; for a WSL path the
    /// scan runs inside the distro (not over 9P) and entries come back as UNC
    /// paths.
    pub async fn scan_workspace(
        &self,
        path: &str,
    ) -> Result<Vec<crate::git_client::host::WorkspaceRepoEntry>, GitCommandError> {
        let Some((agent, linux)) = self.route(path).await? else {
            let path = path.to_string();
            return blocking(move || crate::git_client::host::scan(std::path::Path::new(&path))).await?;
        };
        let mut entries: Vec<crate::git_client::host::WorkspaceRepoEntry> =
            agent.call_as("scan_workspace", json!({ "path": linux })).await?;
        for e in &mut entries {
            e.path = wslpath::to_unc(&agent.distro, &e.path);
        }
        Ok(entries)
    }

    fn live_agents(&self) -> Vec<Arc<AgentConn>> {
        self.agents.lock().unwrap().values().filter(|c| c.is_alive()).cloned().collect()
    }

    /// Prompt answers/queries are global: tell every live agent, since the
    /// one that asked is not tracked.
    pub async fn broadcast(&self, op: &str, args: Value) {
        for conn in self.live_agents() {
            let _ = conn.call(op, args.clone()).await;
        }
    }

    /// First non-null answer among the agents (for `pending_*` queries).
    pub async fn first_answer<T: DeserializeOwned>(&self, op: &str) -> Option<T> {
        for conn in self.live_agents() {
            if let Ok(Some(v)) = conn.call_as::<Option<T>>(op, json!({})).await {
                return Some(v);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distro_list_is_utf16_without_docker_ones() {
        let text = "\u{feff}Ubuntu\r\ndocker-desktop\r\nDebian\r\n\r\n";
        let bytes: Vec<u8> = text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert_eq!(parse_distro_list(&bytes), vec!["Ubuntu", "Debian"]);
    }

    #[test]
    fn agent_version_names_the_protocol() {
        assert!(agent_version().ends_with(&format!("proto{}", crate::git_client::proto::PROTOCOL_VERSION)));
    }
}

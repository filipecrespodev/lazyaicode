//! Um processo `claude -p` por thread, falando stream-json nos dois sentidos.
//!
//! O processo fica vivo entre turnos (cada mensagem do usuário é uma linha no
//! stdin), e quando morre é recriado com `--resume <session_id>`. Pedidos de
//! permissão chegam como `control_request { subtype: "can_use_tool" }` porque
//! o CLI é iniciado com `--permission-prompt-tool stdio` — o mesmo protocolo
//! que o Agent SDK usa.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;

/// Imagem anexada a uma mensagem (colada ou arrastada no chat), em base64.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ImageInput {
    pub mime: String,
    pub data: String,
}

/// Tipos que o Claude aceita em blocos de imagem.
pub const IMAGE_MIMES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

/// Conteúdo de uma mensagem do usuário no formato do Claude: imagens antes do
/// texto. É também o que vai para a transcrição das threads ACP.
pub fn user_content(text: &str, images: &[ImageInput]) -> Vec<Value> {
    let mut c: Vec<Value> =
        images.iter().map(|i| json!({ "type": "image", "source": { "type": "base64", "media_type": i.mime, "data": i.data } })).collect();
    if !text.is_empty() {
        c.push(json!({ "type": "text", "text": text }));
    }
    c
}

#[derive(Debug, Clone, Default)]
pub struct SpawnOpts {
    pub bin: PathBuf,
    pub claude_dir: PathBuf,
    pub cwd: PathBuf,
    pub model: String,
    pub permission_mode: String,
    pub resume: Option<String>,
    /// low | medium | high | xhigh | max (`None` = padrão do modelo)
    pub effort: Option<String>,
    /// agente principal da sessão (`--agent`)
    pub agent: Option<String>,
    /// pastas extras com acesso (`--add-dir`)
    pub add_dirs: Vec<String>,
    /// `--dangerously-skip-permissions`: nenhuma ferramenta pede permissão
    pub skip_permissions: bool,
}

#[derive(Debug)]
pub enum AgentOut {
    Message(Value),
    Exit { code: Option<i32>, stderr: String },
}

pub type Sink = Arc<dyn Fn(&str, AgentOut) + Send + Sync>;

struct Proc {
    stdin: Mutex<ChildStdin>,
    child: Mutex<Child>,
}

type ProcMap = Arc<parking_lot::Mutex<HashMap<String, Arc<Proc>>>>;
type Pending = Arc<parking_lot::Mutex<HashMap<String, tokio::sync::oneshot::Sender<Value>>>>;

#[derive(Default)]
pub struct AgentManager {
    procs: ProcMap,
    /// pedidos de controle aguardando `control_response` (request_id → resposta)
    pending: Pending,
}

pub fn build_args(o: &SpawnOpts) -> Vec<String> {
    let mut a: Vec<String> = [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--permission-prompt-tool",
        "stdio",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if !o.model.is_empty() && o.model != "default" {
        a.push("--model".into());
        a.push(o.model.clone());
    }
    if !o.permission_mode.is_empty() && o.permission_mode != "default" {
        a.push("--permission-mode".into());
        a.push(o.permission_mode.clone());
    }
    if let Some(e) = o.effort.as_deref().filter(|e| !e.is_empty()) {
        a.push("--effort".into());
        a.push(e.to_string());
    }
    if let Some(ag) = o.agent.as_deref().filter(|g| !g.is_empty()) {
        a.push("--agent".into());
        a.push(ag.to_string());
    }
    for d in &o.add_dirs {
        a.push("--add-dir".into());
        a.push(d.clone());
    }
    if o.skip_permissions {
        a.push("--dangerously-skip-permissions".into());
    }
    if let Some(r) = &o.resume {
        a.push("--resume".into());
        a.push(r.clone());
    }
    a
}

fn rid() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
}

impl AgentManager {
    pub fn is_running(&self, thread_id: &str) -> bool {
        self.procs.lock().contains_key(thread_id)
    }

    pub async fn ensure(&self, thread_id: &str, opts: SpawnOpts, sink: Sink) -> Result<()> {
        if self.is_running(thread_id) {
            return Ok(());
        }
        let mut cmd = lazyaicode_platform::command(&opts.bin);
        cmd.args(build_args(&opts)).current_dir(&opts.cwd);
        crate::claude::apply_env(&mut cmd, &opts.claude_dir);
        let mut cmd = Command::from(cmd);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| anyhow!("Não foi possível iniciar o Claude CLI ({}): {e}", opts.bin.display()))?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("sem stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("sem stdout"))?;
        let mut stderr = child.stderr.take().ok_or_else(|| anyhow!("sem stderr"))?;

        let proc = Arc::new(Proc { stdin: Mutex::new(stdin), child: Mutex::new(child) });
        self.procs.lock().insert(thread_id.to_string(), proc.clone());

        let err_buf = Arc::new(parking_lot::Mutex::new(String::new()));
        let eb = err_buf.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            while let Ok(n) = stderr.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                let mut s = eb.lock();
                s.push_str(&String::from_utf8_lossy(&buf[..n]));
                if s.len() > 8000 {
                    let cut = s.len() - 8000;
                    let cut = (cut..s.len()).find(|i| s.is_char_boundary(*i)).unwrap_or(0);
                    s.drain(..cut);
                }
            }
        });

        let tid = thread_id.to_string();
        let procs = self.procs.clone();
        let pending = self.pending.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<Value>(&line) {
                    Ok(v) => {
                        if v["type"] == "control_response" {
                            let rid = v["response"]["request_id"].as_str().unwrap_or_default().to_string();
                            if let Some(tx) = pending.lock().remove(&rid) {
                                let _ = tx.send(v["response"].clone());
                                continue;
                            }
                        }
                        sink(&tid, AgentOut::Message(v))
                    }
                    Err(_) => sink(&tid, AgentOut::Message(json!({"type":"raw","text":line}))),
                }
            }
            let code = {
                let mut c = proc.child.lock().await;
                c.wait().await.ok().and_then(|s| s.code())
            };
            {
                let mut map = procs.lock();
                if map.get(&tid).map(|p| Arc::ptr_eq(p, &proc)).unwrap_or(false) {
                    map.remove(&tid);
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let stderr = err_buf.lock().clone();
            sink(&tid, AgentOut::Exit { code, stderr });
        });

        self.write(
            thread_id,
            &json!({
                "type": "control_request",
                "request_id": rid(),
                "request": { "subtype": "initialize" }
            }),
        )
        .await
    }

    async fn write(&self, thread_id: &str, v: &Value) -> Result<()> {
        let proc = self.procs.lock().get(thread_id).cloned().ok_or_else(|| anyhow!("O agente desta thread não está rodando"))?;
        let mut line = serde_json::to_string(v)?;
        line.push('\n');
        let mut stdin = proc.stdin.lock().await;
        stdin.write_all(line.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    pub async fn send_user(&self, thread_id: &str, text: &str, images: &[ImageInput]) -> Result<()> {
        self.write(
            thread_id,
            &json!({
                "type": "user",
                "message": { "role": "user", "content": user_content(text, images) },
                "parent_tool_use_id": null,
                "session_id": ""
            }),
        )
        .await
    }

    pub async fn control(&self, thread_id: &str, request: Value) -> Result<()> {
        self.write(
            thread_id,
            &json!({
                "type": "control_request",
                "request_id": rid(),
                "request": request
            }),
        )
        .await
    }

    /// Pedido de controle que espera a resposta do CLI (ex.: `mcp_status`).
    pub async fn request(&self, thread_id: &str, request: Value) -> Result<Value> {
        let id = rid();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending.lock().insert(id.clone(), tx);
        let sent = self.write(thread_id, &json!({ "type": "control_request", "request_id": id, "request": request })).await;
        if let Err(e) = sent {
            self.pending.lock().remove(&id);
            return Err(e);
        }
        let resp = tokio::time::timeout(std::time::Duration::from_secs(20), rx)
            .await
            .map_err(|_| {
                self.pending.lock().remove(&id);
                anyhow!("O agente não respondeu a tempo")
            })?
            .map_err(|_| anyhow!("O agente encerrou antes de responder"))?;
        if resp["subtype"] == "error" {
            return Err(anyhow!("{}", resp["error"].as_str().unwrap_or("erro do CLI")));
        }
        Ok(resp["response"].clone())
    }

    pub async fn respond_permission(
        &self,
        thread_id: &str,
        request_id: &str,
        allow: bool,
        input: Value,
        message: Option<String>,
        tool_use_id: Option<String>,
        updated_permissions: Option<Vec<Value>>,
    ) -> Result<()> {
        let response = if allow {
            let mut r = json!({ "behavior": "allow", "updatedInput": input, "toolUseID": tool_use_id });
            // "sempre permitir": o CLI grava as regras (sessão ou settings) e não pergunta de novo
            if let Some(p) = updated_permissions.filter(|p| !p.is_empty()) {
                r["updatedPermissions"] = json!(p);
            }
            r
        } else {
            json!({
                "behavior": "deny",
                "message": message.unwrap_or_else(|| "O usuário negou esta ação.".into()),
                "toolUseID": tool_use_id
            })
        };
        self.write(
            thread_id,
            &json!({
                "type": "control_response",
                "response": { "subtype": "success", "request_id": request_id, "response": response }
            }),
        )
        .await
    }

    pub async fn stop(&self, thread_id: &str) {
        let proc = self.procs.lock().remove(thread_id);
        if let Some(p) = proc {
            let _ = p.child.lock().await.start_kill();
        }
    }
}

/// Abre um `claude` só para perguntar o que ele carregou nesta pasta —
/// comandos/skills, agentes, modelos, conta — e o status dos MCPs, e fecha.
/// Nenhuma mensagem é enviada, então nada é cobrado.
pub async fn probe(bin: &std::path::Path, claude_dir: &std::path::Path, cwd: &std::path::Path) -> Result<Value> {
    let mut cmd = lazyaicode_platform::command(bin);
    cmd.args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--permission-prompt-tool", "stdio"])
        .current_dir(cwd);
    crate::claude::apply_env(&mut cmd, claude_dir);
    let mut cmd = Command::from(cmd);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|e| anyhow!("Não foi possível iniciar o Claude CLI: {e}"))?;
    let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("sem stdin"))?;
    let mut lines = BufReader::new(child.stdout.take().ok_or_else(|| anyhow!("sem stdout"))?).lines();

    async fn ask(
        stdin: &mut ChildStdin,
        lines: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
        id: &str,
        req: Value,
    ) -> Result<Value> {
        let line = json!({ "type": "control_request", "request_id": id, "request": req }).to_string() + "\n";
        stdin.write_all(line.as_bytes()).await?;
        stdin.flush().await?;
        while let Some(l) = lines.next_line().await? {
            let Ok(v) = serde_json::from_str::<Value>(&l) else { continue };
            if v["type"] == "control_response" && v["response"]["request_id"] == id {
                if v["response"]["subtype"] == "error" {
                    return Err(anyhow!("{}", v["response"]["error"].as_str().unwrap_or("erro do CLI")));
                }
                return Ok(v["response"]["response"].clone());
            }
        }
        Err(anyhow!("O Claude CLI encerrou sem responder"))
    }

    let work = async {
        let init = ask(&mut stdin, &mut lines, "init", json!({ "subtype": "initialize" })).await?;
        // servidores MCP conectam em segundo plano: espera sair de "pending"
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut mcp = Value::Null;
        for n in 0.. {
            mcp = ask(&mut stdin, &mut lines, &format!("mcp{n}"), json!({ "subtype": "mcp_status" })).await?;
            let pending = mcp["mcpServers"].as_array().is_some_and(|a| a.iter().any(|s| s["status"] == "pending"));
            if !pending || std::time::Instant::now() > deadline {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        }
        Ok::<_, anyhow::Error>(json!({ "init": init, "mcp": mcp["mcpServers"] }))
    };
    let out = tokio::time::timeout(std::time::Duration::from_secs(40), work)
        .await
        .map_err(|_| anyhow!("O Claude CLI demorou demais para responder"))?;
    let _ = child.start_kill();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn user_content_puts_images_before_text() {
        let img = ImageInput { mime: "image/png".into(), data: "AAAA".into() };
        let c = user_content("o que é isso?", &[img.clone()]);
        assert_eq!(c[0], json!({ "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "AAAA" } }));
        assert_eq!(c[1], json!({ "type": "text", "text": "o que é isso?" }));
        // só imagem: sem bloco de texto vazio
        assert_eq!(user_content("", &[img]).len(), 1);
    }

    #[test]
    fn session_options_become_cli_flags() {
        let o = SpawnOpts {
            effort: Some("high".into()),
            agent: Some("revisor".into()),
            add_dirs: vec!["/a".into(), "/b".into()],
            ..Default::default()
        };
        let a = build_args(&o);
        assert!(a.windows(2).any(|w| w == ["--effort", "high"]));
        assert!(a.windows(2).any(|w| w == ["--agent", "revisor"]));
        assert_eq!(a.iter().filter(|x| *x == "--add-dir").count(), 2);
        let none = build_args(&SpawnOpts { effort: Some(String::new()), ..Default::default() });
        assert!(!none.contains(&"--effort".to_string()));
        assert!(!none.contains(&"--dangerously-skip-permissions".to_string()));
    }

    #[test]
    fn skip_permissions_survives_resume() {
        let a = build_args(&SpawnOpts { skip_permissions: true, resume: Some("s1".into()), ..Default::default() });
        assert!(a.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(a.windows(2).any(|w| w == ["--resume", "s1"]));
    }

    #[test]
    fn default_model_and_mode_are_left_to_the_users_settings() {
        let o = SpawnOpts {
            bin: "claude".into(),
            claude_dir: "/x".into(),
            cwd: "/".into(),
            model: "default".into(),
            permission_mode: "default".into(),
            resume: Some("abc".into()),
            ..Default::default()
        };
        let a = build_args(&o);
        assert!(!a.contains(&"--model".to_string()));
        assert!(!a.contains(&"--permission-mode".to_string()));
        assert!(a.windows(2).any(|w| w == ["--resume", "abc"]));
        assert!(a.windows(2).any(|w| w == ["--permission-prompt-tool", "stdio"]));
    }

    /// Um "claude" falso: ecoa cada linha do stdin com o tipo trocado, e sai
    /// quando o stdin fecha. Exercita o encanamento sem rede.
    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    async fn streams_lines_both_ways_and_reports_exit() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("fake-claude");
        std::fs::write(
            &bin,
            "#!/bin/sh\necho '{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s1\"}'\n\
             while read -r l; do echo \"{\\\"type\\\":\\\"echo\\\",\\\"len\\\":${#l}}\"; done\n\
             echo erro-final >&2\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let (tx, rx) = mpsc::channel::<String>();
        let tx = parking_lot::Mutex::new(tx);
        let sink: Sink = Arc::new(move |_tid, out| {
            let s = match out {
                AgentOut::Message(v) => v["type"].as_str().unwrap_or("?").to_string(),
                AgentOut::Exit { stderr, .. } => format!("exit:{}", stderr.trim()),
            };
            let _ = tx.lock().send(s);
        });
        let mgr = AgentManager::default();
        mgr.ensure(
            "t1",
            SpawnOpts {
                bin,
                claude_dir: dir.path().into(),
                cwd: dir.path().into(),
                model: "default".into(),
                permission_mode: "".into(),
                resume: None,
                ..Default::default()
            },
            sink,
        )
        .await
        .unwrap();
        mgr.send_user("t1", "olá", &[]).await.unwrap();
        let recv = || rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(recv(), "system");
        assert_eq!(recv(), "echo"); // initialize
        assert_eq!(recv(), "echo"); // mensagem do usuário
        mgr.stop("t1").await;
        let last = recv();
        assert!(last.starts_with("exit:"), "{last}");
        assert!(!mgr.is_running("t1"));
    }
}

/// Ponta a ponta com o Claude CLI de verdade e as credenciais locais.
/// `cargo test -- --ignored real_claude` (consome alguns tokens do Haiku).
#[cfg(test)]
mod e2e {
    use super::*;
    use std::sync::mpsc;

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_claude_asks_permission_edits_the_isolated_checkout_and_shows_in_the_diff() {
        let bin = crate::claude::find_claude_binary().expect("claude no PATH");
        let claude_dir = crate::claude::resolve_active(None);
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let g = |a: &[&str]| lazyaicode_git::thread::git(&repo, a).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.email", "t@t"]);
        g(&["config", "user.name", "t"]);
        std::fs::write(repo.join("README.md"), "teste\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "init"]);
        let iso = lazyaicode_git::thread::create_isolated(&repo, &tmp.path().join("wt"), "e2e", false).unwrap();

        let (tx, rx) = mpsc::channel::<Value>();
        let tx = parking_lot::Mutex::new(tx);
        let sink: Sink = Arc::new(move |_t, out| {
            if let AgentOut::Message(v) = out {
                let _ = tx.lock().send(v);
            }
        });
        let mgr = AgentManager::default();
        mgr.ensure(
            "e2e",
            SpawnOpts {
                bin,
                claude_dir,
                cwd: iso.path.clone(),
                model: "haiku".into(),
                permission_mode: "manual".into(),
                resume: None,
                ..Default::default()
            },
            sink,
        )
        .await
        .unwrap();
        mgr.send_user("e2e", "Use the Write tool to create a file named ola.txt containing exactly: oi. Do nothing else.", &[])
            .await
            .unwrap();

        let (mut asked, mut session) = (false, None);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            let v = rx.recv_timeout(left).expect("sem resposta do claude");
            if v["type"] == "system" && v["subtype"] == "init" {
                session = v["session_id"].as_str().map(str::to_string);
            }
            if v["type"] == "control_request" && v["request"]["subtype"] == "can_use_tool" {
                asked = true;
                mgr.respond_permission(
                    "e2e",
                    v["request_id"].as_str().unwrap(),
                    true,
                    v["request"]["input"].clone(),
                    None,
                    v["request"]["tool_use_id"].as_str().map(str::to_string),
                    None,
                )
                .await
                .unwrap();
            }
            if v["type"] == "result" {
                break;
            }
        }
        mgr.stop("e2e").await;
        assert!(session.is_some(), "sem session_id no init");
        assert!(asked, "o CLI não pediu permissão em modo manual");
        assert_eq!(std::fs::read_to_string(iso.path.join("ola.txt")).unwrap().trim(), "oi");
        assert!(!repo.join("ola.txt").exists(), "a edição vazou para o checkout principal");
        let (base, _) = lazyaicode_git::thread::resolve_base(&iso.path, "upstream", Some("main"), None).unwrap();
        let ch = lazyaicode_git::thread::changes(&iso.path, &base, "main").unwrap();
        assert_eq!(ch.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), vec!["ola.txt"]);
        // o histórico nativo da sessão é legível pelo app
        let file =
            crate::claude::session_file(&crate::claude::resolve_active(None), &iso.path.to_string_lossy(), session.as_deref().unwrap());
        assert!(!crate::transcript::load(&file).is_empty(), "histórico vazio em {}", file.display());
    }

    /// `cargo test -- --ignored real_probe --nocapture`
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_probe_lists_skills_agents_and_mcp_without_secrets() {
        let bin = crate::claude::find_claude_binary().unwrap();
        let dir = crate::claude::resolve_active(None);
        let cwd = std::env::var("PROBE_CWD").map(PathBuf::from).unwrap_or_else(|_| std::env::current_dir().unwrap());
        let t = std::time::Instant::now();
        let v = probe(&bin, &dir, &cwd).await.unwrap();
        let (cmds, agents) = crate::catalog::merge(&v["init"], crate::catalog::scan(&dir, Some(&cwd)));
        let mcp = crate::catalog::redact_mcp(&v["mcp"]);
        println!(
            "sondagem em {:?}: {} comandos/skills, {} agentes, {} mcp",
            t.elapsed(),
            cmds.len(),
            agents.len(),
            mcp.as_array().unwrap().len()
        );
        for c in cmds.iter().filter(|c| !c.builtin).take(4) {
            println!("  skill {} [{}] loaded={} path={}", c.entry.name, c.entry.scope, c.loaded, c.entry.path);
        }
        for a in agents.iter().take(4) {
            println!("  agente {} [{}] path={}", a.entry.name, a.entry.scope, a.entry.path);
        }
        for m in mcp.as_array().unwrap() {
            println!("  mcp {} {} {} ferramentas", m["name"], m["status"], m["tools"].as_array().unwrap().len());
        }
        assert!(!mcp.to_string().contains("Bearer"));
        assert!(!cmds.is_empty() && !agents.is_empty());
    }
}

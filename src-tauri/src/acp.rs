//! Cliente ACP (Agent Client Protocol, JSON-RPC por stdio) para os agentes
//! que não são o Claude Code: Codex (OpenAI, via `codex-acp`) e Gemini CLI
//! (Google). Cada um usa o próprio login da máquina (`~/.codex`, `~/.gemini`)
//! ou a chave do ambiente (`OPENAI_API_KEY`, `GEMINI_API_KEY`) — o app não
//! guarda chave.
//!
//! Tudo o que o agente manda passa pelo [`Translator`] e sai no mesmo formato
//! do stream-json do Claude, pelo mesmo `Sink`.

use crate::acp_translate::Translator;
use crate::agent::{AgentOut, ImageInput, Sink};
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};

pub const CODEX_ACP: &str = "@zed-industries/codex-acp@0.16.0";
pub const GEMINI_CLI: &str = "@google/gemini-cli@0.64.0";
pub const CLAUDE_ACP: &str = "@agentclientprotocol/claude-agent-acp@0.90.0";

/// Provedores além do Claude Code nativo.
pub const PROVIDERS: &[&str] = &["codex", "gemini"];

fn in_path(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|d| names.iter().map(|n| d.join(n)).find(|p| p.is_file()))
}

fn npx() -> Option<PathBuf> {
    if cfg!(windows) { in_path(&["npx.cmd", "npx.exe"]) } else { in_path(&["npx"]) }
}

/// Como iniciar o agente ACP de um provedor: variável de ambiente, binário
/// instalado ou, por fim, `npx` com versão fixa (baixa na primeira vez).
pub fn command_for(provider: &str) -> Result<(PathBuf, Vec<String>, &'static str)> {
    let exe = |n: &str| if cfg!(windows) { vec![format!("{n}.cmd"), format!("{n}.exe"), n.to_string()] } else { vec![n.to_string()] };
    let (env, bins, args, pkg): (&str, Vec<String>, Vec<&str>, &str) = match provider {
        "codex" => ("LAZYAICODE_CODEX_ACP", exe("codex-acp"), vec![], CODEX_ACP),
        "gemini" => ("LAZYAICODE_GEMINI", exe("gemini"), vec!["--experimental-acp"], GEMINI_CLI),
        "claude-acp" => ("LAZYAICODE_CLAUDE_ACP", exe("claude-agent-acp"), vec![], CLAUDE_ACP),
        p => bail!("Provedor desconhecido: {p}"),
    };
    let args: Vec<String> = args.into_iter().map(String::from).collect();
    if let Ok(p) = std::env::var(env) {
        return Ok((PathBuf::from(p), args, "env"));
    }
    let names: Vec<&str> = bins.iter().map(String::as_str).collect();
    if let Some(p) = in_path(&names) {
        return Ok((p, args, "installed"));
    }
    let npx = npx().ok_or_else(|| anyhow!("Node.js (npx) não encontrado — necessário para rodar o agente {provider}"))?;
    let mut a = vec!["-y".to_string(), pkg.to_string()];
    a.extend(args);
    Ok((npx, a, "npx"))
}

#[derive(Debug, Clone, Default)]
pub struct AcpOpts {
    pub provider: String,
    pub cwd: PathBuf,
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub mode: Option<String>,
}

struct Conn {
    stdin: Mutex<ChildStdin>,
    child: Mutex<Child>,
    next_id: AtomicU64,
    pending: parking_lot::Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>,
    session_id: parking_lot::Mutex<Option<String>>,
    /// pedidos de permissão em aberto: id → (id JSON-RPC, opções)
    permissions: parking_lot::Mutex<HashMap<String, (Value, Vec<Value>)>>,
    translator: parking_lot::Mutex<Translator>,
    /// durante `session/load` o agente repete o histórico: não reemitir
    replaying: AtomicBool,
    /// opções de configuração da sessão (modo/modelo) vindas do agente
    config: parking_lot::Mutex<Value>,
    /// o agente aceita blocos de imagem no prompt (`promptCapabilities.image`)
    images: AtomicBool,
}

impl Conn {
    async fn write(&self, v: &Value) -> Result<()> {
        let mut line = serde_json::to_string(v)?;
        line.push('\n');
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(line.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    async fn call(&self, method: &str, params: Value, timeout: Option<std::time::Duration>) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().insert(id, tx);
        self.write(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).await?;
        let res = match timeout {
            Some(t) => tokio::time::timeout(t, rx).await.map_err(|_| {
                self.pending.lock().remove(&id);
                anyhow!("{method}: o agente não respondeu a tempo")
            })?,
            None => rx.await,
        };
        res.map_err(|_| anyhow!("O agente encerrou antes de responder a {method}"))?.map_err(|e| anyhow!(e))
    }

    async fn notify(&self, method: &str, params: Value) -> Result<()> {
        self.write(&json!({ "jsonrpc": "2.0", "method": method, "params": params })).await
    }

    fn sid(&self) -> Result<String> {
        self.session_id.lock().clone().ok_or_else(|| anyhow!("Sessão ACP ainda não aberta"))
    }
}

fn rpc_error(v: &Value) -> String {
    let msg = v["message"].as_str().unwrap_or("erro do agente").to_string();
    match v["data"].as_str().or_else(|| v.pointer("/data/details").and_then(Value::as_str)) {
        Some(d) if !d.is_empty() && !msg.contains(d) => format!("{msg}: {d}"),
        _ => msg,
    }
}

/// Opções de uma categoria (modo ou modelo), juntando o formato novo
/// (`configOptions`) e o antigo (`modes` / `models`).
pub fn options_of(session: &Value, category: &str) -> (Option<String>, Vec<Value>) {
    for c in session["configOptions"].as_array().into_iter().flatten() {
        if c["category"] == category || c["id"] == category {
            let opts = c["options"].as_array().cloned().unwrap_or_default()
                .into_iter().map(|o| json!({ "id": o["value"], "name": o["name"], "description": o["description"] })).collect();
            return (c["currentValue"].as_str().map(str::to_string), opts);
        }
    }
    let (key, list, cur, idk) = if category == "mode" { ("modes", "availableModes", "currentModeId", "id") } else { ("models", "availableModels", "currentModelId", "modelId") };
    let opts = session[key][list].as_array().cloned().unwrap_or_default()
        .into_iter().map(|o| json!({ "id": o[idk], "name": o["name"], "description": o["description"] })).collect();
    (session[key][cur].as_str().map(str::to_string), opts)
}

#[derive(Default)]
pub struct AcpManager {
    conns: Arc<parking_lot::Mutex<HashMap<String, Arc<Conn>>>>,
}

impl AcpManager {
    pub fn is_running(&self, thread_id: &str) -> bool {
        self.conns.lock().contains_key(thread_id)
    }

    fn conn(&self, thread_id: &str) -> Result<Arc<Conn>> {
        self.conns.lock().get(thread_id).cloned().ok_or_else(|| anyhow!("O agente desta thread não está rodando"))
    }

    async fn spawn(&self, thread_id: &str, provider: &str, cwd: &Path, sink: Sink) -> Result<Arc<Conn>> {
        let (bin, args, _) = command_for(provider)?;
        let mut cmd = crate::platform::command(&bin);
        cmd.args(&args).current_dir(cwd).env_remove("CLAUDECODE");
        let mut cmd = Command::from(cmd);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| anyhow!("Não foi possível iniciar o agente {provider} ({}): {e}", bin.display()))?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("sem stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("sem stdout"))?;
        let stderr = child.stderr.take().ok_or_else(|| anyhow!("sem stderr"))?;
        let conn = Arc::new(Conn {
            stdin: Mutex::new(stdin),
            child: Mutex::new(child),
            next_id: AtomicU64::new(1),
            pending: parking_lot::Mutex::new(HashMap::new()),
            session_id: parking_lot::Mutex::new(None),
            permissions: parking_lot::Mutex::new(HashMap::new()),
            translator: parking_lot::Mutex::new(Translator::default()),
            replaying: AtomicBool::new(false),
            config: parking_lot::Mutex::new(Value::Null),
            images: AtomicBool::new(false),
        });
        self.conns.lock().insert(thread_id.to_string(), conn.clone());

        let err_buf = Arc::new(parking_lot::Mutex::new(String::new()));
        let eb = err_buf.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                let mut s = eb.lock();
                s.push_str(&l);
                s.push('\n');
                if s.len() > 8000 {
                    let cut = s.len() - 8000;
                    let cut = (cut..s.len()).find(|i| s.is_char_boundary(*i)).unwrap_or(0);
                    s.drain(..cut);
                }
            }
        });

        let (tid, c, conns) = (thread_id.to_string(), conn.clone(), self.conns.clone());
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                let method = v["method"].as_str().map(str::to_string);
                match (method, v.get("id").cloned()) {
                    // resposta a uma chamada nossa
                    (None, Some(id)) => {
                        if let Some(tx) = id.as_u64().and_then(|i| c.pending.lock().remove(&i)) {
                            let _ = tx.send(match v.get("error") {
                                Some(e) => Err(rpc_error(e)),
                                None => Ok(v["result"].clone()),
                            });
                        }
                    }
                    // pedido do agente
                    (Some(m), Some(id)) if m == "session/request_permission" => {
                        let p = &v["params"];
                        let key = format!("acp-{}", id.to_string().trim_matches('"'));
                        c.permissions.lock().insert(key.clone(), (id, p["options"].as_array().cloned().unwrap_or_default()));
                        let tc = &p["toolCall"];
                        let name = tc.pointer("/_meta/claudeCode/toolName").and_then(Value::as_str)
                            .or_else(|| tc["name"].as_str()).or_else(|| tc["kind"].as_str()).unwrap_or("ferramenta");
                        let mut input = tc["rawInput"].as_object().cloned().unwrap_or_default();
                        if let Some(t) = tc["title"].as_str() {
                            input.insert("title".into(), json!(t));
                        }
                        sink(&tid, AgentOut::Message(json!({
                            "type": "control_request", "request_id": key,
                            "request": { "subtype": "can_use_tool", "tool_name": name, "input": input,
                                         "tool_use_id": tc["toolCallId"], "options": p["options"] }
                        })));
                    }
                    // fs/terminal: não anunciamos suporte; recusa educada
                    (Some(m), Some(id)) => {
                        let _ = c.write(&json!({ "jsonrpc": "2.0", "id": id,
                            "error": { "code": -32601, "message": format!("método não suportado: {m}") } })).await;
                    }
                    (Some(m), None) if m == "session/update" => {
                        if c.replaying.load(Ordering::SeqCst) {
                            continue;
                        }
                        let out = c.translator.lock().on_update(&v["params"]["update"]);
                        for msg in out {
                            if msg["type"] == "lazy_config" {
                                let mut cfg = c.config.lock();
                                if let Some(opts) = msg.get("configOptions").filter(|o| !o.is_null()) {
                                    cfg["configOptions"] = opts.clone();
                                }
                            }
                            sink(&tid, AgentOut::Message(msg));
                        }
                    }
                    _ => {}
                }
            }
            let code = { c.child.lock().await.wait().await.ok().and_then(|s| s.code()) };
            for (_, tx) in c.pending.lock().drain() {
                let _ = tx.send(Err("o agente encerrou".into()));
            }
            {
                let mut map = conns.lock();
                if map.get(&tid).is_some_and(|x| Arc::ptr_eq(x, &c)) {
                    map.remove(&tid);
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let stderr = err_buf.lock().clone();
            sink(&tid, AgentOut::Exit { code, stderr });
        });
        Ok(conn)
    }

    /// Abre (ou retoma) a sessão da thread. Emite um `system init` sintético
    /// com o id da sessão e as opções de modo/modelo do agente.
    pub async fn ensure(&self, thread_id: &str, opts: AcpOpts, sink: Sink) -> Result<()> {
        if self.is_running(thread_id) {
            return Ok(());
        }
        let conn = self.spawn(thread_id, &opts.provider, &opts.cwd, sink.clone()).await?;
        let short = Some(std::time::Duration::from_secs(120));
        let init = conn
            .call("initialize", json!({ "protocolVersion": 1,
                "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false }, "terminal": false },
                "clientInfo": { "name": "lazyaicode", "version": env!("CARGO_PKG_VERSION") } }), short)
            .await;
        let init = match init {
            Ok(i) => i,
            Err(e) => {
                self.stop(thread_id).await;
                return Err(e);
            }
        };
        let accepts_images = init.pointer("/agentCapabilities/promptCapabilities/image").and_then(Value::as_bool);
        conn.images.store(accepts_images.unwrap_or(false), Ordering::SeqCst);
        let cwd = opts.cwd.to_string_lossy().to_string();
        let mut notice = None;
        let session = match opts.session_id.as_deref() {
            Some(sid) if init.pointer("/agentCapabilities/loadSession").and_then(Value::as_bool).unwrap_or(false) => {
                conn.replaying.store(true, Ordering::SeqCst);
                let r = conn.call("session/load", json!({ "sessionId": sid, "cwd": cwd, "mcpServers": [] }), short).await;
                conn.replaying.store(false, Ordering::SeqCst);
                match r {
                    Ok(mut v) => {
                        v["sessionId"] = json!(sid);
                        Ok(v)
                    }
                    Err(e) => {
                        notice = Some(format!("Não foi possível retomar a sessão anterior ({e}); uma sessão nova foi aberta — o agente não lembra da conversa."));
                        conn.call("session/new", json!({ "cwd": cwd, "mcpServers": [] }), short).await
                    }
                }
            }
            _ => conn.call("session/new", json!({ "cwd": cwd, "mcpServers": [] }), short).await,
        };
        let session = match session {
            Ok(s) => s,
            Err(e) => {
                self.stop(thread_id).await;
                return Err(e);
            }
        };
        let sid = session["sessionId"].as_str().ok_or_else(|| anyhow!("O agente não devolveu sessionId"))?.to_string();
        *conn.session_id.lock() = Some(sid.clone());
        *conn.config.lock() = session.clone();
        let (cur_mode, modes) = options_of(&session, "mode");
        let (cur_model, models) = options_of(&session, "model");
        sink(thread_id, AgentOut::Message(json!({
            "type": "system", "subtype": "init", "session_id": sid, "provider": opts.provider,
            "agent": init["agentInfo"],
            "acp": { "modes": modes, "currentMode": cur_mode, "models": models, "currentModel": cur_model }
        })));
        if let Some(n) = notice {
            sink(thread_id, AgentOut::Message(json!({ "type": "lazy_notice", "text": n })));
        }
        // vazio = padrão do agente. "default" NÃO é sentinela aqui: é um id
        // real de modo em alguns agentes (o "Manual" do Claude, por exemplo)
        if let Some(m) = opts.model.filter(|m| !m.is_empty()) {
            if let Err(e) = self.set_option(thread_id, "model", &m).await {
                sink(thread_id, AgentOut::Message(json!({ "type": "lazy_notice", "text": format!("Não foi possível usar o modelo {m}: {e}") })));
            }
        }
        if let Some(m) = opts.mode.filter(|m| !m.is_empty()) {
            if let Err(e) = self.set_option(thread_id, "mode", &m).await {
                sink(thread_id, AgentOut::Message(json!({ "type": "lazy_notice", "text": format!("Não foi possível usar o modo {m}: {e}") })));
            }
        }
        Ok(())
    }

    /// Envia o prompt e devolve logo; o fim do turno chega como `result`.
    pub async fn prompt(&self, thread_id: &str, text: &str, images: &[ImageInput], sink: Sink) -> Result<()> {
        let conn = self.conn(thread_id)?;
        let sid = conn.sid()?;
        if !images.is_empty() && !conn.images.load(Ordering::SeqCst) {
            bail!("Este agente não aceita imagens; envie só o texto.");
        }
        let mut prompt: Vec<Value> = images
            .iter()
            .map(|i| json!({ "type": "image", "mimeType": i.mime, "data": i.data }))
            .collect();
        if !text.is_empty() {
            prompt.push(json!({ "type": "text", "text": text }));
        }
        conn.translator.lock().begin_turn();
        let tid = thread_id.to_string();
        tokio::spawn(async move {
            let started = std::time::Instant::now();
            let r = conn.call("session/prompt", json!({ "sessionId": sid, "prompt": prompt }), None).await;
            let ms = started.elapsed().as_millis() as u64;
            let out = match &r {
                Ok(v) => conn.translator.lock().end_turn(Ok(v), ms),
                Err(e) => conn.translator.lock().end_turn(Err(e.to_string()), ms),
            };
            for m in out {
                sink(&tid, AgentOut::Message(m));
            }
        });
        Ok(())
    }

    pub async fn cancel(&self, thread_id: &str) -> Result<()> {
        let conn = self.conn(thread_id)?;
        let sid = conn.sid()?;
        // permissões pendentes são canceladas junto, como pede o protocolo
        let pending: Vec<(Value, Vec<Value>)> = conn.permissions.lock().drain().map(|(_, v)| v).collect();
        for (id, _) in pending {
            let _ = conn.write(&json!({ "jsonrpc": "2.0", "id": id, "result": { "outcome": { "outcome": "cancelled" } } })).await;
        }
        conn.notify("session/cancel", json!({ "sessionId": sid })).await
    }

    pub async fn respond_permission(&self, thread_id: &str, request_id: &str, allow: bool, always: bool) -> Result<()> {
        let conn = self.conn(thread_id)?;
        let (id, options) = conn.permissions.lock().remove(request_id).ok_or_else(|| anyhow!("Pedido de permissão não encontrado (já respondido?)"))?;
        let kinds: &[&str] = match (allow, always) {
            (true, true) => &["allow_always", "allow_once"],
            (true, false) => &["allow_once", "allow_always"],
            _ => &["reject_once", "reject_always"],
        };
        let chosen = kinds
            .iter()
            .find_map(|k| options.iter().find(|o| o["kind"] == *k))
            .and_then(|o| o["optionId"].as_str())
            .map(str::to_string);
        let outcome = match chosen {
            Some(opt) => json!({ "outcome": "selected", "optionId": opt }),
            None => json!({ "outcome": "cancelled" }),
        };
        conn.write(&json!({ "jsonrpc": "2.0", "id": id, "result": { "outcome": outcome } })).await
    }

    /// Troca modo ou modelo da sessão aberta: `session/set_config_option`
    /// quando o agente usa configOptions, senão os métodos antigos.
    pub async fn set_option(&self, thread_id: &str, category: &str, value: &str) -> Result<()> {
        let conn = self.conn(thread_id)?;
        let sid = conn.sid()?;
        let cfg = conn.config.lock().clone();
        let config_id = cfg["configOptions"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["category"] == category || c["id"] == category)
            .and_then(|c| c["id"].as_str())
            .map(str::to_string);
        let t = Some(std::time::Duration::from_secs(30));
        // modo: `session/set_mode` é o que todos os agentes aplicam de fato
        // (no adaptador do Claude o set_config_option só muda o valor exibido)
        if category == "mode" {
            let legacy = conn.call("session/set_mode", json!({ "sessionId": sid, "modeId": value }), t).await;
            if legacy.is_ok() || config_id.is_none() {
                return legacy.map(|_| ());
            }
        }
        match config_id {
            Some(id) => conn.call("session/set_config_option", json!({ "sessionId": sid, "configId": id, "value": value }), t).await.map(|_| ()),
            None if category == "mode" => conn.call("session/set_mode", json!({ "sessionId": sid, "modeId": value }), t).await.map(|_| ()),
            None => conn.call("session/set_model", json!({ "sessionId": sid, "modelId": value }), t).await.map(|_| ()),
        }
    }

    pub async fn stop(&self, thread_id: &str) {
        let conn = self.conns.lock().remove(thread_id);
        if let Some(c) = conn {
            let _ = c.child.lock().await.start_kill();
        }
    }
}

/// Situação de um provedor na máquina: como seria iniciado, se responde e se
/// já tem login (uma sessão abre). Não envia prompt — não custa nada.
pub async fn status(provider: &str, cwd: &Path) -> Value {
    let launch = match command_for(provider) {
        Ok((bin, args, how)) => json!({ "bin": bin, "args": args, "how": how }),
        Err(e) => return json!({ "provider": provider, "ok": false, "available": false, "error": e.to_string() }),
    };
    let mgr = AcpManager::default();
    let tid = format!("status-{provider}-{}", uuid::Uuid::new_v4().simple());
    let captured: Arc<parking_lot::Mutex<Option<Value>>> = Arc::new(parking_lot::Mutex::new(None));
    let cap = captured.clone();
    let sink: Sink = Arc::new(move |_t, out| {
        if let AgentOut::Message(m) = out {
            if m["type"] == "system" {
                *cap.lock() = Some(m);
            }
        }
    });
    // a inicialização pode baixar o pacote via npx na primeira vez
    let work = tokio::time::timeout(
        std::time::Duration::from_secs(240),
        mgr.ensure(&tid, AcpOpts { provider: provider.into(), cwd: cwd.to_path_buf(), ..Default::default() }, sink),
    )
    .await;
    mgr.stop(&tid).await;
    let init = captured.lock().clone();
    match work {
        Ok(Ok(())) => json!({ "provider": provider, "ok": true, "available": true, "launch": launch,
            "agent": init.as_ref().map(|i| i["agent"].clone()), "acp": init.map(|i| i["acp"].clone()) }),
        Ok(Err(e)) => json!({ "provider": provider, "ok": false, "available": true, "launch": launch, "error": e.to_string() }),
        Err(_) => json!({ "provider": provider, "ok": false, "available": true, "launch": launch, "error": "tempo esgotado ao iniciar o agente" }),
    }
}

/// Métodos de login que o agente oferece (`initialize`).
pub async fn auth_methods(provider: &str, cwd: &Path) -> Result<Value> {
    let mgr = AcpManager::default();
    let tid = format!("auth-{}", uuid::Uuid::new_v4().simple());
    let conn = mgr.spawn(&tid, provider, cwd, Arc::new(|_, _| {})).await?;
    let r = conn
        .call("initialize", json!({ "protocolVersion": 1, "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false }, "terminal": false } }),
            Some(std::time::Duration::from_secs(240)))
        .await;
    mgr.stop(&tid).await;
    Ok(json!({ "methods": r?["authMethods"], }))
}

/// Login pelo próprio agente (abre o navegador: Google, ChatGPT). Espera até
/// 5 minutos pela conclusão.
pub async fn login(provider: &str, method_id: &str, cwd: &Path) -> Result<()> {
    let mgr = AcpManager::default();
    let tid = format!("login-{}", uuid::Uuid::new_v4().simple());
    let conn = mgr.spawn(&tid, provider, cwd, Arc::new(|_, _| {})).await?;
    let t = Some(std::time::Duration::from_secs(240));
    let r = async {
        conn.call("initialize", json!({ "protocolVersion": 1, "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false }, "terminal": false } }), t).await?;
        conn.call("authenticate", json!({ "methodId": method_id }), Some(std::time::Duration::from_secs(300))).await
    }
    .await;
    mgr.stop(&tid).await;
    r.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_modes_and_models_from_new_and_legacy_shapes() {
        let new = json!({ "configOptions": [
            { "id": "mode", "category": "mode", "currentValue": "auto", "options": [{ "value": "auto", "name": "Auto" }] },
            { "id": "model", "category": "model", "currentValue": "gpt-x", "options": [{ "value": "gpt-x", "name": "GPT X" }] }
        ]});
        assert_eq!(options_of(&new, "mode").0.as_deref(), Some("auto"));
        assert_eq!(options_of(&new, "model").1[0]["id"], "gpt-x");
        let legacy = json!({ "modes": { "currentModeId": "default", "availableModes": [{ "id": "default", "name": "Default" }, { "id": "yolo", "name": "YOLO" }] },
                             "models": { "currentModelId": "gemini-pro", "availableModels": [{ "modelId": "gemini-pro", "name": "Pro" }] } });
        assert_eq!(options_of(&legacy, "mode").1.len(), 2);
        assert_eq!(options_of(&legacy, "model").1[0]["id"], "gemini-pro");
        assert!(options_of(&json!({}), "model").1.is_empty());
    }

    #[test]
    fn unknown_provider_is_refused() {
        assert!(command_for("x").is_err());
    }

    /// Agente ACP falso (Python): sessão, prompt com texto, ferramenta,
    /// pedido de permissão e fim de turno — o encanamento sem rede.
    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    async fn talks_acp_with_a_fake_agent_end_to_end() {
        if in_path(&["python3"]).is_none() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-acp");
        std::fs::write(&script, FAKE_AGENT).unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::env::set_var("LAZYAICODE_CODEX_ACP", &script);
        let (tx, rx) = std::sync::mpsc::channel::<Value>();
        let tx = parking_lot::Mutex::new(tx);
        let sink: Sink = Arc::new(move |_t, out| {
            if let AgentOut::Message(m) = out {
                let _ = tx.lock().send(m);
            }
        });
        let mgr = AcpManager::default();
        mgr.ensure("t", AcpOpts { provider: "codex".into(), cwd: dir.path().into(), mode: Some("ask".into()), ..Default::default() }, sink.clone())
            .await
            .unwrap();
        let recv = || rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        let init = recv();
        assert_eq!(init["subtype"], "init");
        assert_eq!(init["session_id"], "s-1");
        assert_eq!(init["acp"]["modes"][1]["id"], "ask");
        mgr.prompt("t", "oi", &[], sink).await.unwrap();
        let mut seen = vec![];
        loop {
            let m = recv();
            if m["type"] == "control_request" {
                let rid = m["request_id"].as_str().unwrap().to_string();
                assert_eq!(m["request"]["tool_name"], "execute");
                mgr.respond_permission("t", &rid, true, false).await.unwrap();
            }
            let done = m["type"] == "result";
            seen.push(m);
            if done {
                break;
            }
        }
        let text: String = seen.iter().filter(|m| m["type"] == "assistant").flat_map(|m| m["message"]["content"].as_array().cloned().unwrap_or_default())
            .filter_map(|b| b["text"].as_str().map(str::to_string)).collect();
        assert_eq!(text, "permitido: allow");
        let result = seen.last().unwrap();
        assert_eq!(result["is_error"], false);
        // o agente falso confirma o modo pedido na abertura
        assert!(seen.iter().any(|m| m["type"] == "lazy_config"));
        mgr.stop("t").await;
        std::env::remove_var("LAZYAICODE_CODEX_ACP");
    }

    const FAKE_AGENT: &str = r#"#!/usr/bin/env python3
import sys, json
def out(o): sys.stdout.write(json.dumps(o)+"\n"); sys.stdout.flush()
pending = {}
for line in sys.stdin:
    m = json.loads(line)
    meth, i = m.get("method"), m.get("id")
    if meth == "initialize":
        out({"jsonrpc":"2.0","id":i,"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True},"authMethods":[]}})
    elif meth == "session/new":
        out({"jsonrpc":"2.0","id":i,"result":{"sessionId":"s-1","modes":{"currentModeId":"auto","availableModes":[{"id":"auto","name":"Auto"},{"id":"ask","name":"Ask"}]}}})
    elif meth == "session/set_mode":
        out({"jsonrpc":"2.0","id":i,"result":{}})
        out({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s-1","update":{"sessionUpdate":"current_mode_update","currentModeId":m["params"]["modeId"]}}})
    elif meth == "session/prompt":
        pending["prompt"] = i
        out({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s-1","update":{"sessionUpdate":"tool_call","toolCallId":"c1","kind":"execute","title":"ls","status":"pending"}}})
        out({"jsonrpc":"2.0","id":99,"method":"session/request_permission","params":{"sessionId":"s-1","toolCall":{"toolCallId":"c1","kind":"execute","title":"ls"},"options":[{"optionId":"allow","name":"Sim","kind":"allow_once"},{"optionId":"deny","name":"Não","kind":"reject_once"}]}})
    elif i == 99 and "result" in m:
        chosen = m["result"]["outcome"].get("optionId")
        out({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s-1","update":{"sessionUpdate":"tool_call_update","toolCallId":"c1","status":"completed","rawOutput":"a.txt"}}})
        out({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s-1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"permitido: "}}}})
        out({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s-1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":chosen}}}})
        out({"jsonrpc":"2.0","id":pending["prompt"],"result":{"stopReason":"end_turn"}})
"#;
}

/// Ponta a ponta com um agente ACP real: o adaptador ACP do Claude Code,
/// que usa as credenciais locais do Claude. Valida o cliente que o Codex e o
/// Gemini usam. `cargo test -- --ignored real_acp --nocapture`
#[cfg(test)]
mod e2e {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_acp_agent_streams_asks_permission_and_edits() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = std::sync::mpsc::channel::<Value>();
        let tx = parking_lot::Mutex::new(tx);
        let sink: Sink = Arc::new(move |_t, out| {
            if let AgentOut::Message(m) = out {
                let _ = tx.lock().send(m);
            }
        });
        let mgr = AcpManager::default();
        mgr.ensure("e", AcpOpts { provider: "claude-acp".into(), cwd: dir.path().into(), mode: Some("default".into()), ..Default::default() }, sink.clone())
            .await
            .unwrap();
        let recv = || rx.recv_timeout(std::time::Duration::from_secs(240)).unwrap();
        let init = recv();
        println!("init: {}", init["acp"]["currentMode"]);
        mgr.prompt("e", "Use the Write tool to create ola.txt containing exactly: oi. Then reply with one short sentence.", &[], sink).await.unwrap();
        let (mut asked, mut usage, mut text) = (false, false, String::new());
        loop {
            let m = recv();
            if std::env::var("ACP_DEBUG").is_ok() {
                println!("evt {} {}", m["type"], m.to_string().chars().take(160).collect::<String>());
            }
            match m["type"].as_str().unwrap_or("") {
                "control_request" => {
                    asked = true;
                    mgr.respond_permission("e", m["request_id"].as_str().unwrap(), true, false).await.unwrap();
                }
                "lazy_usage" => usage = true,
                "assistant" => {
                    for b in m["message"]["content"].as_array().unwrap() {
                        if let Some(t) = b["text"].as_str() {
                            text.push_str(t);
                        }
                    }
                }
                "result" => {
                    println!("result: {m}");
                    assert_eq!(m["is_error"], false);
                    break;
                }
                _ => {}
            }
        }
        mgr.stop("e").await;
        println!("texto: {text}");
        assert!(asked, "o agente não pediu permissão em modo manual");
        assert!(usage, "sem usage_update");
        assert!(!text.is_empty());
        assert_eq!(std::fs::read_to_string(dir.path().join("ola.txt")).unwrap().trim(), "oi");
    }

    /// Codex e Gemini sem login nesta máquina: o status tem que dizer
    /// "precisa de login" com a mensagem do agente, sem travar.
    /// `cargo test -- --ignored real_provider_status --nocapture`
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_provider_status_reports_missing_login() {
        let dir = tempfile::tempdir().unwrap();
        for p in PROVIDERS {
            let t = std::time::Instant::now();
            let s = status(p, dir.path()).await;
            println!("{p} ({:?}): ok={} how={} erro={}", t.elapsed(), s["ok"], s["launch"]["how"], s["error"]);
            assert_eq!(s["available"], true);
        }
    }
}

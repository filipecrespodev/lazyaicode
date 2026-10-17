//! Ponta a ponta com o binário de verdade: o cenário de todas as
//! funcionalidades (`lazyaicode_testkit::scenario`) roda contra um
//! `lazyaicode-cli serve` pelo canal de controle TCP e contra um
//! `lazyaicode-cli mcp` pelo stdio; a CLI de uma chamada só (`run`, `batch`,
//! `commands`…) é conferida à parte. Os processos filhos recebem o ambiente
//! hermético do cenário: `claude`, agentes ACP e abridores falsos.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader, Write};
use std::net::SocketAddr;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use lazyaicode_automation::ControlClient;
use lazyaicode_testkit::scenario::{self, Fail, Fixture, Runner};
use serde_json::{json, Value};

const BIN: &str = env!("CARGO_BIN_EXE_lazyaicode-cli");

fn cli(fx: &Fixture) -> Command {
    let mut c = Command::new(BIN);
    c.envs(fx.env()).env_remove("LAZYAICODE_TOKEN").current_dir(&fx.root);
    c
}

/// Mata o filho ao sair do teste, mesmo se ele falhar.
struct Kill(Child);

impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> SocketAddr {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap()
}

fn all_commands() -> Vec<String> {
    lazyaicode_engine::commands().into_iter().map(|c| c.id).collect()
}

// ---------- canal de controle TCP ----------

struct Tcp {
    rt: tokio::runtime::Runtime,
    client: ControlClient,
}

impl Runner for Tcp {
    fn call(&mut self, id: &str, params: Value) -> Result<Value, Fail> {
        self.rt.block_on(self.client.call(id, params)).map_err(|e| Fail { code: e.code, message: e.message })
    }
}

#[test]
fn every_engine_command_over_the_control_channel() {
    let tmp = tempfile::tempdir().unwrap();
    let fx = Fixture::new(tmp.path());
    let addr = free_port();
    let token = "token-de-teste-e2e";
    let _server = Kill(
        cli(&fx)
            .args(["serve", "--control", &addr.to_string(), "--token", token])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let client = loop {
        match rt.block_on(ControlClient::connect(addr, token)) {
            Ok(c) => break c,
            Err(e) if Instant::now() > deadline => panic!("o servidor não subiu: {e}"),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    };
    // token errado é recusado
    assert!(rt.block_on(ControlClient::connect(addr, "errado")).is_err(), "token errado aceito");
    // os eventos chegam a quem assina
    let mut events = rt.block_on(client.subscribe(&[])).unwrap();
    let mut r = Tcp { rt, client };
    let report = scenario::run(&mut r, &fx);
    let mut seen = std::collections::BTreeSet::new();
    while let Ok(ev) = events.try_recv() {
        seen.insert(ev.event);
    }
    for ev in ["agent-event", "thread-status", "term-data"] {
        assert!(seen.contains(ev), "evento {ev} não chegou pelo canal: {seen:?}");
    }
    eprintln!("TCP: {} chamadas, {} comandos", report.calls, report.covered.len());
    report.assert_complete(&all_commands());
}

// ---------- WebSocket da versão web ----------

struct Ws {
    sock: tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    next: u64,
}

impl Runner for Ws {
    fn call(&mut self, id: &str, params: Value) -> Result<Value, Fail> {
        let n = self.next;
        self.next += 1;
        self.sock.send(tungstenite::Message::Text(json!({"id": n, "method": id, "params": params}).to_string().into())).unwrap();
        loop {
            let msg = self.sock.read().unwrap();
            let tungstenite::Message::Text(t) = msg else { continue };
            let v: Value = serde_json::from_str(t.as_str()).unwrap();
            if v["id"] != json!(n) {
                continue; // evento
            }
            return match v.get("error").filter(|e| !e.is_null()) {
                Some(e) => Err(Fail { code: e["code"].as_str().unwrap_or("").into(), message: e["message"].as_str().unwrap_or("").into() }),
                None => Ok(v["result"].clone()),
            };
        }
    }
}

#[test]
fn every_engine_command_over_the_web_socket() {
    let tmp = tempfile::tempdir().unwrap();
    let fx = Fixture::new(tmp.path());
    let addr = free_port();
    let token = "token-web-e2e";
    let _server = Kill(
        cli(&fx).args(["serve", "--web", &addr.to_string(), "--token", token]).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap(),
    );
    let connect = || {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match tungstenite::connect(format!("ws://{addr}/ws")) {
                Ok((s, _)) => {
                    if let tungstenite::stream::MaybeTlsStream::Plain(t) = s.get_ref() {
                        t.set_read_timeout(Some(Duration::from_secs(120))).unwrap();
                    }
                    break Ws { sock: s, next: 1 };
                }
                Err(e) if Instant::now() > deadline => panic!("o servidor web não subiu: {e}"),
                Err(_) => std::thread::sleep(Duration::from_millis(100)),
            }
        }
    };
    let denied = connect().call("engine.ping", json!({}));
    assert!(denied.as_ref().is_err_and(|f| f.code == "unauthorized"), "sem auth, nada roda: {denied:?}");
    let denied = connect().call("auth", json!({"token": "errado"}));
    assert!(denied.as_ref().is_err_and(|f| f.code == "unauthorized"), "token errado: {denied:?}");
    let mut r = connect();
    r.call("auth", json!({"token": token})).unwrap();
    let report = scenario::run(&mut r, &fx);
    eprintln!("WebSocket: {} chamadas, {} comandos", report.calls, report.covered.len());
    report.assert_complete(&all_commands());
}

// ---------- MCP no stdio ----------

struct Mcp {
    _child: Kill,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Mcp {
    fn start(fx: &Fixture) -> Self {
        let mut child = cli(fx).arg("mcp").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self { _child: Kill(child), stdin, stdout, next: 1 }
    }

    fn rpc(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        writeln!(self.stdin, "{}", json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})).unwrap();
        self.stdin.flush().unwrap();
        loop {
            let mut line = String::new();
            assert!(self.stdout.read_line(&mut line).unwrap() > 0, "o MCP fechou");
            let v: Value = serde_json::from_str(&line).unwrap();
            if v["id"] == json!(id) {
                return v;
            }
        }
    }

    /// Chama uma ferramenta; `Err` com o corpo de erro quando `isError`.
    fn tool(&mut self, name: &str, args: Value) -> Result<Value, Value> {
        let v = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        assert!(v.get("error").is_none(), "erro JSON-RPC em {name}: {v}");
        let text = v["result"]["content"][0]["text"].as_str().unwrap_or("null");
        let body: Value = serde_json::from_str(text).unwrap_or(Value::String(text.to_string()));
        if v["result"]["isError"] == json!(true) {
            Err(body)
        } else {
            Ok(body)
        }
    }
}

impl Runner for Mcp {
    fn call(&mut self, id: &str, params: Value) -> Result<Value, Fail> {
        self.tool("command_run", json!({"command": id, "params": params})).map_err(|e| Fail {
            code: e["error"]["code"].as_str().unwrap_or("").to_string(),
            message: e["error"]["message"].as_str().unwrap_or("").to_string(),
        })
    }
}

#[test]
fn every_engine_command_over_mcp() {
    let tmp = tempfile::tempdir().unwrap();
    let fx = Fixture::new(tmp.path());
    let mut m = Mcp::start(&fx);
    let init =
        m.rpc("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "e2e", "version": "1"}}));
    assert_eq!(init["result"]["serverInfo"]["name"], json!("lazyaicode"), "{init}");
    writeln!(m.stdin, "{}", json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).unwrap();
    let report = scenario::run(&mut m, &fx);
    eprintln!("MCP: {} chamadas, {} comandos", report.calls, report.covered.len());
    report.assert_complete(&all_commands());
}

/// As ferramentas próprias do MCP: lista, descrição, lote, estado, journal,
/// recursos e a conversa (`thread_send_and_wait` até a permissão).
#[test]
fn mcp_tools_and_resources() {
    let tmp = tempfile::tempdir().unwrap();
    let fx = Fixture::new(tmp.path());
    let mut m = Mcp::start(&fx);
    m.rpc("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "e2e", "version": "1"}}));
    let tools = m.rpc("tools/list", json!({}));
    let names: Vec<&str> = tools["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    for t in
        ["command_list", "command_describe", "command_run", "command_batch", "state_get", "journal", "thread_send_and_wait", "thread_wait"]
    {
        assert!(names.contains(&t), "ferramenta {t} ausente: {names:?}");
    }
    assert!(!names.contains(&"ui_inspect"), "sem ponte não há ferramentas de tela");
    let list = m.tool("command_list", json!({"group": "thread"})).unwrap();
    assert!(list.as_array().unwrap().iter().all(|c| c["group"] == "thread"), "{list}");
    let d = m.tool("command_describe", json!({"id": "thread.send"})).unwrap();
    assert_eq!(d["id"], json!("thread.send"));
    let b = m.tool("command_batch", json!({"steps": [{"command": "engine.ping"}, {"command": "engine.version"}]})).unwrap();
    assert_eq!(b.as_array().map(Vec::len), Some(2), "{b}");
    assert!(m.tool("command_run", json!({"command": "nao.existe"})).is_err());

    let w = m.tool("command_run", json!({"command": "workspace.create", "params": {"path": fx.ws.to_string_lossy()}})).unwrap();
    let wid = w["id"].as_str().unwrap().to_string();
    let projects = m.tool("command_run", json!({"command": "project.list", "params": {"workspaceId": wid}})).unwrap();
    let app = projects.as_array().unwrap().iter().find(|p| p["name"] == "app").unwrap()["id"].clone();
    let t = m
        .tool("command_run", json!({"command": "thread.create", "params": {"workspaceId": wid, "projectId": app, "isolated": false}}))
        .unwrap();
    let tid = t["id"].as_str().unwrap().to_string();

    let turn = m.tool("thread_send_and_wait", json!({"id": tid, "text": "olá", "timeoutMs": 20000})).unwrap();
    assert_eq!(turn["status"], json!("idle"), "{turn}");
    assert!(turn["text"].as_str().unwrap_or("").contains("feito: olá"), "{turn}");

    let turn = m.tool("thread_send_and_wait", json!({"id": tid, "text": "PERMISSAO", "timeoutMs": 20000})).unwrap();
    assert_eq!(turn["status"], json!("awaiting"), "{turn}");
    // o pedido traz o toolUseId que o CLI exige na resposta
    assert_eq!(turn["pendingPermission"]["toolUseId"], json!("tu-1"), "{turn}");
    let req = turn["pendingPermission"]["requestId"].as_str().unwrap().to_string();
    m.tool("command_run", json!({"command": "thread.respond-permission", "params": {"id": tid, "requestId": req, "allow": false}}))
        .unwrap();
    let done = m.tool("thread_wait", json!({"id": tid, "timeoutMs": 20000})).unwrap();
    let st = m.tool("command_run", json!({"command": "thread.status", "params": {"id": tid}})).unwrap();
    assert_eq!(st, json!("idle"), "thread_wait: {done}");
    let last = m.tool("command_run", json!({"command": "thread.last-message", "params": {"id": tid}})).unwrap();
    assert!(last.to_string().contains("permissão: deny"), "{last}");

    let state = m.tool("state_get", json!({})).unwrap();
    assert!(state.to_string().contains(&tid), "state_get");
    let j = m.tool("journal", json!({"last": 50})).unwrap();
    assert!(j.to_string().contains("thread.create"), "journal: {j}");
    let res = m.rpc("resources/list", json!({}));
    assert_eq!(res["result"]["resources"].as_array().map(Vec::len), Some(3));
    for uri in ["lazyaicode://commands", "lazyaicode://state", "lazyaicode://journal"] {
        let r = m.rpc("resources/read", json!({"uri": uri}));
        assert!(r["result"]["contents"][0]["text"].as_str().is_some_and(|t| !t.is_empty()), "{uri}: {r}");
    }
    let bad = m.rpc("resources/read", json!({"uri": "lazyaicode://nada"}));
    assert!(bad.get("error").is_some());
    let unknown = m.rpc("metodo/inexistente", json!({}));
    assert_eq!(unknown["error"]["code"], json!(-32601));
}

// ---------- CLI de uma chamada ----------

fn run_cli(fx: &Fixture, args: &[&str]) -> (bool, String, String) {
    let out = cli(fx).args(args).output().unwrap();
    (out.status.success(), String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string())
}

#[test]
fn the_one_shot_cli_subcommands() {
    let tmp = tempfile::tempdir().unwrap();
    let fx = Fixture::new(tmp.path());
    let (ok, out, _) = run_cli(&fx, &["run", "engine.ping"]);
    assert!(ok && out.trim() == "pong", "{out}");
    let (ok, _, err) = run_cli(&fx, &["commands"]);
    assert!(ok && err.contains("comandos"), "{err}");
    let (ok, out, _) = run_cli(&fx, &["commands", "--json", "--group", "terminal"]);
    assert!(ok && serde_json::from_str::<Value>(&out).unwrap().as_array().unwrap().iter().all(|c| c["group"] == "terminal"));
    let (ok, out, _) = run_cli(&fx, &["commands", "--markdown"]);
    assert!(ok && out.starts_with("| Comando"), "{out}");
    let (ok, out, _) = run_cli(&fx, &["describe", "thread.create"]);
    assert!(ok && out.contains("workspaceId"), "{out}");
    // estado em disco: o workspace criado por uma chamada aparece na próxima
    let ws = fx.ws.to_string_lossy().to_string();
    let (ok, out, err) = run_cli(&fx, &["run", "workspace.create", &format!("path={ws}")]);
    assert!(ok, "{out}{err}");
    let (ok, out, _) = run_cli(&fx, &["state"]);
    assert!(ok && out.contains(&ws), "{out}");
    // o journal é da sessão: numa chamada isolada começa vazio
    let (ok, out, _) = run_cli(&fx, &["journal"]);
    assert!(ok && out.trim() == "[]", "{out}");
    let (ok, out, _) = run_cli(&fx, &["run", "thread.get", r#"{"id": "nao"}"#]);
    assert!(!ok && out.is_empty(), "erro sai com código != 0");
    let steps = fx.root.join("lote.json");
    std::fs::write(&steps, r#"[{"command": "engine.ping"}, {"command": "prefs.set", "params": {"key": "a", "value": 1}}]"#).unwrap();
    let (ok, out, err) = run_cli(&fx, &["batch", &steps.to_string_lossy()]);
    assert!(ok && out.contains("\"ok\": true"), "{out}{err}");
    std::fs::write(&steps, r#"[{"command": "nao.existe"}, {"command": "engine.ping"}]"#).unwrap();
    let (ok, out, _) = run_cli(&fx, &["batch", "--keep-going", &steps.to_string_lossy()]);
    assert!(!ok && out.contains("pong"), "--keep-going segue e falha no fim: {out}");
    let (ok, out, _) = run_cli(&fx, &["--ephemeral", "run", "workspace.list"]);
    assert!(ok && !out.contains(&ws), "efêmero não lê o disco: {out}");
}

/// `--connect`: a CLI manda os comandos para um servidor aberto (o token sai
/// de `<dados>/control.token`), e o journal é o da sessão do servidor.
#[test]
fn the_cli_drives_a_running_server_with_connect() {
    let tmp = tempfile::tempdir().unwrap();
    let fx = Fixture::new(tmp.path());
    let addr = free_port();
    let _server =
        Kill(cli(&fx).args(["serve", "--control", &addr.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap());
    let a = addr.to_string();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let (ok, out, err) = run_cli(&fx, &["--connect", &a, "run", "engine.ping"]);
        if ok && out.trim() == "pong" {
            break;
        }
        assert!(Instant::now() < deadline, "o servidor não subiu: {out}{err}");
        std::thread::sleep(Duration::from_millis(100));
    }
    let ws = fx.ws.to_string_lossy().to_string();
    let (ok, out, err) = run_cli(&fx, &["--connect", &a, "run", "workspace.create", &format!("path={ws}")]);
    assert!(ok, "{out}{err}");
    let (ok, out, _) = run_cli(&fx, &["--connect", &a, "journal"]);
    assert!(ok && out.contains("workspace.create"), "{out}");
    let (ok, out, _) = run_cli(&fx, &["--connect", &a, "state"]);
    assert!(ok && out.contains(&ws), "{out}");
    let (ok, _, err) = run_cli(&fx, &["--connect", &a, "commands"]);
    assert!(ok && err.contains("comandos"), "{err}");
    let bad = fx.root.join("token-errado");
    std::fs::write(&bad, "errado").unwrap();
    let (ok, _, err) = run_cli(&fx, &["--connect", &a, "--token-file", &bad.to_string_lossy(), "run", "engine.ping"]);
    assert!(!ok && !err.is_empty(), "token errado recusado");
}

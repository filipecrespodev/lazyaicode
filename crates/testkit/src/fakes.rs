//! Programas falsos para testes ponta a ponta sem rede nem tokens: um
//! `claude` que fala os dois protocolos do app (stream-json da conversa e o
//! `-p --output-format json` das ações de IA), um agente ACP (Codex/Gemini)
//! e um abridor (`xdg-open`/`open`/`wslview`/editor) que só registra o
//! que mandaram abrir. Precisam de `python3` (só Unix).

use std::path::{Path, PathBuf};

/// O `claude` falso. Na conversa: anuncia a sessão `sess-1`, responde
/// `feito: <texto>`; uma mensagem com `PERMISSAO` pede permissão para a
/// ferramenta `Write` e só fecha o turno depois da resposta. Atende os
/// pedidos de controle (`initialize`, `mcp_status`, `mcp_toggle`…). No modo
/// `-p` responde as ações de IA (mensagem de commit, PR, conflito,
/// composição de commits) olhando a entrada.
pub const FAKE_CLAUDE: &str = r#"#!/usr/bin/env python3
import sys, json, re, os, uuid
args = sys.argv[1:]
def out(o):
    sys.stdout.write(json.dumps(o) + "\n"); sys.stdout.flush()
SID = "sess-1"
def log(o):
    # a sessão em disco, como o Claude Code grava: <conta>/projects/<cwd codificado>/<sessão>.jsonl
    d = os.path.join(ACCOUNT, "projects", re.sub(r"[^A-Za-z0-9]", "-", os.getcwd()))
    os.makedirs(d, exist_ok=True)
    o = dict(o, uuid=str(uuid.uuid4()), sessionId=SID, cwd=os.getcwd(), timestamp="2026-01-01T00:00:00Z")
    with open(os.path.join(d, SID + ".jsonl"), "a") as f:
        f.write(json.dumps(o) + "\n")
def say(mid, content):
    m = {"type": "assistant", "message": {"id": mid, "role": "assistant", "content": content}}
    log(m); out(m)
if "--version" in args:
    print("9.9.9 (Claude Code)"); sys.exit(0)
# só a conta de $FAKE_CLAUDE_LOGGED_IN tem login (como o CLI real com outra CLAUDE_CONFIG_DIR)
ACCOUNT = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.join(os.path.expanduser("~"), ".claude")
LOGGED = os.environ.get("FAKE_CLAUDE_LOGGED_IN")
NOT_LOGGED = bool(LOGGED) and os.path.realpath(ACCOUNT) != os.path.realpath(LOGGED)
if "auth" in args and "status" in args:
    print(json.dumps({"loggedIn": not NOT_LOGGED, "email": "demo@example.com"})); sys.exit(0)
if "--output-format" in args and args[args.index("--output-format") + 1] == "json":
    data = sys.stdin.read()
    if NOT_LOGGED:
        out({"type": "result", "is_error": True, "result": "Not logged in · Please run /login"}); sys.exit(0)
    if "Staged change units" in data:
        ids = re.findall(r"^### \[(.+)\]$", data, re.M)
        text = json.dumps({"commits": [{"summary": "feat: tudo junto", "description": "", "changes": ids}]})
    elif "=== OURS ===" in data:
        text = "resolvido\n"
    elif "Commits:" in data:
        text = "feat: título do PR\n\ncorpo do PR"
    else:
        text = "feat: mensagem gerada\n\ncorpo gerado"
    out({"type": "result", "is_error": False, "result": text}); sys.exit(0)
init = {"commands": [{"name": "review", "description": "Revisar", "argumentHint": ""}],
        "agents": [{"name": "general", "description": "Agente"}],
        "models": [{"value": "sonnet", "displayName": "Sonnet", "description": "padrão"}],
        "account": {"email": "demo@example.com"}, "claude_code_version": "9.9.9"}
mcp = [{"name": "demo", "status": "connected", "config": {"type": "stdio", "command": "demo"}}]
out({"type": "system", "subtype": "init", "session_id": SID, "model": "sonnet", "tools": ["Write"],
     "mcp_servers": [{"name": "demo", "status": "connected"}], "slash_commands": ["review"]})
waiting = None
for line in sys.stdin:
    try:
        m = json.loads(line)
    except Exception:
        continue
    t = m.get("type")
    if t == "control_request":
        req, rid = m.get("request", {}), m.get("request_id")
        sub = req.get("subtype")
        resp = {}
        if sub == "initialize": resp = init
        elif sub == "mcp_status": resp = {"mcpServers": mcp}
        elif sub == "interrupt" and waiting:
            out({"type": "result", "subtype": "error_during_execution", "is_error": True, "duration_ms": 1, "total_cost_usd": 0})
            waiting = None
        out({"type": "control_response", "response": {"subtype": "success", "request_id": rid, "response": resp}})
    elif t == "control_response":
        if waiting:
            r = m.get("response", {}).get("response", {})
            if r.get("behavior") == "allow" and r.get("toolUseID") != "tu-1":
                # como o CLI real: sem o toolUseID do pedido, a permissão não vale e ele pergunta de novo
                waiting = "perm-%d" % (int(waiting.split("-")[1]) + 1)
                out({"type": "control_request", "request_id": waiting, "request": {"subtype": "can_use_tool", "tool_name": "Write", "input": {"file_path": "x.txt", "content": "x"}, "tool_use_id": "tu-1"}})
                continue
            say("m2", [{"type": "text", "text": "permissão: " + str(r.get("behavior"))}])
            out({"type": "result", "is_error": False, "duration_ms": 5, "total_cost_usd": 0})
            waiting = None
    elif t == "user":
        content = m.get("message", {}).get("content", "")
        if isinstance(content, list):
            text = " ".join(c.get("text", "") for c in content if isinstance(c, dict))
        else:
            text = str(content)
        log({"type": "user", "message": {"role": "user", "content": content}})
        if NOT_LOGGED:
            say("m0", [{"type": "text", "text": "Not logged in · Please run /login"}])
            out({"type": "result", "subtype": "success", "is_error": True, "result": "Not logged in · Please run /login", "duration_ms": 1, "total_cost_usd": 0})
        elif "PERMISSAO" in text:
            waiting = "perm-1"
            say("m1", [{"type": "tool_use", "id": "tu-1", "name": "Write", "input": {"file_path": "x.txt", "content": "x"}}])
            out({"type": "control_request", "request_id": "perm-1", "request": {"subtype": "can_use_tool", "tool_name": "Write", "input": {"file_path": "x.txt", "content": "x"}, "tool_use_id": "tu-1"}})
        else:
            say("m1", [{"type": "text", "text": "feito: " + text.strip()}])
            out({"type": "result", "is_error": False, "duration_ms": 5, "total_cost_usd": 0})
"#;

/// Agente ACP falso (o mesmo protocolo do Codex e do Gemini CLI).
pub const FAKE_ACP: &str = r#"#!/usr/bin/env python3
import sys, json
def out(o): sys.stdout.write(json.dumps(o)+"\n"); sys.stdout.flush()
if "--version" in sys.argv: print("0.0.1"); sys.exit(0)
pending = {}
for line in sys.stdin:
    try: m = json.loads(line)
    except Exception: continue
    meth, i = m.get("method"), m.get("id")
    if meth == "initialize":
        out({"jsonrpc":"2.0","id":i,"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True},"authMethods":[{"id":"fake","name":"Fake","description":"login falso"}]}})
    elif meth == "authenticate":
        out({"jsonrpc":"2.0","id":i,"result":{}})
    elif meth in ("session/new", "session/load"):
        out({"jsonrpc":"2.0","id":i,"result":{"sessionId":"s-1","modes":{"currentModeId":"auto","availableModes":[{"id":"auto","name":"Auto"},{"id":"ask","name":"Ask"}]},"models":{"currentModelId":"m1","availableModels":[{"modelId":"m1","name":"Modelo 1"}]}}})
    elif meth in ("session/set_mode", "session/set_model"):
        out({"jsonrpc":"2.0","id":i,"result":{}})
    elif meth == "session/prompt":
        out({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s-1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"ok acp"}}}})
        out({"jsonrpc":"2.0","id":i,"result":{"stopReason":"end_turn"}})
    elif meth == "session/cancel":
        pass
    elif i is not None and meth is not None:
        out({"jsonrpc":"2.0","id":i,"result":{}})
"#;

/// Abridor falso: acrescenta os argumentos numa linha de `$FAKE_OPEN_LOG`.
pub const FAKE_OPENER: &str = "#!/bin/sh\necho \"$*\" >> \"${FAKE_OPEN_LOG:-/dev/null}\"\n";

#[cfg(unix)]
fn write_exec(path: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_path_buf()
}

/// Uma pasta `bin` com o `claude`, o agente ACP (`codex-acp`, `gemini`) e os
/// abridores falsos. Ponha-a na frente do `PATH`.
#[cfg(unix)]
pub fn fake_bin_dir(dir: &Path) -> PathBuf {
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    write_exec(&bin.join("claude"), FAKE_CLAUDE);
    write_exec(&bin.join("codex-acp"), FAKE_ACP);
    write_exec(&bin.join("gemini"), FAKE_ACP);
    for opener in ["xdg-open", "open", "wslview", "fake-editor"] {
        write_exec(&bin.join(opener), FAKE_OPENER);
    }
    bin
}

/// Um servidor HTTP local que responde a qualquer pedido com o "último
/// release" `tag` do GitHub (para `system.check-update` sem rede). Devolve o
/// endereço; atende em segundo plano enquanto o processo viver.
pub fn fake_release_server(tag: &str) -> String {
    use std::io::{Read, Write};
    let Ok(listener) = std::net::TcpListener::bind("127.0.0.1:0") else { return "http://127.0.0.1:9/".into() };
    let addr = listener.local_addr().map(|a| a.to_string()).unwrap_or_default();
    let body = format!(r#"{{"tag_name":"{tag}","html_url":"https://github.com/filipecrespodev/lazyaicode/releases/tag/{tag}"}}"#);
    std::thread::spawn(move || {
        for mut s in listener.incoming().flatten() {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    format!("http://{addr}/repos/x/y/releases/latest")
}

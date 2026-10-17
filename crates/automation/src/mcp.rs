//! Servidor MCP (Model Context Protocol) no stdio: JSON-RPC 2.0, uma
//! mensagem por linha.
//!
//! Poucas ferramentas, todas genéricas sobre o registro de comandos:
//! `command_list`/`command_describe` para descobrir, `command_run`/
//! `command_batch` para agir, `state_get`/`journal` para ler, e
//! `thread_send_and_wait`/`thread_wait` para conversar com o agente de uma
//! thread sem ter de acompanhar eventos. Na ponte com um app aberto, também
//! `ui_inspect`, `ui_screenshot` e `ui_command`.
//!
//! ```sh
//! claude mcp add lazyaicode -- lazyaicode-cli mcp            # motor no processo
//! claude mcp add lazyaicode -- lazyaicode-cli mcp --bridge 127.0.0.1:7878 --control-token-file ~/.lazyaicode-control.token
//! ```

use std::sync::Arc;
use std::time::Duration;

use lazyaicode_doc::transcript::{Item, Transcript};
use lazyaicode_protocol::{decode_line, events, ErrorBody, MAX_BATCH_STEPS, MAX_REPLY_BYTES};
use serde_json::{json, Value};
use tokio::io::{AsyncWriteExt, BufReader};

use crate::backend::Backend;
use crate::server::read_limited_line;

pub const PROTOCOL: &str = "2025-06-18";

pub struct Mcp {
    backend: Arc<dyn Backend>,
    /// ligado a um app aberto: expõe as ferramentas de UI
    bridge: bool,
}

fn tool(name: &str, description: &str, schema: Value, read_only: bool) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": schema,
        "annotations": { "readOnlyHint": read_only },
    })
}

fn obj(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
}

fn text_result(v: &Value, is_error: bool) -> Value {
    let mut text = serde_json::to_string_pretty(v).unwrap_or_default();
    if text.len() > MAX_REPLY_BYTES {
        let mut cut = MAX_REPLY_BYTES;
        while cut > 0 && !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push_str("\n… (resultado cortado no limite de 8 MiB)");
    }
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

fn err_value(e: &ErrorBody) -> Value {
    json!({ "error": { "code": e.code, "message": e.message, "data": e.data } })
}

impl Mcp {
    pub fn new(backend: Arc<dyn Backend>, bridge: bool) -> Self {
        Self { backend, bridge }
    }

    pub fn tools(&self) -> Vec<Value> {
        let mut t = vec![
            tool(
                "command_list",
                "Lista os comandos do LazyAICode (id, rótulo, grupo, parâmetros). Comece por aqui; filtre por texto ou grupo (thread, workspace, git, review, terminal, settings…).",
                obj(json!({ "filter": {"type": "string"}, "group": {"type": "string"} }), &[]),
                true,
            ),
            tool("command_describe", "Descrição completa de um comando, com os parâmetros.", obj(json!({ "id": {"type": "string"} }), &["id"]), true),
            tool(
                "command_run",
                "Executa um comando pelo id com parâmetros JSON (camelCase). Ex.: {\"command\":\"thread.create\",\"params\":{\"workspaceId\":\"…\",\"projectId\":\"…\",\"isolated\":true}}.",
                obj(json!({ "command": {"type": "string"}, "params": {"type": "object"} }), &["command"]),
                false,
            ),
            tool(
                "command_batch",
                "Executa até 256 comandos em sequência; para no primeiro erro, a menos que stopOnError seja false.",
                obj(
                    json!({
                        "steps": {"type": "array", "items": {"type": "object", "properties": {"command": {"type": "string"}, "params": {"type": "object"}}, "required": ["command"]}},
                        "stopOnError": {"type": "boolean"}
                    }),
                    &["steps"],
                ),
                false,
            ),
            tool("state_get", "Estado da sessão: workspaces, projetos, threads (com status) e preferências.", obj(json!({}), &[]), true),
            tool("journal", "Os últimos comandos que mudaram algo nesta sessão.", obj(json!({ "last": {"type": "integer"} }), &[]), true),
            tool(
                "thread_send_and_wait",
                "Manda uma mensagem ao agente de uma thread e espera o turno terminar (ou pedir permissão). Devolve o texto final, as ferramentas usadas e o pedido de permissão pendente, se houver.",
                obj(json!({ "id": {"type": "string"}, "text": {"type": "string"}, "timeoutMs": {"type": "integer"} }), &["id", "text"]),
                false,
            ),
            tool(
                "thread_wait",
                "Espera a thread sair de `running` (ex.: depois de responder uma permissão com thread.respond-permission).",
                obj(json!({ "id": {"type": "string"}, "timeoutMs": {"type": "integer"} }), &["id"]),
                true,
            ),
        ];
        if self.bridge {
            t.push(tool(
                "ui_inspect",
                "Estado da tela do app aberto (aba, thread selecionada, painéis, diálogos).",
                obj(json!({}), &[]),
                true,
            ));
            t.push(tool(
                "ui_screenshot",
                "Captura a janela do app como PNG (base64). `cpu: true` usa o renderizador de referência em CPU.",
                obj(json!({ "cpu": {"type": "boolean"} }), &[]),
                true,
            ));
            t.push(tool(
                "ui_command",
                "Executa um comando de tela (ui.*: trocar de aba, abrir paleta/configurações, zoom…).",
                obj(json!({ "command": {"type": "string"}, "params": {"type": "object"} }), &["command"]),
                false,
            ));
        }
        t
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, ErrorBody> {
        self.backend.call(method.to_string(), params).await
    }

    async fn wait_thread(
        &self,
        id: &str,
        timeout: Duration,
        mut rx: tokio::sync::broadcast::Receiver<lazyaicode_protocol::Event>,
    ) -> Value {
        let mut tr = Transcript::new();
        let mut status = String::from("running");
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            match tokio::time::timeout_at(deadline, rx.recv()).await {
                Err(_) => {
                    status = "timeout".into();
                    break;
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
                Ok(Err(_)) => continue,
                Ok(Ok(ev)) => {
                    if ev.payload["threadId"].as_str() != Some(id) {
                        continue;
                    }
                    if ev.event == events::AGENT_EVENT {
                        tr.apply(&ev.payload["msg"]);
                    } else if ev.event == events::THREAD_STATUS {
                        let s = ev.payload["status"].as_str().unwrap_or("");
                        if s != "running" {
                            status = s.to_string();
                            break;
                        }
                    } else if ev.event == events::AGENT_EXIT {
                        status = "exited".into();
                        break;
                    }
                }
            }
        }
        summarize(&tr, &status)
    }

    async fn call_tool(&self, name: &str, a: Value) -> Result<Value, ErrorBody> {
        match name {
            "command_list" => {
                let mut list = self.call("engine.commands", json!({ "filter": a.get("filter"), "group": a.get("group") })).await?;
                // lista curta: id, rótulo, grupo e nomes dos parâmetros
                if let Some(items) = list.as_array_mut() {
                    for c in items.iter_mut() {
                        let params: Vec<String> = c["params"]
                            .as_array()
                            .map(|ps| {
                                ps.iter()
                                    .map(|p| {
                                        format!("{}{}", p["name"].as_str().unwrap_or(""), if p["required"] == true { "" } else { "?" })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        *c = json!({ "id": c["id"], "label": c["label"], "group": c["group"], "params": params, "readOnly": c["readOnly"], "destructive": c["destructive"] });
                    }
                }
                Ok(list)
            }
            "command_describe" => self.call("engine.describe", json!({ "id": a.get("id") })).await,
            "command_run" => {
                let cmd = a.get("command").and_then(Value::as_str).unwrap_or("").to_string();
                let params = a.get("params").cloned().unwrap_or(Value::Null);
                self.call(&cmd, params).await
            }
            "command_batch" => {
                if a["steps"].as_array().is_some_and(|s| s.len() > MAX_BATCH_STEPS) {
                    return Err(crate::backend::bad(format!("no máximo {MAX_BATCH_STEPS} passos")));
                }
                self.call("engine.batch", json!({ "steps": a.get("steps"), "stopOnError": a.get("stopOnError") })).await
            }
            "state_get" => self.call("engine.state", Value::Null).await,
            "journal" => self.call("engine.journal", json!({ "last": a.get("last") })).await,
            "thread_send_and_wait" | "thread_wait" => {
                let id = a.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                let timeout = Duration::from_millis(a.get("timeoutMs").and_then(Value::as_u64).unwrap_or(600_000).clamp(1000, 3_600_000));
                // assina antes de mandar: nenhum evento do turno se perde
                let rx = self.backend.events();
                if name == "thread_send_and_wait" {
                    self.call("thread.send", json!({ "id": id, "text": a.get("text") })).await?;
                } else {
                    let t = self.call("thread.get", json!({ "id": id })).await?;
                    if t["status"] != "running" {
                        return Ok(json!({ "status": t["status"], "note": "a thread não estava rodando" }));
                    }
                }
                Ok(self.wait_thread(&id, timeout, rx).await)
            }
            "ui_inspect" if self.bridge => self.call("ui.inspect", Value::Null).await,
            "ui_screenshot" if self.bridge => self.call("ui.screenshot", json!({ "cpu": a.get("cpu") })).await,
            "ui_command" if self.bridge => self.call("ui.run", json!({ "command": a.get("command"), "params": a.get("params") })).await,
            other => Err(ErrorBody { code: "unknown_tool".into(), message: format!("ferramenta desconhecida: {other}"), data: None }),
        }
    }

    async fn read_resource(&self, uri: &str) -> Result<Value, ErrorBody> {
        let v = match uri {
            "lazyaicode://commands" => self.call("engine.commands", Value::Null).await?,
            "lazyaicode://state" => self.call("engine.state", Value::Null).await?,
            "lazyaicode://journal" => self.call("engine.journal", Value::Null).await?,
            _ => return Err(crate::backend::bad(format!("recurso desconhecido: {uri}"))),
        };
        Ok(json!({ "contents": [{ "uri": uri, "mimeType": "application/json", "text": v.to_string() }] }))
    }

    /// Uma mensagem JSON-RPC → a resposta (nada para notificações).
    pub async fn handle(&self, msg: Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg["method"].as_str().unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let result: Result<Value, (i64, String)> = match method {
            "initialize" => Ok(json!({
                "protocolVersion": PROTOCOL,
                "capabilities": { "tools": {}, "resources": {} },
                "serverInfo": { "name": "lazyaicode", "title": "LazyAICode", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "LazyAICode é um workspace de agentes de código. Use command_list para descobrir os comandos; \
                    workspace.list → project.list → thread.create → thread_send_and_wait é o caminho de uma conversa; \
                    thread.changes mostra o que o agente mudou e thread.land traz para o projeto.",
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": self.tools() })),
            "tools/call" => {
                let name = params["name"].as_str().unwrap_or("").to_string();
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                Ok(match self.call_tool(&name, args).await {
                    Ok(v) => text_result(&v, false),
                    Err(e) => text_result(&err_value(&e), true),
                })
            }
            "resources/list" => Ok(json!({ "resources": [
                { "uri": "lazyaicode://commands", "name": "commands", "description": "Todos os comandos do motor", "mimeType": "application/json" },
                { "uri": "lazyaicode://state", "name": "state", "description": "Estado da sessão", "mimeType": "application/json" },
                { "uri": "lazyaicode://journal", "name": "journal", "description": "Journal da sessão", "mimeType": "application/json" },
            ]})),
            "resources/read" => self.read_resource(params["uri"].as_str().unwrap_or("")).await.map_err(|e| (-32602, e.message)),
            m if m.starts_with("notifications/") => return None,
            _ => Err((-32601, format!("método desconhecido: {method}"))),
        };
        let id = id?;
        Some(match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
        })
    }
}

/// Resumo de um turno para o agente que chamou.
fn summarize(tr: &Transcript, status: &str) -> Value {
    let mut text = Vec::new();
    let mut tools = Vec::new();
    let mut permission = Value::Null;
    let mut result = Value::Null;
    for i in &tr.items {
        match i {
            Item::Text { text: t, .. } => text.push(t.clone()),
            Item::Tool { name, input, result: r, .. } => tools.push(json!({
                "name": name,
                "summary": lazyaicode_doc::transcript::tool_summary(name, input),
                "error": r.as_ref().is_some_and(|r| r.is_error),
            })),
            Item::Permission { request_id, tool_name, input, tool_use_id, decided: None, suggestions, .. } => {
                permission = json!({ "requestId": request_id, "toolName": tool_name, "input": input, "toolUseId": tool_use_id, "suggestions": suggestions });
            }
            Item::Result { duration_ms, cost_usd, is_error, text: t, .. } => {
                result = json!({ "durationMs": duration_ms, "costUsd": cost_usd, "isError": is_error, "error": t });
            }
            _ => {}
        }
    }
    json!({ "status": status, "text": text.join("\n\n"), "tools": tools, "pendingPermission": permission, "result": result })
}

/// Atende no stdin/stdout até o cliente fechar.
pub async fn serve_stdio(backend: Arc<dyn Backend>, bridge: bool) -> std::io::Result<()> {
    let mcp = Arc::new(Mcp::new(backend, bridge));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        let mut out = tokio::io::stdout();
        while let Some(line) = rx.recv().await {
            if out.write_all(line.as_bytes()).await.is_err() || out.flush().await.is_err() {
                break;
            }
        }
    });
    let mut reader = BufReader::new(tokio::io::stdin());
    let mut buf = Vec::new();
    while let Some(ok) = read_limited_line(&mut reader, &mut buf).await? {
        if !ok {
            let _ = tx.send(
                json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32600, "message": "mensagem grande demais"}}).to_string() + "\n",
            );
            continue;
        }
        let line = String::from_utf8_lossy(&buf).trim().to_string();
        if line.is_empty() {
            continue;
        }
        let msg: Value = match decode_line(&line) {
            Ok(v) => v,
            Err(e) => {
                let _ =
                    tx.send(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": e.to_string()}}).to_string() + "\n");
                continue;
            }
        };
        // cada pedido numa task: um thread_send_and_wait não trava os outros
        let (mcp, tx) = (mcp.clone(), tx.clone());
        tokio::spawn(async move {
            if let Some(r) = mcp.handle(msg).await {
                let _ = tx.send(r.to_string() + "\n");
            }
        });
    }
    drop(tx);
    let _ = writer.await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::EngineBackend;
    use lazyaicode_engine::{Engine, EngineOpts};

    fn mcp(dir: &std::path::Path) -> Mcp {
        Mcp::new(EngineBackend::new(Engine::new(EngineOpts { data_dir: Some(dir.to_path_buf()), ephemeral: true })), false)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn speaks_json_rpc_and_runs_commands_through_tools() {
        let d = tempfile::tempdir().unwrap();
        let m = mcp(d.path());
        let init = m.handle(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}})).await.unwrap();
        assert_eq!(init["result"]["protocolVersion"], json!(PROTOCOL));
        assert!(m.handle(json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await.is_none());
        let tools = m.handle(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).await.unwrap();
        let names: Vec<&str> = tools["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"command_run") && !names.contains(&"ui_inspect"));

        let r = m
            .handle(
                json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"command_run","arguments":{"command":"engine.ping"}}}),
            )
            .await
            .unwrap();
        assert_eq!(r["result"]["isError"], json!(false));
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("pong"));

        let r = m
            .handle(json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"command_run","arguments":{"command":"thread.get","params":{"id":"x"}}}}))
            .await
            .unwrap();
        assert_eq!(r["result"]["isError"], json!(true));

        let r = m
            .handle(json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"command_list","arguments":{"group":"thread"}}}))
            .await
            .unwrap();
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("thread.send"));

        let r = m.handle(json!({"jsonrpc":"2.0","id":6,"method":"resources/read","params":{"uri":"lazyaicode://state"}})).await.unwrap();
        assert!(r["result"]["contents"][0]["text"].as_str().unwrap().contains("workspaces"));

        let r = m.handle(json!({"jsonrpc":"2.0","id":7,"method":"bogus"})).await.unwrap();
        assert_eq!(r["error"]["code"], json!(-32601));
    }
}

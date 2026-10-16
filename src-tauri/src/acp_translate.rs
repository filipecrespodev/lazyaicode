//! Tradução dos eventos ACP (Agent Client Protocol — Codex, Gemini) para o
//! mesmo formato de mensagens que o Claude Code emite em stream-json. Assim a
//! UI tem um único renderizador, um único redutor de transcrição e as mesmas
//! regras de status/notificação para todos os provedores.
//!
//! Eventos próprios (`lazy_*`) carregam o que não existe no stream-json:
//! uso de contexto, comandos `/` e as opções de modo/modelo da sessão.

use serde_json::{json, Map, Value};
use std::collections::HashMap;

#[derive(Default)]
pub struct Translator {
    turn: u64,
    msg_id: String,
    /// blocos do texto em curso: (thinking|text, conteúdo)
    blocks: Vec<(String, String)>,
    message_open: bool,
    /// chamadas de ferramenta conhecidas: id → (nome, input acumulado)
    tools: HashMap<String, (String, Map<String, Value>)>,
    plan_id: Option<String>,
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().map(text_of).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n"),
        Value::Object(o) => match o.get("type").and_then(Value::as_str) {
            Some("text") => o.get("text").and_then(Value::as_str).unwrap_or_default().to_string(),
            // item de ferramenta: { type: "content", content: { type: "text", text } }
            Some("content") => o.get("content").map(text_of).unwrap_or_default(),
            Some("diff") => {
                let path = o.get("path").and_then(Value::as_str).unwrap_or("");
                let new = o.get("newText").and_then(Value::as_str).unwrap_or("");
                let lines = new.lines().count();
                format!("{path} ({lines} linha(s))")
            }
            Some("terminal") => "[terminal]".into(),
            _ => o.get("text").and_then(Value::as_str).map(str::to_string).unwrap_or_default(),
        },
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Nome curto da ferramenta: o nome real quando o agente informa
/// (Claude/Gemini mandam em `_meta`/`name`), senão o tipo ACP (`edit`,
/// `execute`, `read`, …) com inicial maiúscula.
fn tool_name(u: &Value) -> String {
    let real = u.pointer("/_meta/claudeCode/toolName").and_then(Value::as_str).or_else(|| u.get("name").and_then(Value::as_str));
    if let Some(n) = real.filter(|n| !n.is_empty()) {
        return n.to_string();
    }
    let kind = u.get("kind").and_then(Value::as_str).unwrap_or("tool");
    let mut c = kind.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_else(|| "Ferramenta".into())
}

impl Translator {
    /// Início de um prompt: mensagens novas não se misturam com as anteriores.
    pub fn begin_turn(&mut self) {
        self.turn += 1;
        self.msg_id = format!("acp-{}-0", self.turn);
        self.blocks.clear();
        self.message_open = false;
        self.plan_id = None;
    }

    fn open_message(&mut self, out: &mut Vec<Value>, msg_id: Option<&str>) {
        if let Some(id) = msg_id {
            if self.message_open && id != self.msg_id {
                out.extend(self.close_message());
            }
            if !self.message_open {
                self.msg_id = id.to_string();
            }
        }
        if !self.message_open {
            self.message_open = true;
            self.blocks.clear();
            out.push(json!({ "type": "stream_event", "event": { "type": "message_start", "message": { "id": self.msg_id } } }));
        }
    }

    /// Fecha a mensagem em curso com o texto completo — é o que fica no
    /// histórico e o que finaliza os blocos que vinham em pedaços.
    fn close_message(&mut self) -> Vec<Value> {
        if !self.message_open {
            return vec![];
        }
        self.message_open = false;
        let content: Vec<Value> = self
            .blocks
            .drain(..)
            .map(|(k, t)| if k == "thinking" { json!({ "type": "thinking", "thinking": t }) } else { json!({ "type": "text", "text": t }) })
            .collect();
        if content.is_empty() {
            return vec![];
        }
        vec![json!({ "type": "assistant", "message": { "id": self.msg_id, "role": "assistant", "content": content } })]
    }

    fn tool_use(&self, id: &str) -> Value {
        let (name, input) = self.tools.get(id).cloned().unwrap_or_default();
        json!({ "type": "assistant", "message": { "id": format!("tool-{id}"), "role": "assistant",
            "content": [{ "type": "tool_use", "id": id, "name": name, "input": input }] } })
    }

    fn tool_result(id: &str, text: String, is_error: bool) -> Value {
        json!({ "type": "user", "message": { "role": "user",
            "content": [{ "type": "tool_result", "tool_use_id": id, "content": text, "is_error": is_error }] } })
    }

    fn merge_tool(&mut self, u: &Value) -> String {
        let id = u.get("toolCallId").and_then(Value::as_str).unwrap_or("tool").to_string();
        let entry = self.tools.entry(id.clone()).or_insert_with(|| (tool_name(u), Map::new()));
        if u.get("name").is_some() || u.pointer("/_meta/claudeCode/toolName").is_some() || u.get("kind").is_some() {
            let n = tool_name(u);
            if !n.is_empty() && entry.0 != n && (entry.0 == "Tool" || entry.0.is_empty() || u.get("name").is_some()) {
                entry.0 = n;
            }
        }
        if let Some(raw) = u.get("rawInput").and_then(Value::as_object) {
            for (k, v) in raw {
                entry.1.insert(k.clone(), v.clone());
            }
        }
        if let Some(t) = u.get("title").and_then(Value::as_str) {
            entry.1.insert("title".into(), json!(t));
        }
        id
    }

    pub fn on_update(&mut self, u: &Value) -> Vec<Value> {
        let mut out = Vec::new();
        match u.get("sessionUpdate").and_then(Value::as_str).unwrap_or_default() {
            kind @ ("agent_message_chunk" | "agent_thought_chunk") => {
                let text = text_of(u.get("content").unwrap_or(&Value::Null));
                if text.is_empty() {
                    return out;
                }
                self.open_message(&mut out, u.get("messageId").and_then(Value::as_str));
                let block = if kind == "agent_thought_chunk" { "thinking" } else { "text" };
                if self.blocks.last().map(|b| b.0.as_str()) != Some(block) {
                    let index = self.blocks.len();
                    self.blocks.push((block.into(), String::new()));
                    let start = if block == "thinking" { json!({ "type": "thinking", "thinking": "" }) } else { json!({ "type": "text", "text": "" }) };
                    out.push(json!({ "type": "stream_event", "event": { "type": "content_block_start", "index": index, "content_block": start } }));
                }
                let index = self.blocks.len() - 1;
                self.blocks[index].1.push_str(&text);
                let delta = if block == "thinking" { json!({ "type": "thinking_delta", "thinking": text }) } else { json!({ "type": "text_delta", "text": text }) };
                out.push(json!({ "type": "stream_event", "event": { "type": "content_block_delta", "index": index, "delta": delta } }));
            }
            "tool_call" | "tool_call_update" => {
                // texto anterior à ferramenta vira uma mensagem completa
                out.extend(self.close_message());
                let id = self.merge_tool(u);
                let touches_input = u.get("rawInput").is_some() || u.get("title").is_some() || u.get("sessionUpdate").and_then(Value::as_str) == Some("tool_call");
                if touches_input {
                    out.push(self.tool_use(&id));
                }
                match u.get("status").and_then(Value::as_str) {
                    Some(st @ ("completed" | "failed")) => {
                        let mut text = match u.get("rawOutput") {
                            Some(Value::String(s)) => s.clone(),
                            Some(v) if !v.is_null() => text_of(v).trim().to_string().chars().take(20000).collect::<String>().pipe_or(|| v.to_string()),
                            _ => String::new(),
                        };
                        if text.is_empty() {
                            text = text_of(u.get("content").unwrap_or(&Value::Null));
                        }
                        out.push(Self::tool_result(&id, text, st == "failed"));
                    }
                    _ => {}
                }
            }
            "plan" => {
                out.extend(self.close_message());
                let id = self.plan_id.get_or_insert_with(|| format!("plan-{}", self.turn)).clone();
                let entries = u.get("entries").and_then(Value::as_array).cloned().unwrap_or_default();
                let done = entries.iter().filter(|e| e["status"] == "completed").count();
                let lines: Vec<String> = entries
                    .iter()
                    .map(|e| {
                        let mark = match e["status"].as_str() { Some("completed") => "[x]", Some("in_progress") => "[~]", _ => "[ ]" };
                        format!("{mark} {}", e["content"].as_str().unwrap_or(""))
                    })
                    .collect();
                let mut input = Map::new();
                input.insert("title".into(), json!(format!("{done}/{} passos", entries.len())));
                self.tools.insert(id.clone(), ("Plano".into(), input));
                out.push(self.tool_use(&id));
                out.push(Self::tool_result(&id, lines.join("\n"), false));
            }
            "usage_update" => {
                if let (Some(used), Some(size)) = (u.get("used").and_then(Value::as_u64), u.get("size").and_then(Value::as_u64)) {
                    let model = u.pointer("/_meta/_claude~1model").and_then(Value::as_str).unwrap_or("");
                    out.push(json!({ "type": "lazy_usage", "used": used, "size": size, "model": model, "cost": u.get("cost") }));
                }
            }
            "available_commands_update" => {
                let cmds: Vec<Value> = u
                    .get("availableCommands")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(|c| json!({ "name": c["name"], "description": c["description"], "hint": c.pointer("/input/hint") }))
                    .collect();
                out.push(json!({ "type": "lazy_commands", "commands": cmds }));
            }
            "current_mode_update" => {
                out.push(json!({ "type": "lazy_config", "currentModeId": u.get("currentModeId") }));
            }
            "config_option_update" => {
                out.push(json!({ "type": "lazy_config", "configOptions": u.get("configOptions") }));
            }
            _ => {}
        }
        out
    }

    /// Fim do prompt: fecha o texto e emite o `result`.
    pub fn end_turn(&mut self, response: Result<&Value, String>, duration_ms: u64) -> Vec<Value> {
        let mut out = self.close_message();
        let (is_error, text, stop) = match response {
            Ok(r) => {
                let stop = r.get("stopReason").and_then(Value::as_str).unwrap_or("end_turn").to_string();
                let err = match stop.as_str() {
                    "refusal" => Some("O modelo recusou a solicitação".to_string()),
                    "max_tokens" => Some("Limite de tokens de saída atingido".to_string()),
                    "max_turn_requests" => Some("Limite de chamadas no turno atingido".to_string()),
                    _ => None,
                };
                (err.is_some(), err, stop)
            }
            Err(e) => (true, Some(e), "error".to_string()),
        };
        out.push(json!({ "type": "result", "subtype": if is_error { "error" } else { "success" },
            "is_error": is_error, "result": text, "stop_reason": stop, "duration_ms": duration_ms }));
        out
    }
}

trait PipeOr {
    fn pipe_or(self, f: impl FnOnce() -> String) -> String;
}
impl PipeOr for String {
    fn pipe_or(self, f: impl FnOnce() -> String) -> String {
        if self.is_empty() { f() } else { self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn types(v: &[Value]) -> Vec<String> {
        v.iter()
            .map(|m| match m["type"].as_str().unwrap() {
                "stream_event" => format!("se:{}", m["event"]["type"].as_str().unwrap()),
                t => t.to_string(),
            })
            .collect()
    }

    #[test]
    fn streams_text_and_closes_it_with_the_full_message_at_the_end() {
        let mut t = Translator::default();
        t.begin_turn();
        let mut all = vec![];
        all.extend(t.on_update(&json!({"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"pensando"}})));
        all.extend(t.on_update(&json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Olá"}})));
        all.extend(t.on_update(&json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":", mundo"}})));
        all.extend(t.end_turn(Ok(&json!({"stopReason":"end_turn"})), 1200));
        assert_eq!(
            types(&all),
            vec!["se:message_start", "se:content_block_start", "se:content_block_delta", "se:content_block_start", "se:content_block_delta", "se:content_block_delta", "assistant", "result"]
        );
        let full = &all[6]["message"]["content"];
        assert_eq!(full[0]["thinking"], "pensando");
        assert_eq!(full[1]["text"], "Olá, mundo");
        assert_eq!(all[7]["is_error"], false);
        assert_eq!(all[7]["duration_ms"], 1200);
    }

    #[test]
    fn a_new_message_id_closes_the_previous_message() {
        let mut t = Translator::default();
        t.begin_turn();
        t.on_update(&json!({"sessionUpdate":"agent_message_chunk","messageId":"m1","content":{"type":"text","text":"a"}}));
        let out = t.on_update(&json!({"sessionUpdate":"agent_message_chunk","messageId":"m2","content":{"type":"text","text":"b"}}));
        assert_eq!(types(&out)[0], "assistant");
        assert_eq!(out[0]["message"]["id"], "m1");
        assert_eq!(out[1]["event"]["message"]["id"], "m2");
    }

    #[test]
    fn tool_calls_become_tool_use_and_tool_result_with_the_real_name_and_title() {
        let mut t = Translator::default();
        t.begin_turn();
        t.on_update(&json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"vou criar"}}));
        let a = t.on_update(&json!({"sessionUpdate":"tool_call","toolCallId":"t1","_meta":{"claudeCode":{"toolName":"Write"}},"kind":"edit","title":"Preparing","rawInput":{},"status":"pending"}));
        // fecha o texto antes da ferramenta
        assert_eq!(types(&a), vec!["assistant", "assistant"]);
        assert_eq!(a[1]["message"]["content"][0]["name"], "Write");
        let b = t.on_update(&json!({"sessionUpdate":"tool_call_update","toolCallId":"t1","title":"Write ola.txt","rawInput":{"file_path":"/x/ola.txt"}}));
        assert_eq!(b[0]["message"]["content"][0]["input"]["file_path"], "/x/ola.txt");
        assert_eq!(b[0]["message"]["content"][0]["input"]["title"], "Write ola.txt");
        let c = t.on_update(&json!({"sessionUpdate":"tool_call_update","toolCallId":"t1","status":"completed","rawOutput":"File created"}));
        assert_eq!(types(&c), vec!["user"]);
        assert_eq!(c[0]["message"]["content"][0]["content"], "File created");
        let d = t.on_update(&json!({"sessionUpdate":"tool_call","toolCallId":"t2","kind":"execute","title":"ls","status":"failed","content":[{"type":"content","content":{"type":"text","text":"boom"}}]}));
        assert_eq!(d[0]["message"]["content"][0]["name"], "Execute");
        assert_eq!(d[1]["message"]["content"][0]["is_error"], true);
        assert_eq!(d[1]["message"]["content"][0]["content"], "boom");
    }

    #[test]
    fn plan_usage_commands_and_errors() {
        let mut t = Translator::default();
        t.begin_turn();
        let p = t.on_update(&json!({"sessionUpdate":"plan","entries":[{"content":"ler","status":"completed"},{"content":"editar","status":"pending"}]}));
        assert_eq!(p[0]["message"]["content"][0]["input"]["title"], "1/2 passos");
        assert_eq!(p[1]["message"]["content"][0]["content"], "[x] ler\n[ ] editar");
        let u = t.on_update(&json!({"sessionUpdate":"usage_update","used":22688,"size":1000000,"_meta":{"_claude/model":"claude-opus-5-5"}}));
        assert_eq!(u[0], json!({"type":"lazy_usage","used":22688,"size":1000000,"model":"claude-opus-5-5","cost":null}));
        let c = t.on_update(&json!({"sessionUpdate":"available_commands_update","availableCommands":[{"name":"init","description":"Cria AGENTS.md","input":null}]}));
        assert_eq!(c[0]["commands"][0]["name"], "init");
        let e = t.end_turn(Err("Authentication required".into()), 5);
        assert_eq!(e[0]["is_error"], true);
        assert_eq!(e[0]["result"], "Authentication required");
        let r = t.end_turn(Ok(&json!({"stopReason":"refusal"})), 5);
        assert_eq!(r[0]["is_error"], true);
        let x = t.end_turn(Ok(&json!({"stopReason":"cancelled"})), 5);
        assert_eq!(x[0]["is_error"], false);
    }
}

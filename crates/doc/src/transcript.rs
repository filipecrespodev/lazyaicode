//! A conversa de uma thread como uma lista de itens para desenhar.
//!
//! Uma única função ([`Transcript::apply`]) para as duas origens: o stream ao
//! vivo do agente (stream-json do Claude Code, ou o ACP já traduzido) e o
//! histórico `.jsonl` da sessão, que é o mesmo stream sem os eventos parciais.
//!
//! As mensagens vêm de um processo externo e são tratadas como hostis: campos
//! ausentes ou de tipo errado viram valores vazios, nunca um pânico.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

/// Texto de um resultado de ferramenta guardado na transcrição (o resto é
/// cortado; um `cat` de 50 MB não pode travar a tela).
pub const MAX_TOOL_RESULT: usize = 256 * 1024;
/// Itens guardados por thread; acima disso os mais antigos saem.
pub const MAX_ITEMS: usize = 20_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResult {
    pub text: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Allow,
    Always,
    Deny,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Item {
    /// `images`: data URLs das imagens anexadas.
    User {
        key: String,
        text: String,
        images: Vec<String>,
    },
    Text {
        key: String,
        msg_id: String,
        text: String,
        is_final: bool,
    },
    Thinking {
        key: String,
        msg_id: String,
        text: String,
        is_final: bool,
    },
    Tool {
        key: String,
        id: String,
        name: String,
        input: Value,
        partial: String,
        result: Option<ToolResult>,
    },
    Permission {
        key: String,
        request_id: String,
        tool_name: String,
        tool_use_id: Option<String>,
        input: Value,
        reason: Option<String>,
        /// regras que o CLI sugere para "não perguntar de novo"
        suggestions: Vec<Value>,
        /// opções do agente ACP (allow_once, allow_always, …)
        options: Vec<Value>,
        decided: Option<Decision>,
    },
    Result {
        key: String,
        duration_ms: u64,
        cost_usd: f64,
        is_error: bool,
        /// o turno foi interrompido por quem usa (Esc), não falhou
        #[serde(default)]
        interrupted: bool,
        text: Option<String>,
    },
    Error {
        key: String,
        text: String,
    },
    Notice {
        key: String,
        text: String,
    },
}

impl Item {
    pub fn key(&self) -> &str {
        match self {
            Item::User { key, .. }
            | Item::Text { key, .. }
            | Item::Thinking { key, .. }
            | Item::Tool { key, .. }
            | Item::Permission { key, .. }
            | Item::Result { key, .. }
            | Item::Error { key, .. }
            | Item::Notice { key, .. } => key,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Transcript {
    pub items: Vec<Item>,
    /// bloco do stream (`index`) → chave do item, na mensagem em curso
    blocks: HashMap<i64, String>,
    current_msg: String,
    seq: u64,
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn truncate(mut t: String, max: usize) -> String {
    if t.len() > max {
        let mut cut = max;
        while cut > 0 && !t.is_char_boundary(cut) {
            cut -= 1;
        }
        t.truncate(cut);
        t.push_str("\n…");
    }
    t
}

/// Texto de um `content` (string ou lista de blocos).
pub fn content_text(c: &Value) -> String {
    match c {
        Value::String(t) => t.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| match s(&b["type"]) {
                "text" => Some(s(&b["text"]).to_string()),
                "image" => Some("[image]".to_string()),
                _ => None,
            })
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Imagens (base64) de uma mensagem do usuário, como data URLs.
fn content_images(c: &Value) -> Vec<String> {
    c.as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| s(&b["type"]) == "image" && s(&b["source"]["type"]) == "base64")
                .filter(|b| !s(&b["source"]["data"]).is_empty())
                .map(|b| format!("data:{};base64,{}", s(&b["source"]["media_type"]), s(&b["source"]["data"])))
                .collect()
        })
        .unwrap_or_default()
}

/// Retira os blocos que o CLI injeta e que não são do usuário.
/// O aviso que o CLI do Claude põe como fala do usuário ao interromper
/// (`[Request interrupted by user]`, `… for tool use]`): vira um
/// [`Item::Notice`] com este texto, que a tela traduz.
pub const INTERRUPTED_NOTICE: &str = "[Request interrupted by user]";

pub fn clean_user_text(t: &str) -> String {
    let mut out = t.to_string();
    for tag in ["system-reminder", "local-command-caveat"] {
        let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
        while let Some(a) = out.find(&open) {
            let Some(rel) = out.get(a..).and_then(|r| r.find(&close)) else { break };
            out.replace_range(a..a + rel + close.len(), "");
        }
    }
    out.trim().to_string()
}

impl Transcript {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_key(&mut self, p: &str) -> String {
        self.seq += 1;
        format!("{p}-{}", self.seq)
    }

    fn push(&mut self, it: Item) {
        self.items.push(it);
        if self.items.len() > MAX_ITEMS {
            let extra = self.items.len() - MAX_ITEMS;
            self.items.drain(..extra);
        }
    }

    fn tool_mut(&mut self, id: &str) -> Option<&mut Item> {
        self.items.iter_mut().find(|i| matches!(i, Item::Tool { id: x, .. } if x == id))
    }

    fn upsert_final(&mut self, msg_id: &str, thinking: bool, text: &str) {
        let open = self.items.iter_mut().find(|i| match i {
            Item::Text { msg_id: m, is_final, .. } if !thinking => m == msg_id && !*is_final,
            Item::Thinking { msg_id: m, is_final, .. } if thinking => m == msg_id && !*is_final,
            _ => false,
        });
        if let Some(Item::Text { text: t, is_final, .. } | Item::Thinking { text: t, is_final, .. }) = open {
            // o texto final é a fonte de verdade; o parcial só serviu para a animação
            if !text.is_empty() {
                *t = text.to_string();
            }
            *is_final = true;
            return;
        }
        if text.is_empty() {
            return;
        }
        let key = self.next_key(if thinking { "thinking" } else { "text" });
        let (msg_id, text) = (msg_id.to_string(), text.to_string());
        self.push(if thinking {
            Item::Thinking { key, msg_id, text, is_final: true }
        } else {
            Item::Text { key, msg_id, text, is_final: true }
        });
    }

    /// Aplica uma mensagem do agente (ou do histórico).
    pub fn apply(&mut self, msg: &Value) {
        if !msg["parent_tool_use_id"].is_null() && matches!(s(&msg["type"]), "stream_event" | "assistant" | "user") {
            return;
        }
        match s(&msg["type"]) {
            "stream_event" => self.stream_event(&msg["event"]),
            "assistant" => {
                let m = &msg["message"];
                let msg_id = match (m["id"].as_str(), msg["uuid"].as_str()) {
                    (Some(id), _) | (None, Some(id)) => id.to_string(),
                    _ => self.next_key("m"),
                };
                let Some(content) = m["content"].as_array() else { return };
                for b in content {
                    match s(&b["type"]) {
                        "text" => self.upsert_final(&msg_id, false, s(&b["text"])),
                        "thinking" => self.upsert_final(&msg_id, true, s(&b["thinking"])),
                        "tool_use" => {
                            let id = s(&b["id"]).to_string();
                            let input = if b["input"].is_object() { b["input"].clone() } else { Value::Object(Map::new()) };
                            if let Some(Item::Tool { input: i, partial, .. }) = self.tool_mut(&id) {
                                *i = input;
                                partial.clear();
                            } else {
                                let key = self.next_key("tool");
                                let name = s(&b["name"]).to_string();
                                self.push(Item::Tool { key, id, name, input, partial: String::new(), result: None });
                            }
                        }
                        _ => {}
                    }
                }
            }
            // o eco da mensagem que o CLI tirou da fila (`--replay-user-messages`):
            // quem mandou já a mostra
            "user" if msg["isReplay"] == true => {}
            // o que o CLI põe na boca do usuário para o modelo (o corpo de um
            // skill carregado, lembretes): ao vivo `isSynthetic`, no histórico `isMeta`
            "user" if msg["isSynthetic"] == true || msg["isMeta"] == true => {}
            "user" => {
                let c = &msg["message"]["content"];
                if let Some(blocks) = c.as_array().filter(|b| b.iter().any(|x| s(&x["type"]) == "tool_result")) {
                    for b in blocks.iter().filter(|b| s(&b["type"]) == "tool_result") {
                        let text = truncate(content_text(&b["content"]), MAX_TOOL_RESULT);
                        let is_error = b["is_error"].as_bool().unwrap_or(false);
                        if let Some(Item::Tool { result, .. }) = self.tool_mut(s(&b["tool_use_id"])) {
                            *result = Some(ToolResult { text, is_error });
                        }
                    }
                    return;
                }
                let images = content_images(c);
                let text = if images.is_empty() {
                    content_text(c)
                } else {
                    let only_text: Vec<Value> =
                        c.as_array().map(|a| a.iter().filter(|b| s(&b["type"]) != "image").cloned().collect()).unwrap_or_default();
                    content_text(&Value::Array(only_text))
                };
                let text = clean_user_text(&text);
                if images.is_empty() && text.starts_with("[Request interrupted by user") && text.ends_with(']') {
                    let key = msg["uuid"].as_str().map(str::to_string).unwrap_or_else(|| self.next_key("note"));
                    self.push(Item::Notice { key, text: INTERRUPTED_NOTICE.to_string() });
                    return;
                }
                if !text.is_empty() || !images.is_empty() {
                    let key = msg["uuid"].as_str().map(str::to_string).unwrap_or_else(|| self.next_key("user"));
                    self.push(Item::User { key, text, images });
                }
            }
            "control_request" => {
                let r = &msg["request"];
                if s(&r["subtype"]) != "can_use_tool" {
                    return;
                }
                let key = self.next_key("perm");
                let reason = r["decision_reason"].as_str().or_else(|| r["blocked_path"].as_str()).map(str::to_string);
                self.push(Item::Permission {
                    key,
                    request_id: s(&msg["request_id"]).to_string(),
                    tool_name: s(&r["tool_name"]).to_string(),
                    tool_use_id: r["tool_use_id"].as_str().map(str::to_string),
                    input: if r["input"].is_object() { r["input"].clone() } else { Value::Object(Map::new()) },
                    reason,
                    suggestions: r["permission_suggestions"].as_array().cloned().unwrap_or_default(),
                    options: r["options"].as_array().cloned().unwrap_or_default(),
                    decided: None,
                });
            }
            "control_cancel_request" => {
                let rid = s(&msg["request_id"]);
                for i in &mut self.items {
                    if let Item::Permission { request_id, decided: d @ None, .. } = i {
                        if request_id == rid {
                            *d = Some(Decision::Cancelled);
                        }
                    }
                }
            }
            "lazy_notice" => {
                let key = self.next_key("note");
                let text = match &msg["text"] {
                    Value::String(t) => t.clone(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                };
                self.push(Item::Notice { key, text });
            }
            "result" => {
                for i in &mut self.items {
                    match i {
                        // permissões pendentes morrem com o turno
                        Item::Permission { decided: d @ None, .. } => *d = Some(Decision::Cancelled),
                        Item::Text { is_final, .. } | Item::Thinking { is_final, .. } => *is_final = true,
                        _ => {}
                    }
                }
                let interrupted = was_interrupted(msg);
                let is_error = msg["is_error"].as_bool().unwrap_or(false) && !interrupted;
                let text = is_error.then(|| match msg["result"].as_str() {
                    Some(r) => r.to_string(),
                    None => msg["errors"]
                        .as_array()
                        .map(|e| e.iter().map(|x| x.as_str().map_or_else(|| x.to_string(), str::to_string)).collect::<Vec<_>>().join("\n"))
                        .unwrap_or_default(),
                });
                let key = self.next_key("res");
                self.push(Item::Result {
                    key,
                    duration_ms: msg["duration_ms"].as_u64().unwrap_or(0),
                    cost_usd: msg["total_cost_usd"].as_f64().filter(|c| c.is_finite()).unwrap_or(0.0),
                    is_error,
                    interrupted,
                    text,
                });
            }
            _ => {}
        }
    }

    fn stream_event(&mut self, ev: &Value) {
        let index = ev["index"].as_i64().unwrap_or(-1);
        match s(&ev["type"]) {
            "message_start" => {
                self.current_msg = match ev["message"]["id"].as_str() {
                    Some(id) => id.to_string(),
                    None => self.next_key("m"),
                };
                self.blocks.clear();
            }
            "content_block_start" => {
                let b = &ev["content_block"];
                match s(&b["type"]) {
                    t @ ("text" | "thinking") => {
                        let key = self.next_key(t);
                        let text = b["text"].as_str().or_else(|| b["thinking"].as_str()).unwrap_or("").to_string();
                        let msg_id = self.current_msg.clone();
                        self.push(if t == "text" {
                            Item::Text { key: key.clone(), msg_id, text, is_final: false }
                        } else {
                            Item::Thinking { key: key.clone(), msg_id, text, is_final: false }
                        });
                        self.blocks.insert(index, key);
                    }
                    "tool_use" => {
                        let id = s(&b["id"]).to_string();
                        if self.tool_mut(&id).is_none() {
                            let key = self.next_key("tool");
                            let name = s(&b["name"]).to_string();
                            self.push(Item::Tool {
                                key,
                                id: id.clone(),
                                name,
                                input: Value::Object(Map::new()),
                                partial: String::new(),
                                result: None,
                            });
                        }
                        self.blocks.insert(index, id);
                    }
                    _ => {}
                }
            }
            "content_block_delta" => {
                let Some(key) = self.blocks.get(&index).cloned() else { return };
                let d = &ev["delta"];
                match s(&d["type"]) {
                    "text_delta" | "thinking_delta" => {
                        let add = d["text"].as_str().or_else(|| d["thinking"].as_str()).unwrap_or("");
                        if let Some(Item::Text { text, .. } | Item::Thinking { text, .. }) =
                            self.items.iter_mut().rev().find(|i| i.key() == key)
                        {
                            text.push_str(add);
                        }
                    }
                    "input_json_delta" => {
                        if let Some(Item::Tool { partial, .. }) = self.tool_mut(&key) {
                            if partial.len() < MAX_TOOL_RESULT {
                                partial.push_str(s(&d["partial_json"]));
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Mensagem do usuário mostrada antes de o agente ecoá-la.
    pub fn add_local_user(&mut self, text: &str, images: Vec<String>) {
        let key = self.next_key("local");
        self.push(Item::User { key, text: text.to_string(), images });
    }

    pub fn add_error(&mut self, text: &str) {
        let key = self.next_key("err");
        self.push(Item::Error { key, text: text.to_string() });
    }

    /// Marca a decisão tomada num pedido de permissão.
    pub fn decide(&mut self, request_id: &str, decision: Decision) {
        for i in &mut self.items {
            if let Item::Permission { request_id: r, decided, .. } = i {
                if r == request_id {
                    *decided = Some(decision);
                }
            }
        }
    }

    /// Pedido de permissão ainda sem resposta (o primeiro).
    pub fn pending_permission(&self) -> Option<&Item> {
        self.items.iter().find(|i| matches!(i, Item::Permission { decided: None, .. }))
    }

    pub fn is_streaming(&self) -> bool {
        self.items.iter().any(|i| matches!(i, Item::Text { is_final: false, .. } | Item::Thinking { is_final: false, .. }))
    }
}

/// Uma linha que resume a chamada de ferramenta no cabeçalho do card.
pub fn tool_summary(name: &str, input: &Value) -> String {
    let pick = |k: &str| input[k].as_str().unwrap_or("").to_string();
    let or = |a: String, b: String| if a.is_empty() { b } else { a };
    match name {
        "Bash" => or(pick("description"), pick("command")),
        "Read" | "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => or(pick("file_path"), pick("notebook_path")),
        "Grep" | "Glob" => pick("pattern"),
        "WebFetch" => pick("url"),
        "WebSearch" => pick("query"),
        "Task" | "Agent" => pick("description"),
        "TodoWrite" => {
            let n = input["todos"].as_array().map_or(0, Vec::len);
            format!("{n} task{}", if n == 1 { "" } else { "s" })
        }
        // agentes ACP (Codex, Gemini) mandam um título pronto
        _ => or(
            pick("title"),
            input.as_object().and_then(|o| o.values().find_map(Value::as_str)).map(|v| v.chars().take(120).collect()).unwrap_or_default(),
        ),
    }
}

/// O `result` de um turno interrompido (Esc): o CLI do Claude o fecha com
/// `error_during_execution` e um `terminal_reason` `aborted_*` (e um
/// diagnóstico interno em `errors`) — é a parada pedida, não uma falha.
pub fn was_interrupted(result: &Value) -> bool {
    result["terminal_reason"].as_str().is_some_and(|r| r.starts_with("aborted"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn texts(t: &Transcript) -> Vec<String> {
        t.items
            .iter()
            .map(|i| match i {
                Item::User { text, .. } => format!("user:{text}"),
                Item::Text { text, is_final, .. } => format!("text:{text}:{is_final}"),
                Item::Thinking { text, .. } => format!("thinking:{text}"),
                Item::Tool { name, result, .. } => format!("tool:{name}:{}", result.as_ref().map_or("-", |r| r.text.as_str())),
                Item::Permission { tool_name, decided, .. } => format!("perm:{tool_name}:{decided:?}"),
                Item::Result { is_error, interrupted: true, .. } => format!("result:{is_error}:interrupted"),
                Item::Result { is_error, .. } => format!("result:{is_error}"),
                Item::Error { text, .. } => format!("error:{text}"),
                Item::Notice { text, .. } => format!("notice:{text}"),
            })
            .collect()
    }

    #[test]
    fn streams_text_then_the_final_message_replaces_it() {
        let mut t = Transcript::new();
        t.apply(&json!({"type":"stream_event","event":{"type":"message_start","message":{"id":"m1"}}}));
        t.apply(&json!({"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}}));
        t.apply(&json!({"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Ol"}}}));
        t.apply(&json!({"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"á"}}}));
        assert_eq!(texts(&t), vec!["text:Olá:false"]);
        assert!(t.is_streaming());
        t.apply(&json!({"type":"assistant","message":{"id":"m1","content":[{"type":"text","text":"Olá!"}]}}));
        assert_eq!(texts(&t), vec!["text:Olá!:true"]);
    }

    #[test]
    fn tool_calls_get_their_results_and_subagent_messages_are_ignored() {
        let mut t = Transcript::new();
        t.apply(&json!({"type":"assistant","message":{"id":"m","content":[{"type":"tool_use","id":"tu1","name":"Bash","input":{"command":"ls"}}]}}));
        t.apply(&json!({"type":"assistant","parent_tool_use_id":"x","message":{"id":"s","content":[{"type":"text","text":"sub"}]}}));
        t.apply(&json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tu1","content":[{"type":"text","text":"a.txt"}]}]}}));
        assert_eq!(texts(&t), vec!["tool:Bash:a.txt"]);
        assert_eq!(tool_summary("Bash", &json!({"command":"ls"})), "ls");
    }

    #[test]
    fn permissions_are_cancelled_when_the_turn_ends() {
        let mut t = Transcript::new();
        t.apply(&json!({"type":"control_request","request_id":"r1","request":{"subtype":"can_use_tool","tool_name":"Write","input":{}}}));
        assert!(t.pending_permission().is_some());
        t.apply(&json!({"type":"result","is_error":false,"duration_ms":10,"total_cost_usd":0.01}));
        assert_eq!(texts(&t), vec!["perm:Write:Some(Cancelled)", "result:false"]);
    }

    #[test]
    fn an_interrupted_turn_is_not_an_error() {
        let mut t = Transcript::new();
        t.apply(&json!({"type":"result","subtype":"error_during_execution","is_error":true,"terminal_reason":"aborted_streaming",
            "errors":["[ede_diagnostic] result_type=user last_content_type=n/a stop_reason=null"]}));
        t.apply(&json!({"type":"result","subtype":"error_during_execution","is_error":true,"errors":["falhou"]}));
        assert_eq!(texts(&t), vec!["result:false:interrupted", "result:true"]);
        let Some(Item::Result { text, .. }) = t.items.first() else { panic!("sem item") };
        assert_eq!(text, &None, "o diagnóstico interno não aparece");
        // o aviso que o CLI escreve como fala do usuário vira um aviso só
        for marker in ["[Request interrupted by user]", "[Request interrupted by user for tool use]"] {
            let mut t = Transcript::new();
            t.apply(&json!({"type":"user","uuid":"u9","message":{"content":[{"type":"text","text":marker}]}}));
            assert_eq!(texts(&t), vec![format!("notice:{INTERRUPTED_NOTICE}")]);
        }
    }

    #[test]
    fn what_the_cli_injects_as_the_user_is_not_shown() {
        // o corpo de um skill carregado: ao vivo `isSynthetic`, no histórico `isMeta`
        let mut t = Transcript::new();
        for flag in ["isSynthetic", "isMeta"] {
            t.apply(&json!({"type":"user", flag: true, "message":{"content":[{"type":"text","text":"Base directory for this skill: /x\n# Skill"}]}}));
        }
        assert!(t.items.is_empty(), "{:?}", texts(&t));
    }

    #[test]
    fn the_cli_echo_of_a_sent_message_is_not_shown_twice() {
        let mut t = Transcript::new();
        t.add_local_user("oi", vec![]);
        t.apply(&json!({"type":"user","isReplay":true,"uuid":"u1","message":{"content":[{"type":"text","text":"oi"}]}}));
        assert_eq!(texts(&t), vec!["user:oi"]);
    }

    #[test]
    fn user_text_drops_injected_blocks_and_keeps_images() {
        let mut t = Transcript::new();
        t.apply(&json!({"type":"user","uuid":"u1","message":{"content":[
            {"type":"text","text":"<system-reminder>x</system-reminder>oi"},
            {"type":"image","source":{"type":"base64","media_type":"image/png","data":"AAA"}}]}}));
        let Some(Item::User { text, images, .. }) = t.items.first() else { panic!("sem item") };
        assert_eq!(text, "oi");
        assert_eq!(images, &vec!["data:image/png;base64,AAA".to_string()]);
        assert_eq!(clean_user_text("<system-reminder>sem fim"), "<system-reminder>sem fim");
    }

    proptest::proptest! {
        /// Qualquer JSON que chegue do agente é aceito sem pânico.
        #[test]
        fn never_panics_on_hostile_messages(ty in "(stream_event|assistant|user|result|control_request|lazy_notice|x)",
                                            idx in -3i64..5, txt in ".{0,40}") {
            let mut t = Transcript::new();
            for m in [
                json!({"type": ty, "event": {"type":"content_block_delta","index": idx, "delta": {"type":"text_delta","text": txt}}}),
                json!({"type": ty, "message": {"content": txt}, "request": 3, "result": 5}),
                json!({"type": ty, "message": {"content": [{"type":"tool_result","tool_use_id": 1, "content": {"a": txt}}]}}),
                json!({"type": ty, "event": {"type":"content_block_start","index": idx, "content_block": {"type":"tool_use"}}}),
            ] {
                t.apply(&m);
            }
        }
    }
}

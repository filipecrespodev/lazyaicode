//! O que merece um aviso do sistema: um turno que terminou ou um pedido de
//! permissão. Só lógica sobre os eventos do motor, sem tela — a janela usa
//! e, no desktop, um observador em segundo plano também (com a janela
//! escondida não há quadros, e o aviso tem que sair assim mesmo).

use std::collections::HashMap;

use lazyaicode_i18n::{t, tf};
use lazyaicode_protocol::events;
use serde_json::Value;

/// Um aviso para a thread `thread`; o título leva o nome dela, que quem
/// mostra resolve (`✓ Título`, `⏸ Título`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub thread: String,
    pub kind: NoticeKind,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Finished,
    Permission,
}

impl Notice {
    pub fn title(&self, thread_title: &str) -> String {
        match self.kind {
            NoticeKind::Finished => format!("✓ {thread_title}"),
            NoticeKind::Permission => format!("⏸ {thread_title}"),
        }
    }
}

/// O último texto do agente em cada thread (o corpo do aviso de fim).
#[derive(Default)]
pub struct Attention {
    last_text: HashMap<String, String>,
}

/// Teto do corpo do aviso, em caracteres.
const BODY_MAX: usize = 160;

impl Attention {
    /// Lê um evento do motor; devolve o aviso, se for o caso.
    pub fn on_event(&mut self, event: &str, payload: &Value) -> Option<Notice> {
        if event != events::AGENT_EVENT {
            return None;
        }
        let tid = payload["threadId"].as_str()?.to_string();
        let m = &payload["msg"];
        match m["type"].as_str()? {
            "assistant" => {
                let text: String = m["message"]["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|b| b["type"] == "text")
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                if !text.trim().is_empty() {
                    self.last_text.insert(tid, text);
                }
                None
            }
            "result" => {
                let last = self.last_text.remove(&tid);
                let body = if m["is_error"].as_bool().unwrap_or(false) {
                    match m["result"].as_str() {
                        Some(r) => tf("app.finishedWithErrorDetail", &[("detail", &r.chars().take(120).collect::<String>())]),
                        None => t("app.finishedWithError"),
                    }
                } else {
                    last.map(|s| plain(&s)).filter(|s| !s.is_empty()).unwrap_or_else(|| t("app.done"))
                };
                Some(Notice { thread: tid, kind: NoticeKind::Finished, body })
            }
            "control_request" if m["request"]["subtype"] == "can_use_tool" => {
                let tool = m["request"]["tool_name"].as_str().unwrap_or("").to_string();
                Some(Notice { thread: tid, kind: NoticeKind::Permission, body: tf("app.needsPermission", &[("tool", &tool)]) })
            }
            _ => None,
        }
    }
}

/// Markdown virando texto corrido, cortado no teto.
fn plain(s: &str) -> String {
    s.replace(['#', '*', '`', '_', '>'], "").split_whitespace().collect::<Vec<_>>().join(" ").chars().take(BODY_MAX).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(tid: &str, msg: Value) -> (String, Value) {
        (events::AGENT_EVENT.to_string(), json!({ "threadId": tid, "msg": msg }))
    }

    #[test]
    fn a_finished_turn_carries_the_last_agent_text() {
        let mut a = Attention::default();
        let (e, p) = ev("t1", json!({"type":"assistant","message":{"content":[{"type":"text","text":"## Pronto\nTudo **certo**"}]}}));
        assert_eq!(a.on_event(&e, &p), None);
        let (e, p) = ev("t1", json!({"type":"result","is_error":false}));
        let n = a.on_event(&e, &p).unwrap();
        assert_eq!(n.kind, NoticeKind::Finished);
        assert_eq!(n.body, "Pronto Tudo certo");
        assert_eq!(n.title("Loja"), "✓ Loja");
        // o texto não vaza para o próximo turno
        let n = a.on_event(&e, &p).unwrap();
        assert_ne!(n.body, "Pronto Tudo certo");
    }

    #[test]
    fn a_permission_request_and_errors_are_notices_and_other_events_are_not() {
        let mut a = Attention::default();
        let (e, p) = ev("t2", json!({"type":"control_request","request":{"subtype":"can_use_tool","tool_name":"Bash"}}));
        let n = a.on_event(&e, &p).unwrap();
        assert_eq!((n.thread.as_str(), n.kind), ("t2", NoticeKind::Permission));
        assert!(n.body.contains("Bash"));
        let (e, p) = ev("t2", json!({"type":"result","is_error":true,"result":"falhou"}));
        assert!(a.on_event(&e, &p).unwrap().body.contains("falhou"));
        assert_eq!(a.on_event(events::THREAD_STATUS, &json!({"threadId":"t2","status":"idle"})), None);
        assert_eq!(a.on_event(events::AGENT_EVENT, &json!({"msg": {"type":"result"}})), None);
        let (e, p) = ev("t2", json!({"type":"stream_event"}));
        assert_eq!(a.on_event(&e, &p), None);
    }

    #[test]
    fn the_body_has_a_ceiling() {
        let mut a = Attention::default();
        let long = "palavra ".repeat(500);
        let (e, p) = ev("t3", json!({"type":"assistant","message":{"content":[{"type":"text","text": long}]}}));
        a.on_event(&e, &p);
        let (e, p) = ev("t3", json!({"type":"result"}));
        assert!(a.on_event(&e, &p).unwrap().body.chars().count() <= BODY_MAX);
    }
}

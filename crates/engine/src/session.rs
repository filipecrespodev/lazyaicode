//! Ponte entre os processos dos agentes e o motor: cada linha que um agente
//! escreve passa por [`make_sink`], que atualiza a thread (sessão, status,
//! não lida), grava a transcrição das threads ACP e publica o evento.

use std::path::PathBuf;
use std::sync::Arc;

use lazyaicode_agent::agent::{AgentOut, Sink};
use lazyaicode_doc::Status;
use lazyaicode_protocol::events;
use serde_json::{json, Value};

use crate::Engine;

impl Engine {
    /// Transcrição das threads ACP (o app grava; não há .jsonl comum entre
    /// os provedores).
    pub(crate) fn acp_transcript(&self, id: &str) -> PathBuf {
        self.core.data_dir.join("transcripts").join(format!("{id}.jsonl"))
    }

    pub(crate) fn append_transcript(&self, id: &str, msg: &Value) {
        use std::io::Write;
        let path = self.acp_transcript(id);
        if let Some(p) = path.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{msg}");
        }
    }

    pub(crate) fn make_sink(&self) -> Sink {
        let weak = Arc::downgrade(&self.core);
        Arc::new(move |tid: &str, out: AgentOut| {
            let Some(core) = weak.upgrade() else { return };
            let app = Engine { core };
            match out {
                AgentOut::Message(msg) => app.on_agent_message(tid, msg),
                AgentOut::Exit { code, stderr } => {
                    app.core.permissions.lock().remove(tid);
                    let running = app.status_of(tid) != Status::Idle;
                    let failed = code.is_some_and(|c| c != 0) && running;
                    app.set_status(tid, if failed { Status::Error } else { Status::Idle });
                    app.emit(
                        events::AGENT_EXIT,
                        json!({ "threadId": tid, "code": code, "stderr": if failed { stderr } else { String::new() } }),
                    );
                }
            }
        })
    }

    fn on_agent_message(&self, tid: &str, msg: Value) {
        if self.core.acp.is_running(tid) && matches!(msg["type"].as_str(), Some("assistant" | "user" | "result" | "lazy_notice")) {
            self.append_transcript(tid, &msg);
        }
        // o que muda na thread; o status sai depois do evento, para quem
        // espera o fim do turno já ter recebido o `result`
        let mut next_status = None;
        match (msg["type"].as_str(), msg["subtype"].as_str()) {
            (Some("system"), Some("init")) => {
                if let Some(sid) = msg["session_id"].as_str() {
                    let sid = sid.to_string();
                    let _ = self.update_thread(tid, |t| {
                        if let Some(old) = t.session_id.take() {
                            if old != sid && !t.past_sessions.contains(&old) {
                                t.past_sessions.push(old);
                            }
                        }
                        t.session_id = Some(sid);
                    });
                }
            }
            (Some("result"), _) => {
                if let Ok(t) = self.update_thread(tid, |t| {
                    t.unread = true;
                    t.updated_at = crate::store::now();
                }) {
                    self.emit(events::THREAD_UPDATED, json!(self.view(&t)));
                }
                self.core.permissions.lock().remove(tid);
                let failed = msg["is_error"].as_bool().unwrap_or(false);
                next_status = Some(if failed { Status::Error } else { Status::Idle });
            }
            (Some("control_request"), _) if msg["request"]["subtype"] == "can_use_tool" => {
                let r = &msg["request"];
                self.core.permissions.lock().insert(
                    tid.to_string(),
                    json!({
                        "requestId": msg["request_id"],
                        "toolName": r["tool_name"],
                        "input": r["input"],
                        "toolUseId": r["tool_use_id"],
                        "suggestions": r["permission_suggestions"],
                    }),
                );
                next_status = Some(Status::Awaiting);
            }
            _ => {}
        }
        self.emit(events::AGENT_EVENT, json!({ "threadId": tid, "msg": msg }));
        if let Some(s) = next_status {
            self.set_status(tid, s);
        }
    }
}

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

/// Se o CLI tirou da fila tudo o que recebeu. Com `--replay-user-messages`
/// ele devolve cada mensagem ao começar a respondê-la — as que chegam
/// durante um turno são juntadas num eco só, com os blocos de todas —, então
/// o fim de um turno com blocos ainda na fila não é o fim: vem outro.
#[derive(Debug, Default)]
pub struct Queue {
    /// blocos (texto, imagem) mandados e ainda não devolvidos
    pub pending: u64,
    /// este processo devolve as mensagens (sem eco, vale o fim do turno)
    pub replays: bool,
    /// mensagens do agente até agora (o vigia vê se algo chegou)
    pub seen: u64,
}

/// Quanto o fim de um turno espera o próximo começar antes de dar a thread
/// por parada (o eco que não veio, uma mensagem que o CLI descartou).
const NEXT_TURN_GRACE: std::time::Duration = std::time::Duration::from_secs(15);

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
                    app.core.queued.lock().remove(tid);
                    app.core.interrupted.lock().remove(tid);
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

    /// Depois de um turno com mensagem na fila: se o CLI não disser mais nada
    /// em [`NEXT_TURN_GRACE`], a thread para (nunca fica "rodando" à toa).
    fn watch_next_turn(&self, tid: &str, seen: u64) {
        let Ok(rt) = tokio::runtime::Handle::try_current() else { return };
        let weak = Arc::downgrade(&self.core);
        let tid = tid.to_string();
        rt.spawn(async move {
            tokio::time::sleep(NEXT_TURN_GRACE).await;
            let Some(core) = weak.upgrade() else { return };
            let app = Engine { core };
            let quiet = app.core.queued.lock().get_mut(&tid).filter(|q| q.seen == seen).map(|q| q.pending = 0).is_some();
            if quiet && app.status_of(&tid) == Status::Running {
                app.set_status(&tid, Status::Idle);
            }
        });
    }

    fn on_agent_message(&self, tid: &str, msg: Value) {
        if self.core.acp.is_running(tid) && matches!(msg["type"].as_str(), Some("assistant" | "user" | "result" | "lazy_notice")) {
            self.append_transcript(tid, &msg);
        }
        // o que muda na thread; o status sai depois do evento, para quem
        // espera o fim do turno já ter recebido o `result`
        let mut next_status = None;
        if let Some(q) = self.core.queued.lock().get_mut(tid) {
            q.seen += 1;
        }
        if msg["type"] == "user" && msg["isReplay"] == true {
            let blocks = msg["message"]["content"].as_array().map_or(1, Vec::len) as u64;
            let mut queued = self.core.queued.lock();
            let q = queued.entry(tid.to_string()).or_default();
            q.pending = q.pending.saturating_sub(blocks);
            q.replays = true;
            drop(queued);
            // começou um turno (o da fila, mesmo depois de uma interrupção)
            if !matches!(self.status_of(tid), Status::Running | Status::Awaiting) {
                next_status = Some(Status::Running);
            }
        }
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
                let stopped = self.core.interrupted.lock().remove(tid) || lazyaicode_doc::transcript::was_interrupted(&msg);
                let failed = msg["is_error"].as_bool().unwrap_or(false) && !stopped;
                let more = match self.core.queued.lock().get_mut(tid) {
                    // a interrupção pode ter levado a fila junto; se não levou, o eco reabre
                    Some(q) if stopped => {
                        q.pending = 0;
                        None
                    }
                    Some(q) if q.replays && q.pending > 0 => Some(q.seen),
                    _ => None,
                };
                next_status = Some(match more {
                    Some(seen) => {
                        self.watch_next_turn(tid, seen);
                        Status::Running
                    }
                    None if failed => Status::Error,
                    None => Status::Idle,
                });
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
            // o CLI desistiu do pedido (interrupção, hook): ninguém mais responde a ele
            (Some("control_cancel_request"), _) => {
                let cancelled = {
                    let mut perms = self.core.permissions.lock();
                    let hit = perms.get(tid).is_some_and(|p| p["requestId"] == msg["request_id"]);
                    if hit {
                        perms.remove(tid);
                    }
                    hit
                };
                if cancelled && self.status_of(tid) == Status::Awaiting {
                    next_status = Some(Status::Running);
                }
            }
            _ => {}
        }
        self.emit(events::AGENT_EVENT, json!({ "threadId": tid, "msg": msg }));
        if let Some(s) = next_status {
            self.set_status(tid, s);
        }
    }
}

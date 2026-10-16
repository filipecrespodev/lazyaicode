//! Como a UI fala com o motor. A UI nunca chama o motor direto: manda um
//! método e recebe a resposta depois (no próximo quadro), com os eventos
//! no meio. O app desktop usa um cliente local (o motor no mesmo processo,
//! `apps/lazyaicode`); a versão web e o desktop ligado a um servidor usam
//! o [`WsClient`], que fala o canal de controle por WebSocket.

use std::collections::VecDeque;

use lazyaicode_protocol::{decode_line, encode_line, ErrorBody, Event, Frame, Request};
use serde_json::{json, Value};

pub type Ticket = u64;

#[derive(Debug)]
pub enum Incoming {
    Reply(Ticket, Result<Value, ErrorBody>),
    Event(Event),
    /// O cliente conectou (ou reconectou) e está pronto.
    Connected,
    /// Perdeu a conexão com o motor (só clientes remotos).
    Disconnected(String),
}

pub trait Client {
    /// Manda um método; a resposta chega em [`Client::poll`] com o mesmo ticket.
    fn call(&mut self, method: &str, params: Value) -> Ticket;
    /// O que chegou desde a última chamada.
    fn poll(&mut self) -> Vec<Incoming>;
    /// `local` ou o endereço do servidor (para a barra de status).
    fn describe(&self) -> String;
}

/// Cliente de mentira para testes de UI: responde o que foi programado.
#[derive(Default)]
pub struct MockClient {
    next: Ticket,
    pub calls: Vec<(String, Value)>,
    pub answers: std::collections::HashMap<String, Value>,
    queue: VecDeque<Incoming>,
}

impl MockClient {
    pub fn answer(mut self, method: &str, v: Value) -> Self {
        self.answers.insert(method.into(), v);
        self
    }

    pub fn push_event(&mut self, event: &str, payload: Value) {
        self.queue.push_back(Incoming::Event(Event { event: event.into(), payload }));
    }
}

impl Client for MockClient {
    fn call(&mut self, method: &str, params: Value) -> Ticket {
        self.next += 1;
        self.calls.push((method.to_string(), params));
        let r = self.answers.get(method).cloned().ok_or_else(|| ErrorBody {
            code: "unknown_method".into(),
            message: format!("mock sem resposta para {method}"),
            data: None,
        });
        self.queue.push_back(Incoming::Reply(self.next, r));
        self.next
    }

    fn poll(&mut self) -> Vec<Incoming> {
        self.queue.drain(..).collect()
    }

    fn describe(&self) -> String {
        "mock".into()
    }
}

/// O canal de controle por WebSocket (versão web, ou desktop remoto).
pub struct WsClient {
    url: String,
    token: String,
    sender: Option<ewebsock::WsSender>,
    receiver: Option<ewebsock::WsReceiver>,
    next: Ticket,
    /// pedidos feitos antes de a conexão abrir
    backlog: Vec<String>,
    open: bool,
    wake: std::sync::Arc<dyn Fn() + Send + Sync>,
}

impl WsClient {
    /// `wake` acorda a UI quando chega mensagem (`ctx.request_repaint`).
    pub fn connect(url: &str, token: &str, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let mut c = Self {
            url: url.into(),
            token: token.into(),
            sender: None,
            receiver: None,
            next: 1000,
            backlog: vec![],
            open: false,
            wake: std::sync::Arc::new(wake),
        };
        c.reconnect();
        c
    }

    fn reconnect(&mut self) {
        let wake = self.wake.clone();
        let opts = ewebsock::Options { max_incoming_frame_size: lazyaicode_protocol::MAX_REPLY_BYTES, ..Default::default() };
        match ewebsock::connect_with_wakeup(self.url.clone(), opts, move || wake()) {
            Ok((s, r)) => {
                self.sender = Some(s);
                self.receiver = Some(r);
            }
            Err(e) => log::warn!("websocket: {e}"),
        }
        self.open = false;
    }

    fn send_raw(&mut self, line: String) {
        match (&mut self.sender, self.open) {
            (Some(s), true) => s.send(ewebsock::WsMessage::Text(line.trim_end().to_string())),
            _ => self.backlog.push(line),
        }
    }
}

impl Client for WsClient {
    fn call(&mut self, method: &str, params: Value) -> Ticket {
        self.next += 1;
        let line = encode_line(&Request { id: json!(self.next), method: method.into(), params });
        self.send_raw(line);
        self.next
    }

    fn poll(&mut self) -> Vec<Incoming> {
        let mut out = Vec::new();
        let Some(rx) = &self.receiver else { return out };
        let mut events = Vec::new();
        while let Some(ev) = rx.try_recv() {
            events.push(ev);
        }
        for ev in events {
            match ev {
                ewebsock::WsEvent::Opened => {
                    self.open = true;
                    let auth = encode_line(&Request { id: json!(1), method: "auth".into(), params: json!({ "token": self.token }) });
                    let sub = encode_line(&Request { id: json!(2), method: "events.subscribe".into(), params: json!({ "events": [] }) });
                    if let Some(s) = &mut self.sender {
                        s.send(ewebsock::WsMessage::Text(auth));
                        s.send(ewebsock::WsMessage::Text(sub));
                        for l in self.backlog.drain(..) {
                            s.send(ewebsock::WsMessage::Text(l));
                        }
                    }
                    out.push(Incoming::Connected);
                }
                ewebsock::WsEvent::Message(ewebsock::WsMessage::Text(t)) => match decode_line::<Frame>(&t) {
                    Ok(Frame::Event(e)) => out.push(Incoming::Event(e)),
                    Ok(Frame::Reply(r)) => {
                        let id = r.id.as_u64().unwrap_or(0);
                        if id == 1 {
                            if let Some(e) = r.error {
                                out.push(Incoming::Disconnected(format!("{}: {}", e.code, e.message)));
                            }
                        } else if id > 2 {
                            out.push(Incoming::Reply(id, r.into_result()));
                        }
                    }
                    Err(e) => log::warn!("quadro inválido: {e}"),
                },
                ewebsock::WsEvent::Message(_) => {}
                ewebsock::WsEvent::Error(e) => {
                    self.open = false;
                    out.push(Incoming::Disconnected(e));
                }
                ewebsock::WsEvent::Closed => {
                    self.open = false;
                    out.push(Incoming::Disconnected("conexão fechada".into()));
                }
            }
        }
        out
    }

    fn describe(&self) -> String {
        self.url.clone()
    }
}

impl WsClient {
    /// Tenta de novo depois de cair (o botão "reconectar").
    pub fn retry(&mut self) {
        self.reconnect();
    }
}

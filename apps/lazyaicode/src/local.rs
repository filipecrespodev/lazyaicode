//! O cliente da UI com o motor no mesmo processo: cada chamada vira uma
//! task no runtime do tokio; a resposta e os eventos chegam por canais que a
//! UI esvazia a cada quadro.

use std::sync::mpsc;
use std::sync::{Arc, OnceLock};

use lazyaicode_automation::backend::run_engine;
use lazyaicode_engine::Engine;
use lazyaicode_protocol::{ErrorBody, Event};
use lazyaicode_ui_egui::{Client, Incoming, Ticket};
use serde_json::Value;
use tokio::sync::broadcast;

pub type Wake = Arc<OnceLock<egui::Context>>;

pub struct LocalClient {
    engine: Engine,
    rt: tokio::runtime::Handle,
    next: Ticket,
    tx: mpsc::Sender<(Ticket, Result<Value, ErrorBody>)>,
    rx: mpsc::Receiver<(Ticket, Result<Value, ErrorBody>)>,
    events: broadcast::Receiver<Event>,
    wake: Wake,
    connected_sent: bool,
}

impl LocalClient {
    pub fn new(engine: Engine, rt: tokio::runtime::Handle, wake: Wake) -> Self {
        let (tx, rx) = mpsc::channel();
        let events = engine.subscribe();
        // eventos acordam a UI (o stream do agente chega com a janela parada)
        let mut sub = engine.subscribe();
        let w = wake.clone();
        rt.spawn(async move {
            loop {
                match sub.recv().await {
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {
                        if let Some(ctx) = w.get() {
                            ctx.request_repaint();
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => return,
                }
            }
        });
        Self { engine, rt, next: 0, tx, rx, events, wake, connected_sent: false }
    }
}

impl Client for LocalClient {
    fn call(&mut self, method: &str, params: Value) -> Ticket {
        self.next += 1;
        let ticket = self.next;
        let (engine, tx, wake, method) = (self.engine.clone(), self.tx.clone(), self.wake.clone(), method.to_string());
        self.rt.spawn(async move {
            let r = run_engine(&engine, &method, params).await;
            let _ = tx.send((ticket, r));
            if let Some(ctx) = wake.get() {
                ctx.request_repaint();
            }
        });
        ticket
    }

    fn poll(&mut self) -> Vec<Incoming> {
        let mut out = Vec::new();
        if !self.connected_sent {
            self.connected_sent = true;
            out.push(Incoming::Connected);
        }
        while let Ok((t, r)) = self.rx.try_recv() {
            out.push(Incoming::Reply(t, r));
        }
        loop {
            match self.events.try_recv() {
                Ok(e) => out.push(Incoming::Event(e)),
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
        out
    }

    fn describe(&self) -> String {
        "local".into()
    }
}

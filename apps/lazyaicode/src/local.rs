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
    /// eventos repassados do motor sem limite: com a janela escondida (sem
    /// quadros, ninguém chama `poll`) o canal do motor transbordaria e os
    /// mais antigos se perderiam
    events: mpsc::Receiver<Event>,
    wake: Wake,
    /// a janela que esta cliente acorda (cada janela tem a sua)
    viewport: egui::ViewportId,
    connected_sent: bool,
}

impl LocalClient {
    pub fn new(engine: Engine, rt: tokio::runtime::Handle, wake: Wake, viewport: egui::ViewportId) -> Self {
        let (tx, rx) = mpsc::channel();
        let (etx, events) = mpsc::channel();
        // repassa cada evento e acorda a UI (o stream do agente chega com a janela parada)
        let mut sub = engine.subscribe();
        let w = wake.clone();
        rt.spawn(async move {
            loop {
                match sub.recv().await {
                    Ok(e) => {
                        if etx.send(e).is_err() {
                            return; // a janela fechou
                        }
                        if let Some(ctx) = w.get() {
                            ctx.request_repaint_of(viewport);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => log::warn!("{n} eventos do motor perdidos"),
                    Err(broadcast::error::RecvError::Closed) => return,
                }
            }
        });
        Self { engine, rt, next: 0, tx, rx, events, wake, viewport, connected_sent: false }
    }
}

impl Client for LocalClient {
    fn call(&mut self, method: &str, params: Value) -> Ticket {
        self.next += 1;
        let ticket = self.next;
        let (engine, tx, wake, method, vid) = (self.engine.clone(), self.tx.clone(), self.wake.clone(), method.to_string(), self.viewport);
        self.rt.spawn(async move {
            let r = run_engine(&engine, &method, params).await;
            let _ = tx.send((ticket, r));
            if let Some(ctx) = wake.get() {
                ctx.request_repaint_of(vid);
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
        while let Ok(e) = self.events.try_recv() {
            out.push(Incoming::Event(e));
        }
        out
    }

    fn describe(&self) -> String {
        "local".into()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use lazyaicode_engine::EngineOpts;

    /// Com a janela escondida ninguém chama `poll` por minutos; o agente
    /// segue transmitindo. Nada pode se perder (antes, passado o tamanho do
    /// canal do motor, os eventos mais antigos sumiam).
    #[test]
    fn events_are_kept_while_nobody_polls() {
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        let engine = {
            let _g = rt.enter();
            Engine::new(EngineOpts { data_dir: None, ephemeral: true })
        };
        let mut client = LocalClient::new(engine.clone(), rt.handle().clone(), Arc::new(OnceLock::new()), egui::ViewportId::ROOT);
        let n = 20_000;
        for i in 0..n {
            engine.emit("test-event", serde_json::json!({ "i": i }));
            // o agente transmite aos poucos, não tudo no mesmo instante
            if i % 500 == 0 {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
        let mut got = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while got.len() < n && std::time::Instant::now() < deadline {
            for inc in client.poll() {
                if let Incoming::Event(e) = inc {
                    if e.event == "test-event" {
                        got.push(e.payload["i"].as_u64().unwrap());
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(got.len(), n, "eventos perdidos");
        assert!(got.windows(2).all(|w| w[0] + 1 == w[1]), "fora de ordem");
        rt.block_on(engine.shutdown());
    }
}

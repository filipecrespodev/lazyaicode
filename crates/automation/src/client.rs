//! Cliente do canal de controle: a ponte do MCP para um app aberto, a CLI
//! remota (`lazyaicode-cli --connect`) e os testes.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use lazyaicode_protocol::{decode_line, encode_line, ErrorBody, Event, Frame, Request};
use parking_lot::Mutex;
use serde_json::{json, Value};
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::backend::{Backend, BoxFut};
use crate::server::read_limited_line;

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, ErrorBody>>>>>;

#[derive(Clone)]
pub struct ControlClient {
    out: mpsc::UnboundedSender<String>,
    pending: Pending,
    next: Arc<AtomicU64>,
    events: broadcast::Sender<Event>,
}

fn closed() -> ErrorBody {
    ErrorBody { code: "closed".into(), message: "a conexão com o app fechou".into(), data: None }
}

impl ControlClient {
    /// Conecta e autentica. Só aceita endereços de loopback.
    pub async fn connect(addr: SocketAddr, token: &str) -> std::io::Result<Self> {
        if !addr.ip().is_loopback() {
            return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "a ponte só conecta em loopback"));
        }
        let stream = tokio::time::timeout(Duration::from_secs(10), TcpStream::connect(addr))
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "o app não respondeu"))??;
        let _ = stream.set_nodelay(true);
        let (r, mut w) = stream.into_split();
        let (out, mut rx) = mpsc::unbounded_channel::<String>();
        tokio::spawn(async move {
            while let Some(line) = rx.recv().await {
                if w.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
            }
        });
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let (events, _) = broadcast::channel(4096);
        let (p, ev) = (pending.clone(), events.clone());
        tokio::spawn(async move {
            let mut reader = BufReader::new(r);
            let mut buf = Vec::new();
            while let Ok(Some(ok)) = read_limited_line(&mut reader, &mut buf).await {
                if !ok {
                    continue;
                }
                match decode_line::<Frame>(String::from_utf8_lossy(&buf).trim()) {
                    Ok(Frame::Event(e)) => {
                        let _ = ev.send(e);
                    }
                    Ok(Frame::Reply(r)) => {
                        let tx = r.id.as_u64().and_then(|id| p.lock().remove(&id));
                        if let Some(tx) = tx {
                            let _ = tx.send(r.into_result());
                        }
                    }
                    Err(_) => {}
                }
            }
            for (_, tx) in p.lock().drain() {
                let _ = tx.send(Err(closed()));
            }
        });
        let c = Self { out, pending, next: Arc::new(AtomicU64::new(1)), events };
        c.call("auth", json!({ "token": token }))
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::PermissionDenied, format!("{}: {}", e.code, e.message)))?;
        Ok(c)
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<Value, ErrorBody> {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().insert(id, tx);
        let line = encode_line(&Request { id: json!(id), method: method.into(), params });
        if self.out.send(line).is_err() {
            self.pending.lock().remove(&id);
            return Err(closed());
        }
        rx.await.unwrap_or_else(|_| Err(closed()))
    }

    /// Assina eventos (vazio = todos) e devolve o receptor.
    pub async fn subscribe(&self, names: &[&str]) -> Result<broadcast::Receiver<Event>, ErrorBody> {
        let rx = self.events.subscribe();
        self.call("events.subscribe", json!({ "events": names })).await?;
        Ok(rx)
    }
}

/// A ponte: o MCP fala com um app aberto como se fosse o motor.
impl Backend for ControlClient {
    fn call(&self, method: String, params: Value) -> BoxFut<Result<Value, ErrorBody>> {
        let me = self.clone();
        Box::pin(async move { ControlClient::call(&me, &method, params).await })
    }

    fn events(&self) -> broadcast::Receiver<Event> {
        let rx = self.events.subscribe();
        let me = self.clone();
        tokio::spawn(async move {
            let _ = me.call("events.subscribe", json!({ "events": [] })).await;
        });
        rx
    }
}

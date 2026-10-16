//! O canal de controle: JSON delimitado por linha sobre TCP em loopback.
//!
//! O primeiro quadro de toda conexão é `{"id":…,"method":"auth","params":{"token":"…"}}`;
//! um token errado fecha a conexão. Depois, cada pedido recebe uma resposta
//! com o mesmo `id`, e quem fizer `events.subscribe` recebe também os eventos
//! do motor. Pedidos são atendidos em paralelo: um `thread.send` longo não
//! segura um `engine.state`.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use lazyaicode_protocol::{decode_line, encode_line, Event, Reply, Request, MAX_CONNECTIONS, MAX_REPLY_BYTES, MAX_REQUEST_LINE};
use parking_lot::Mutex;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};

use crate::backend::{protocol_methods, Backend};
use crate::token;

/// Quanto um pedido pode levar até a resposta virar `timeout`.
pub const HANDLER_TIMEOUT: Duration = Duration::from_secs(600);

/// Estado de uma conexão: autenticada? assinando quais eventos?
pub struct Conn {
    backend: Arc<dyn Backend>,
    token: Option<String>,
    authed: Mutex<bool>,
    /// `None` = não assina; `Some(vec![])` = todos.
    subscribed: Arc<Mutex<Option<Vec<String>>>>,
    out: mpsc::UnboundedSender<String>,
}

pub enum Step {
    /// Resposta já pronta (ou nada a responder).
    Done(Option<Reply>),
    /// Fechar a conexão depois de mandar a resposta.
    Close(Reply),
}

impl Conn {
    pub fn new(backend: Arc<dyn Backend>, token: Option<String>, out: mpsc::UnboundedSender<String>) -> Arc<Self> {
        let authed = token.is_none();
        let c = Arc::new(Self { backend, token, authed: Mutex::new(authed), subscribed: Arc::new(Mutex::new(None)), out });
        c.forward_events();
        c
    }

    fn forward_events(self: &Arc<Self>) {
        let mut rx = self.backend.events();
        let subscribed = self.subscribed.clone();
        let out = self.out.clone();
        tokio::spawn(async move {
            loop {
                let ev = match rx.recv().await {
                    Ok(ev) => ev,
                    Err(broadcast::error::RecvError::Lagged(n)) => Event { event: "events-lagged".into(), payload: json!({ "missed": n }) },
                    Err(broadcast::error::RecvError::Closed) => return,
                };
                let wanted = subscribed.lock().as_ref().is_some_and(|f| f.is_empty() || f.iter().any(|e| *e == ev.event));
                if wanted && out.send(encode_line(&ev)).is_err() {
                    return;
                }
            }
        });
    }

    fn send(&self, r: &Reply) {
        let mut line = encode_line(r);
        if line.len() > MAX_REPLY_BYTES {
            line = encode_line(&Reply::err(
                r.id.clone(),
                "too_large",
                format!("resposta de {} bytes passa do limite de {MAX_REPLY_BYTES}", line.len()),
            ));
        }
        let _ = self.out.send(line);
    }

    /// Processa uma linha recebida. Devolve `false` para fechar a conexão.
    pub fn handle_line(self: &Arc<Self>, line: &str) -> bool {
        let line = line.trim();
        if line.is_empty() {
            return true;
        }
        let req: Request = match decode_line(line) {
            Ok(r) => r,
            Err(e) => {
                self.send(&Reply::err(Value::Null, "bad_request", e.to_string()));
                return true;
            }
        };
        match self.step(req) {
            Step::Done(Some(r)) => {
                self.send(&r);
                true
            }
            Step::Done(None) => true,
            Step::Close(r) => {
                self.send(&r);
                false
            }
        }
    }

    fn step(self: &Arc<Self>, req: Request) -> Step {
        let id = req.id.clone();
        if !*self.authed.lock() {
            if req.method != "auth" {
                return Step::Close(Reply::err(id, "unauthorized", "o primeiro quadro deve ser `auth`"));
            }
            let got = req.params.get("token").and_then(Value::as_str).unwrap_or("");
            let ok = self.token.as_deref().is_some_and(|t| token::matches(t, got));
            if !ok {
                return Step::Close(Reply::err(id, "unauthorized", "token inválido"));
            }
            *self.authed.lock() = true;
            return Step::Done(Some(Reply::ok(id, json!({ "ok": true }))));
        }
        match req.method.as_str() {
            "auth" => Step::Done(Some(Reply::ok(id, json!({ "ok": true })))),
            "hello" => Step::Done(Some(Reply::ok(
                id,
                json!({ "app": "LazyAICode", "version": env!("CARGO_PKG_VERSION"), "protocol": lazyaicode_protocol::PROTOCOL_VERSION }),
            ))),
            "methods" => {
                let mut m = protocol_methods();
                m.extend(self.backend.extra_methods());
                Step::Done(Some(Reply::ok(id, Value::Array(m))))
            }
            "events.subscribe" => {
                let filter: Vec<String> = req
                    .params
                    .get("events")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                *self.subscribed.lock() = Some(filter);
                Step::Done(Some(Reply::ok(id, json!({ "ok": true }))))
            }
            "events.unsubscribe" => {
                *self.subscribed.lock() = None;
                Step::Done(Some(Reply::ok(id, json!({ "ok": true }))))
            }
            _ => {
                let me = self.clone();
                tokio::spawn(async move {
                    let fut = me.backend.call(req.method.clone(), req.params);
                    let r = match tokio::time::timeout(HANDLER_TIMEOUT, fut).await {
                        Ok(Ok(v)) => Reply::ok(id, v),
                        Ok(Err(e)) => Reply { id, result: None, error: Some(e) },
                        Err(_) => Reply::err(id, "timeout", format!("{} não respondeu em {}s", req.method, HANDLER_TIMEOUT.as_secs())),
                    };
                    me.send(&r);
                });
                Step::Done(None)
            }
        }
    }
}

/// Lê linhas com teto de tamanho (uma linha gigante não cresce a memória).
pub async fn read_limited_line<R: AsyncBufReadExt + Unpin>(r: &mut R, buf: &mut Vec<u8>) -> std::io::Result<Option<bool>> {
    buf.clear();
    let n = (&mut *r).take(MAX_REQUEST_LINE as u64 + 1).read_until(b'\n', buf).await?;
    if n == 0 {
        return Ok(None);
    }
    if buf.len() > MAX_REQUEST_LINE && buf.last() != Some(&b'\n') {
        // descarta o resto da linha gigante
        loop {
            let mut skip = Vec::new();
            let m = (&mut *r).take(64 * 1024).read_until(b'\n', &mut skip).await?;
            if m == 0 || skip.last() == Some(&b'\n') {
                break;
            }
        }
        return Ok(Some(false));
    }
    Ok(Some(true))
}

/// Atende uma conexão qualquer (TCP, stdio) até o outro lado fechar.
pub async fn serve_stream<R, W>(read: R, mut write: W, backend: Arc<dyn Backend>, token: Option<String>)
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            if write.write_all(line.as_bytes()).await.is_err() || write.flush().await.is_err() {
                break;
            }
        }
    });
    let conn = Conn::new(backend, token, tx.clone());
    let mut reader = BufReader::new(read);
    let mut buf = Vec::new();
    loop {
        match read_limited_line(&mut reader, &mut buf).await {
            Ok(None) | Err(_) => break,
            Ok(Some(false)) => {
                let _ = tx.send(encode_line(&Reply::err(Value::Null, "too_large", format!("linha passa de {MAX_REQUEST_LINE} bytes"))));
            }
            Ok(Some(true)) => {
                let line = String::from_utf8_lossy(&buf);
                if !conn.handle_line(&line) {
                    break;
                }
            }
        }
    }
    drop(conn);
    drop(tx);
    let _ = tokio::time::timeout(Duration::from_secs(2), writer).await;
}

/// Escuta em `addr` (só loopback) e atende até o processo sair.
pub async fn serve_tcp(addr: SocketAddr, backend: Arc<dyn Backend>, token: String) -> std::io::Result<()> {
    if !addr.ip().is_loopback() {
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "o canal de controle só escuta em loopback"));
    }
    let listener = TcpListener::bind(addr).await?;
    serve_listener(listener, backend, token).await
}

pub async fn serve_listener(listener: TcpListener, backend: Arc<dyn Backend>, token: String) -> std::io::Result<()> {
    let active = Arc::new(AtomicUsize::new(0));
    loop {
        let (stream, _) = listener.accept().await?;
        if active.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
            drop(stream);
            continue;
        }
        let _ = stream.set_nodelay(true);
        active.fetch_add(1, Ordering::SeqCst);
        let (backend, token, active) = (backend.clone(), token.clone(), active.clone());
        tokio::spawn(async move {
            let (r, w) = stream.into_split();
            serve_stream(r, w, backend, Some(token)).await;
            active.fetch_sub(1, Ordering::SeqCst);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::EngineBackend;
    use crate::client::ControlClient;
    use lazyaicode_engine::{Engine, EngineOpts};

    async fn start() -> (SocketAddr, String, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let engine = Engine::new(EngineOpts { data_dir: Some(tmp.path().to_path_buf()), ephemeral: true });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let token = token::generate();
        tokio::spawn(serve_listener(listener, EngineBackend::new(engine), token.clone()));
        (addr, token, tmp)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn authenticates_then_runs_commands_and_streams_events() {
        let (addr, token, _tmp) = start().await;
        let c = ControlClient::connect(addr, &token).await.unwrap();
        assert_eq!(c.call("engine.ping", Value::Null).await.unwrap(), json!("pong"));
        let v = c.call("engine.run", json!({"command": "engine.version"})).await.unwrap();
        assert_eq!(v["protocol"], json!(1));
        let err = c.call("thread.get", json!({"id": "x"})).await.unwrap_err();
        assert_eq!(err.code, "not_found");
        let mut events = c.subscribe(&[]).await.unwrap();
        c.call("prefs.set", json!({"key": "theme", "value": "dark"})).await.unwrap();
        let ev = tokio::time::timeout(Duration::from_secs(5), events.recv()).await.unwrap().unwrap();
        assert_eq!(ev.event, "state-changed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_wrong_token_closes_the_connection() {
        let (addr, _token, _tmp) = start().await;
        let err = ControlClient::connect(addr, "errado").await.err().unwrap();
        assert!(err.to_string().contains("token"), "{err}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn refuses_to_listen_off_loopback() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = Engine::new(EngineOpts { data_dir: Some(tmp.path().to_path_buf()), ephemeral: true });
        let r = serve_tcp("0.0.0.0:0".parse().unwrap(), EngineBackend::new(engine), "t".into()).await;
        assert!(r.is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn oversized_and_garbage_lines_get_errors_not_crashes() {
        let (addr, token, _tmp) = start().await;
        let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
        let auth = encode_line(&Request { id: json!(1), method: "auth".into(), params: json!({"token": token}) });
        s.write_all(auth.as_bytes()).await.unwrap();
        s.write_all(b"isto nao e json\n").await.unwrap();
        let big = vec![b'a'; MAX_REQUEST_LINE + 10];
        s.write_all(&big).await.unwrap();
        s.write_all(b"\n{\"id\":2,\"method\":\"engine.ping\"}\n").await.unwrap();
        let mut r = BufReader::new(s);
        let mut lines = Vec::new();
        for _ in 0..4 {
            let mut l = String::new();
            tokio::time::timeout(Duration::from_secs(5), r.read_line(&mut l)).await.unwrap().unwrap();
            lines.push(l);
        }
        assert!(lines[1].contains("bad_request"), "{lines:?}");
        assert!(lines[2].contains("too_large"), "{lines:?}");
        assert!(lines[3].contains("pong"), "{lines:?}");
    }
}

//! O canal de controle sobre WebSocket, mais os arquivos estáticos da versão
//! web, na mesma porta: `GET /` serve o app (wasm), `GET /ws` abre o canal.
//!
//! O protocolo é o mesmo do TCP (um quadro JSON por mensagem de texto,
//! `auth` primeiro). O WebSocket só é aceito com `Origin` igual ao `Host`:
//! outra página aberta no navegador não consegue falar com o servidor.

use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use lazyaicode_protocol::{encode_line, Reply, MAX_CONNECTIONS, MAX_REQUEST_LINE};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;

use crate::backend::Backend;
use crate::server::Conn;

pub struct WebOpts {
    pub addr: SocketAddr,
    pub token: String,
    /// Pasta com o `index.html` e o wasm (`trunk build`); `None` = só o canal.
    pub static_dir: Option<PathBuf>,
}

/// Caminho de um pedido HTTP → arquivo dentro de `root`, recusando `..`,
/// caminhos absolutos e prefixos de unidade.
pub fn resolve_static(root: &Path, url_path: &str) -> Option<PathBuf> {
    let path = url_path.split(['?', '#']).next().unwrap_or("/");
    let path = path.trim_start_matches('/');
    let rel = if path.is_empty() { "index.html" } else { path };
    let mut out = root.to_path_buf();
    for c in Path::new(rel).components() {
        match c {
            Component::Normal(p) => out.push(p),
            _ => return None,
        }
    }
    if rel.contains('\\') || rel.contains('\0') {
        return None;
    }
    Some(out)
}

pub fn content_type(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript",
        "wasm" => "application/wasm",
        "css" => "text/css",
        "json" => "application/json",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim())
    })
}

async fn serve_http(mut s: TcpStream, head: &str, root: Option<&Path>) {
    let path = head.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("/");
    let file = root.and_then(|r| resolve_static(r, path)).filter(|f| f.is_file());
    let resp = match file.as_ref().and_then(|f| std::fs::read(f).ok()) {
        Some(body) => {
            let ct = file.as_deref().map_or("application/octet-stream", content_type);
            let mut r = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nX-Content-Type-Options: nosniff\r\nCross-Origin-Opener-Policy: same-origin\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .into_bytes();
            r.extend(body);
            r
        }
        None => b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot found".to_vec(),
    };
    let _ = s.write_all(&resp).await;
}

// o `ErrorResponse` do tungstenite é grande, mas é a assinatura que ele pede
#[allow(clippy::result_large_err)]
async fn serve_ws(stream: TcpStream, backend: Arc<dyn Backend>, token: String) {
    let check = |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
        let host = req.headers().get("host").and_then(|h| h.to_str().ok()).unwrap_or("");
        let origin = req.headers().get("origin").and_then(|h| h.to_str().ok());
        let same = origin.is_none_or(|o| o.split("://").nth(1) == Some(host));
        if req.uri().path() != "/ws" || !same {
            let mut r = ErrorResponse::new(Some("forbidden".into()));
            *r.status_mut() = tokio_tungstenite::tungstenite::http::StatusCode::FORBIDDEN;
            return Err(r);
        }
        Ok(resp)
    };
    let mut cfg = WebSocketConfig::default();
    cfg.max_message_size = Some(MAX_REQUEST_LINE);
    cfg.max_frame_size = Some(MAX_REQUEST_LINE);
    let Ok(ws) = tokio_tungstenite::accept_hdr_async_with_config(stream, check, Some(cfg)).await else { return };
    let (mut sink, mut source) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            if sink.send(Message::Text(line.trim_end().to_string().into())).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });
    let conn = Conn::new(backend, Some(token), tx.clone());
    while let Some(Ok(msg)) = source.next().await {
        match msg {
            Message::Text(t) => {
                if !conn.handle_line(t.as_str()) {
                    break;
                }
            }
            Message::Binary(_) => {
                let _ = tx.send(encode_line(&Reply::err(Value::Null, "bad_request", "use mensagens de texto")));
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    drop(conn);
    drop(tx);
    let _ = writer.await;
}

pub async fn serve(opts: WebOpts, backend: Arc<dyn Backend>) -> std::io::Result<()> {
    let listener = TcpListener::bind(opts.addr).await?;
    serve_listener(listener, opts.token, opts.static_dir, backend).await
}

pub async fn serve_listener(listener: TcpListener, token: String, root: Option<PathBuf>, backend: Arc<dyn Backend>) -> std::io::Result<()> {
    let active = Arc::new(AtomicUsize::new(0));
    let root = Arc::new(root);
    loop {
        let (stream, _) = listener.accept().await?;
        let (backend, token, root, active) = (backend.clone(), token.clone(), root.clone(), active.clone());
        tokio::spawn(async move {
            let mut head = [0u8; 4096];
            let n = match tokio::time::timeout(std::time::Duration::from_secs(10), stream.peek(&mut head)).await {
                Ok(Ok(n)) => n,
                _ => return,
            };
            let text = String::from_utf8_lossy(head.get(..n).unwrap_or(&[])).to_string();
            let upgrade = header(&text, "upgrade").is_some_and(|v| v.eq_ignore_ascii_case("websocket"));
            if upgrade {
                if active.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
                    return;
                }
                active.fetch_add(1, Ordering::SeqCst);
                serve_ws(stream, backend, token).await;
                active.fetch_sub(1, Ordering::SeqCst);
            } else {
                let mut stream = stream;
                let mut buf = vec![0u8; n];
                let _ = stream.read_exact(&mut buf).await;
                serve_http(stream, &text, (*root).as_deref()).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_paths_stay_inside_the_root() {
        let root = Path::new("/srv/web");
        assert_eq!(resolve_static(root, "/"), Some(PathBuf::from("/srv/web/index.html")));
        assert_eq!(resolve_static(root, "/app_bg.wasm?v=1"), Some(PathBuf::from("/srv/web/app_bg.wasm")));
        assert_eq!(resolve_static(root, "/../etc/passwd"), None);
        assert_eq!(resolve_static(root, "/a/../../x"), None);
        assert_eq!(resolve_static(root, "/..\\x"), None);
        assert_eq!(content_type(Path::new("a.wasm")), "application/wasm");
    }

    #[test]
    fn reads_headers_case_insensitively() {
        let h = "GET /ws HTTP/1.1\r\nHost: x\r\nUPGRADE: WebSocket\r\n\r\n";
        assert_eq!(header(h, "upgrade"), Some("WebSocket"));
        assert_eq!(header(h, "origin"), None);
    }
}

#[cfg(test)]
mod live {
    use super::*;
    use crate::backend::EngineBackend;
    use lazyaicode_engine::{Engine, EngineOpts};
    use serde_json::json;

    #[tokio::test(flavor = "multi_thread")]
    async fn serves_static_files_and_the_control_channel_on_one_port() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("index.html"), "<h1>oi</h1>").unwrap();
        let engine = Engine::new(EngineOpts { data_dir: Some(tmp.path().join("data")), ephemeral: true });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(serve_listener(listener, "tok".into(), Some(tmp.path().to_path_buf()), EngineBackend::new(engine)));

        // HTTP
        let mut s = TcpStream::connect(addr).await.unwrap();
        s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).await.unwrap();
        assert!(body.starts_with("HTTP/1.1 200") && body.contains("<h1>oi</h1>"), "{body}");
        let mut s = TcpStream::connect(addr).await.unwrap();
        s.write_all(b"GET /../../etc/passwd HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).await.unwrap();
        assert!(body.starts_with("HTTP/1.1 404"), "{body}");

        // WebSocket: auth, comando, outra origem recusada
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws")).await.unwrap();
        ws.send(Message::Text(json!({"id": 1, "method": "auth", "params": {"token": "tok"}}).to_string().into())).await.unwrap();
        ws.send(Message::Text(json!({"id": 2, "method": "engine.ping"}).to_string().into())).await.unwrap();
        let mut got = Vec::new();
        while got.len() < 2 {
            if let Some(Ok(Message::Text(t))) = ws.next().await {
                got.push(t.to_string());
            }
        }
        assert!(got[1].contains("pong"), "{got:?}");
        let mut req = tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(format!("ws://{addr}/ws")).unwrap();
        req.headers_mut().insert("origin", "https://evil.example".parse().unwrap());
        assert!(tokio_tungstenite::connect_async(req).await.is_err(), "origem de outro site deve ser recusada");
    }
}

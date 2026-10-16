//! Quem atende os pedidos do canal de controle e do MCP.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use lazyaicode_engine::Engine;
use lazyaicode_protocol::{ErrorBody, Event};
use serde_json::{json, Value};
use tokio::sync::broadcast;

pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;

pub trait Backend: Send + Sync + 'static {
    /// Atende um método (`engine.run`, um id de comando, `ui.*`…).
    fn call(&self, method: String, params: Value) -> BoxFut<Result<Value, ErrorBody>>;
    /// Eventos para quem assinar (`events.subscribe`).
    fn events(&self) -> broadcast::Receiver<Event>;
    /// Métodos além dos comandos do motor (a UI acrescenta os `ui.*`).
    fn extra_methods(&self) -> Vec<Value> {
        vec![]
    }
}

pub fn error_of(e: &lazyaicode_engine::EngineError) -> ErrorBody {
    ErrorBody { code: e.code().into(), message: e.to_string(), data: e.data() }
}

pub fn bad(msg: impl Into<String>) -> ErrorBody {
    ErrorBody { code: "bad_request".into(), message: msg.into(), data: None }
}

/// O motor no próprio processo.
#[derive(Clone)]
pub struct EngineBackend {
    pub engine: Engine,
}

impl EngineBackend {
    pub fn new(engine: Engine) -> Arc<Self> {
        Arc::new(Self { engine })
    }
}

/// `engine.run {command, params}` ou o id do comando como método.
pub async fn run_engine(engine: &Engine, method: &str, params: Value) -> Result<Value, ErrorBody> {
    let (id, params) = if method == "engine.run" {
        let id = params.get("command").and_then(Value::as_str).ok_or_else(|| bad("engine.run precisa de `command`"))?.to_string();
        (id, params.get("params").cloned().unwrap_or(Value::Null))
    } else {
        (method.to_string(), params)
    };
    engine.run(&id, params).await.map_err(|e| error_of(&e))
}

impl Backend for EngineBackend {
    fn call(&self, method: String, params: Value) -> BoxFut<Result<Value, ErrorBody>> {
        let engine = self.engine.clone();
        Box::pin(async move { run_engine(&engine, &method, params).await })
    }

    fn events(&self) -> broadcast::Receiver<Event> {
        self.engine.subscribe()
    }
}

/// Descrição dos métodos do protocolo que não são comandos.
pub fn protocol_methods() -> Vec<Value> {
    vec![
        json!({"method": "auth", "params": {"token": "string"}, "doc": "Primeiro quadro de toda conexão TCP/WebSocket."}),
        json!({"method": "hello", "doc": "Versão do protocolo e do app."}),
        json!({"method": "engine.run", "params": {"command": "string", "params": "object"}, "doc": "Executa um comando do motor."}),
        json!({"method": "events.subscribe", "params": {"events": "string[] (vazio = todos)"}, "doc": "Passa a receber eventos nesta conexão."}),
        json!({"method": "events.unsubscribe", "doc": "Para de receber eventos."}),
        json!({"method": "<id de comando>", "doc": "Atalho para engine.run com aquele comando."}),
    ]
}

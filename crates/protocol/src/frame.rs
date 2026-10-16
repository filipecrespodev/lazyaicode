//! Quadros do canal de controle: um objeto JSON por linha.
//!
//! ```json
//! {"id":1,"method":"engine.run","params":{"command":"thread.list","params":{"workspaceId":"…"}}}
//! {"id":1,"result":[…]}
//! {"event":"thread-status","payload":{"threadId":"…","status":"running"}}
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{MAX_JSON_DEPTH, MAX_REQUEST_LINE};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Request {
    /// Número ou texto escolhido pelo cliente; volta igual na resposta.
    #[serde(default)]
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorBody {
    /// `bad_request`, `unknown_method`, `unauthorized`, `disabled`,
    /// `failed`, `too_large`, `timeout`…
    pub code: String,
    pub message: String,
    /// Detalhe estruturado (ex.: o erro do git com `kind`/`details`, para a
    /// UI oferecer a escolha de remote num `RemoteRequired`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reply {
    pub id: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

impl Reply {
    pub fn ok(id: Value, result: Value) -> Self {
        Self { id, result: Some(result), error: None }
    }

    pub fn err(id: Value, code: &str, message: impl Into<String>) -> Self {
        Self { id, result: None, error: Some(ErrorBody { code: code.into(), message: message.into(), data: None }) }
    }

    pub fn into_result(self) -> Result<Value, ErrorBody> {
        match self.error {
            Some(e) => Err(e),
            None => Ok(self.result.unwrap_or(Value::Null)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub event: String,
    #[serde(default)]
    pub payload: Value,
}

/// O que o servidor manda: a resposta a um pedido ou um evento.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Frame {
    Event(Event),
    Reply(Reply),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    TooLarge(usize),
    TooDeep,
    Invalid(String),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge(n) => write!(f, "linha de {n} bytes passa do limite de {MAX_REQUEST_LINE}"),
            Self::TooDeep => write!(f, "JSON aninhado demais (limite {MAX_JSON_DEPTH})"),
            Self::Invalid(e) => write!(f, "JSON inválido: {e}"),
        }
    }
}

impl std::error::Error for FrameError {}

/// Profundidade de aninhamento de um texto JSON, sem montá-lo (conta
/// colchetes e chaves fora de strings). Barra a recursão do parser antes.
pub fn json_depth(s: &str) -> usize {
    let (mut depth, mut max, mut in_str, mut esc) = (0usize, 0usize, false, false);
    for c in s.bytes() {
        if in_str {
            match (esc, c) {
                (true, _) => esc = false,
                (false, b'\\') => esc = true,
                (false, b'"') => in_str = false,
                _ => {}
            }
            continue;
        }
        match c {
            b'"' => in_str = true,
            b'[' | b'{' => {
                depth = depth.saturating_add(1);
                max = max.max(depth);
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    max
}

/// Lê uma linha do canal com os limites de tamanho e profundidade.
pub fn decode_line<T: serde::de::DeserializeOwned>(line: &str) -> Result<T, FrameError> {
    if line.len() > MAX_REQUEST_LINE {
        return Err(FrameError::TooLarge(line.len()));
    }
    if json_depth(line) > MAX_JSON_DEPTH {
        return Err(FrameError::TooDeep);
    }
    serde_json::from_str(line).map_err(|e| FrameError::Invalid(e.to_string()))
}

/// Uma linha pronta para enviar (com o `\n`).
pub fn encode_line<T: Serialize>(v: &T) -> String {
    let mut s = serde_json::to_string(v).unwrap_or_else(|_| "null".into());
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn replies_and_events_round_trip_through_one_frame_type() {
        let r = Reply::ok(json!(7), json!({"a": 1}));
        let e = Event { event: "thread-status".into(), payload: json!({"status": "idle"}) };
        assert_eq!(decode_line::<Frame>(encode_line(&r).trim()).ok(), Some(Frame::Reply(r)));
        assert_eq!(decode_line::<Frame>(encode_line(&e).trim()).ok(), Some(Frame::Event(e)));
        let err = Reply::err(json!("x"), "failed", "boom");
        assert_eq!(err.clone().into_result().err().map(|e| e.code), Some("failed".into()));
    }

    #[test]
    fn refuses_huge_and_deep_lines_before_parsing() {
        let deep = "[".repeat(MAX_JSON_DEPTH + 1) + &"]".repeat(MAX_JSON_DEPTH + 1);
        assert_eq!(decode_line::<Value>(&deep), Err(FrameError::TooDeep));
        let big = format!("\"{}\"", "a".repeat(MAX_REQUEST_LINE));
        assert!(matches!(decode_line::<Value>(&big), Err(FrameError::TooLarge(_))));
        // colchetes dentro de strings não contam
        assert_eq!(json_depth(r#"{"a":"[[[[\"]]]]"}"#), 1);
    }

    proptest::proptest! {
        #[test]
        fn never_panics_on_arbitrary_lines(s in ".{0,512}") {
            let _ = decode_line::<Request>(&s);
            let _ = decode_line::<Frame>(&s);
            let _ = json_depth(&s);
        }
    }
}

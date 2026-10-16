//! O journal da sessão: os últimos comandos que mudaram algo, com
//! parâmetros (segredos ocultos), resultado e duração. `engine.journal`
//! devolve a lista — é o que um agente consulta para saber o que já fez.

use std::collections::VecDeque;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

const MAX: usize = 500;
const SECRET_KEYS: [&str; 6] = ["password", "passphrase", "token", "secret", "data", "images"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    pub at: i64,
    pub command: String,
    pub params: Value,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub ms: u64,
}

#[derive(Default)]
pub struct Journal {
    entries: VecDeque<JournalEntry>,
}

/// Troca valores de chaves sensíveis por `"***"`, em qualquer profundidade.
pub fn redact(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, v)| {
                    let low = k.to_lowercase();
                    if SECRET_KEYS.iter().any(|s| low.contains(s)) {
                        (k.clone(), Value::String("***".into()))
                    } else {
                        (k.clone(), redact(v))
                    }
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(redact).collect()),
        Value::String(s) if s.len() > 2000 => Value::String(format!("{}… ({} bytes)", s.chars().take(200).collect::<String>(), s.len())),
        other => other.clone(),
    }
}

impl Journal {
    pub fn push<E: std::fmt::Display>(&mut self, id: &str, params: &Value, out: &Result<Value, E>, took: Duration) {
        self.entries.push_back(JournalEntry {
            at: crate::store::now(),
            command: id.to_string(),
            params: redact(params),
            ok: out.is_ok(),
            error: out.as_ref().err().map(|e| e.to_string()),
            ms: u64::try_from(took.as_millis()).unwrap_or(u64::MAX),
        });
        while self.entries.len() > MAX {
            self.entries.pop_front();
        }
    }

    pub fn entries(&self) -> Vec<JournalEntry> {
        self.entries.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hides_secrets_at_any_depth() {
        let v = redact(&json!({"url": "x", "password": "p", "nested": [{"apiToken": "t"}]}));
        assert_eq!(v, json!({"url": "x", "password": "***", "nested": [{"apiToken": "***"}]}));
    }

    #[test]
    fn keeps_only_the_last_entries() {
        let mut j = Journal::default();
        for i in 0..(MAX + 10) {
            j.push::<String>(&format!("a.b{i}"), &json!({}), &Ok(Value::Null), Duration::ZERO);
        }
        assert_eq!(j.entries().len(), MAX);
        assert_eq!(j.entries().first().map(|e| e.command.clone()), Some("a.b10".into()));
    }
}

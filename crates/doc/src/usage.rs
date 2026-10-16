//! Uso da janela de contexto, lido do `result` de cada turno.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct ContextUse {
    pub used: u64,
    pub window: u64,
    pub model: String,
}

/// Tokens na janela = entrada + cache lido + cache criado da última chamada.
pub fn used_tokens(u: &Value) -> u64 {
    ["input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"]
        .iter()
        .map(|k| u[*k].as_u64().unwrap_or(0))
        .fold(0u64, u64::saturating_add)
}

pub fn from_result(r: &Value) -> Option<ContextUse> {
    let models = r["modelUsage"].as_object()?;
    // o modelo principal é o de maior janela; subagentes também aparecem aqui
    let (model, mu) = models.iter().max_by_key(|(_, m)| m["contextWindow"].as_u64().unwrap_or(0))?;
    let last = r["usage"]["iterations"].as_array().and_then(|i| i.last()).unwrap_or(&r["usage"]);
    let used = used_tokens(last);
    let window = mu["contextWindow"].as_u64().filter(|w| *w > 0)?;
    (used > 0).then(|| ContextUse { used, window, model: model.clone() })
}

pub fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1000 {
        format!("{}k", (n as f64 / 1000.0).round() as u64)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_the_main_model_window() {
        let r = json!({"modelUsage": {"haiku": {"contextWindow": 1000}, "opus": {"contextWindow": 200000}},
                       "usage": {"input_tokens": 10, "cache_read_input_tokens": 90000}});
        assert_eq!(from_result(&r), Some(ContextUse { used: 90010, window: 200000, model: "opus".into() }));
        assert_eq!(fmt_tokens(90010), "90k");
        assert_eq!(fmt_tokens(1_500_000), "1.5M");
        assert_eq!(from_result(&json!({})), None);
    }
}

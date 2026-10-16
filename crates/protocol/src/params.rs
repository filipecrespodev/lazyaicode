//! Descrição dos parâmetros de um comando: documentação para agentes, o
//! formulário genérico da paleta e a validação antes do `run`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ParamType {
    String,
    Bool,
    Int,
    Number,
    /// Caminho no disco (a UI oferece o seletor de pasta/arquivo).
    Path,
    /// Um dos valores listados.
    Enum(Vec<String>),
    /// Lista de textos.
    Strings,
    /// Qualquer JSON (objetos e listas estruturadas).
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParamInfo {
    /// Nome em camelCase, como no JSON (`workspaceId`).
    pub name: String,
    #[serde(rename = "type")]
    pub ty: ParamType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub doc: String,
}

impl ParamInfo {
    pub fn req(name: &str, ty: ParamType) -> Self {
        Self { name: name.into(), ty, required: true, doc: String::new() }
    }

    pub fn opt(name: &str, ty: ParamType) -> Self {
        Self { name: name.into(), ty, required: false, doc: String::new() }
    }

    pub fn doc(mut self, d: &str) -> Self {
        self.doc = d.into();
        self
    }

    fn accepts(&self, v: &Value) -> bool {
        if v.is_null() {
            return !self.required;
        }
        match &self.ty {
            ParamType::String | ParamType::Path => v.is_string(),
            ParamType::Bool => v.is_boolean(),
            ParamType::Int => v.is_i64() || v.is_u64(),
            ParamType::Number => v.is_number(),
            ParamType::Enum(opts) => v.as_str().is_some_and(|s| opts.iter().any(|o| o == s)),
            ParamType::Strings => v.as_array().is_some_and(|a| a.iter().all(Value::is_string)),
            ParamType::Json => true,
        }
    }
}

/// Confere `params` contra a descrição: objeto (ou nada), obrigatórios
/// presentes, tipos certos e nenhuma chave desconhecida (um erro de digitação
/// de um agente vira mensagem, não um parâmetro ignorado em silêncio).
pub fn check_params(spec: &[ParamInfo], params: &Value) -> Result<(), String> {
    let empty = serde_json::Map::new();
    let obj = match params {
        Value::Null => &empty,
        Value::Object(o) => o,
        _ => return Err("params deve ser um objeto".into()),
    };
    for k in obj.keys() {
        if !spec.iter().any(|p| &p.name == k) {
            let known: Vec<&str> = spec.iter().map(|p| p.name.as_str()).collect();
            return Err(format!("parâmetro desconhecido `{k}` (aceitos: {})", known.join(", ")));
        }
    }
    for p in spec {
        match obj.get(&p.name) {
            None if p.required => return Err(format!("falta o parâmetro `{}`", p.name)),
            Some(v) if !p.accepts(v) => return Err(format!("`{}` deve ser {:?}", p.name, p.ty)),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_presence_types_and_unknown_keys() {
        let spec = vec![
            ParamInfo::req("id", ParamType::String),
            ParamInfo::opt("force", ParamType::Bool),
            ParamInfo::opt("mode", ParamType::Enum(vec!["merge".into(), "apply".into()])),
        ];
        assert!(check_params(&spec, &json!({"id": "x"})).is_ok());
        assert!(check_params(&spec, &json!({"id": "x", "force": null})).is_ok());
        assert!(check_params(&spec, &json!({})).unwrap_err().contains("falta"));
        assert!(check_params(&spec, &json!({"id": 1})).is_err());
        assert!(check_params(&spec, &json!({"id": "x", "mode": "rebase"})).is_err());
        assert!(check_params(&spec, &json!({"id": "x", "forse": true})).unwrap_err().contains("desconhecido"));
        assert!(check_params(&spec, &json!([1])).is_err());
        assert!(check_params(&[], &Value::Null).is_ok());
    }
}

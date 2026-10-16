//! O registro de comandos. Cada comando tem metadados ([`CommandInfo`]: id,
//! rótulo, grupo, parâmetros, atalho, menu) e uma implementação assíncrona
//! que recebe o motor e os parâmetros em JSON.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use lazyaicode_protocol::{CommandInfo, ParamInfo};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::{Engine, EngineError};

pub type BoxFut = Pin<Box<dyn Future<Output = Result<Value, EngineError>> + Send>>;
pub type RunFn = Arc<dyn Fn(Engine, Value) -> BoxFut + Send + Sync>;

pub struct Spec {
    pub info: CommandInfo,
    pub(crate) run: Option<RunFn>,
}

impl Spec {
    pub fn new(id: &str, label: &str, group: &str) -> Self {
        Self {
            info: CommandInfo {
                id: id.into(),
                label: label.into(),
                group: group.into(),
                menu: vec![],
                shortcut: None,
                params: vec![],
                doc: String::new(),
                returns: String::new(),
                read_only: false,
                destructive: false,
                scope: "engine".into(),
            },
            run: None,
        }
    }

    pub fn params(mut self, p: Vec<ParamInfo>) -> Self {
        self.info.params = p;
        self
    }

    pub fn doc(mut self, d: &str) -> Self {
        self.info.doc = d.into();
        self
    }

    pub fn returns(mut self, d: &str) -> Self {
        self.info.returns = d.into();
        self
    }

    pub fn menu(mut self, m: &[&str]) -> Self {
        self.info.menu = m.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn shortcut(mut self, s: &str) -> Self {
        self.info.shortcut = Some(s.into());
        self
    }

    pub fn read_only(mut self) -> Self {
        self.info.read_only = true;
        self
    }

    pub fn destructive(mut self) -> Self {
        self.info.destructive = true;
        self
    }

    pub fn run<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(Engine, Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, EngineError>> + Send + 'static,
    {
        self.run = Some(Arc::new(move |e, v| Box::pin(f(e, v))));
        self
    }
}

#[derive(Default)]
pub struct Registry {
    specs: Vec<Spec>,
    by_id: HashMap<String, usize>,
}

impl Registry {
    pub fn add(&mut self, spec: Spec) {
        debug_assert!(lazyaicode_protocol::valid_command_id(&spec.info.id), "id inválido: {}", spec.info.id);
        debug_assert!(!self.by_id.contains_key(&spec.info.id), "id repetido: {}", spec.info.id);
        self.by_id.entry(spec.info.id.clone()).or_insert(self.specs.len());
        self.specs.push(spec);
    }

    pub fn get(&self, id: &str) -> Option<&Spec> {
        self.by_id.get(id).and_then(|i| self.specs.get(*i))
    }

    pub fn infos(&self) -> impl Iterator<Item = &CommandInfo> {
        self.specs.iter().map(|s| &s.info)
    }

    pub fn len(&self) -> usize {
        self.specs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }
}

/// Parâmetros JSON → struct do comando, com a mensagem do serde como erro.
pub fn parse<T: DeserializeOwned>(v: Value) -> Result<T, EngineError> {
    let v = if v.is_null() { Value::Object(Default::default()) } else { v };
    serde_json::from_value(v).map_err(|e| EngineError::BadParams(e.to_string()))
}

pub fn json_of<T: Serialize>(v: T) -> Result<Value, EngineError> {
    serde_json::to_value(v).map_err(|e| EngineError::Failed(e.to_string()))
}

/// Atalhos para descrever parâmetros: `p::s("id")` (texto obrigatório),
/// `p::os("name")` (texto opcional), `p::b`/`p::ob` (bool), `p::i`/`p::oi`
/// (inteiro), `p::path`/`p::opath`, `p::list`/`p::olist` (textos),
/// `p::json`/`p::ojson`, `p::one_of`.
pub mod p {
    use lazyaicode_protocol::{ParamInfo, ParamType};

    pub fn s(n: &str) -> ParamInfo {
        ParamInfo::req(n, ParamType::String)
    }
    pub fn os(n: &str) -> ParamInfo {
        ParamInfo::opt(n, ParamType::String)
    }
    pub fn b(n: &str) -> ParamInfo {
        ParamInfo::req(n, ParamType::Bool)
    }
    pub fn ob(n: &str) -> ParamInfo {
        ParamInfo::opt(n, ParamType::Bool)
    }
    pub fn i(n: &str) -> ParamInfo {
        ParamInfo::req(n, ParamType::Int)
    }
    pub fn oi(n: &str) -> ParamInfo {
        ParamInfo::opt(n, ParamType::Int)
    }
    pub fn path(n: &str) -> ParamInfo {
        ParamInfo::req(n, ParamType::Path)
    }
    pub fn opath(n: &str) -> ParamInfo {
        ParamInfo::opt(n, ParamType::Path)
    }
    pub fn list(n: &str) -> ParamInfo {
        ParamInfo::req(n, ParamType::Strings)
    }
    pub fn olist(n: &str) -> ParamInfo {
        ParamInfo::opt(n, ParamType::Strings)
    }
    pub fn json(n: &str) -> ParamInfo {
        ParamInfo::req(n, ParamType::Json)
    }
    pub fn ojson(n: &str) -> ParamInfo {
        ParamInfo::opt(n, ParamType::Json)
    }
    pub fn one_of(n: &str, opts: &[&str], required: bool) -> ParamInfo {
        let ty = ParamType::Enum(opts.iter().map(|s| s.to_string()).collect());
        if required {
            ParamInfo::req(n, ty)
        } else {
            ParamInfo::opt(n, ty)
        }
    }
}

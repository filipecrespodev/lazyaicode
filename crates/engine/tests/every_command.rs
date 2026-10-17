//! Todas as funcionalidades do motor, uma a uma, pelo mesmo caminho que a
//! UI, a CLI e o MCP usam (`Engine::run`): o cenário de
//! `lazyaicode_testkit::scenario` roda cada comando do registro com efeito
//! conferido, e o teste falha se algum comando ficar sem cobertura.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use lazyaicode_engine::{Engine, EngineError, EngineOpts};
use lazyaicode_testkit::scenario::{self, Fail, Fixture, Runner};
use serde_json::Value;

struct InProcess {
    rt: tokio::runtime::Runtime,
    engine: Engine,
}

impl Runner for InProcess {
    fn call(&mut self, id: &str, params: Value) -> Result<Value, Fail> {
        self.rt.block_on(self.engine.run(id, params)).map_err(|e: EngineError| Fail { code: e.code().into(), message: e.to_string() })
    }
}

#[test]
fn every_engine_command_works_in_process() {
    let tmp = tempfile::tempdir().unwrap();
    let fx = Fixture::new(tmp.path());
    fx.apply_env();
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let engine = {
        let _g = rt.enter();
        Engine::new(EngineOpts { data_dir: Some(fx.data.clone()), ephemeral: false })
    };
    let mut r = InProcess { rt, engine };
    let report = scenario::run(&mut r, &fx);
    let all: Vec<String> = lazyaicode_engine::commands().into_iter().map(|c| c.id).collect();
    eprintln!("{} chamadas, {} de {} comandos", report.calls, report.covered.len(), all.len());
    let engine = r.engine.clone();
    r.rt.block_on(engine.shutdown());
    report.assert_complete(&all);
}

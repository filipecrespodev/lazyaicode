//! Comandos sobre o próprio motor: descoberta (para agentes), journal,
//! estado e lotes.

use lazyaicode_protocol::{events, MAX_BATCH_STEPS};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::registry::{json_of, p, parse, Registry, Spec};
use crate::{Engine, EngineError};

pub fn register(r: &mut Registry) {
    r.add(Spec::new("engine.ping", "Ping", "engine").read_only().run(|_, _| async move { Ok(json!("pong")) }));
    r.add(Spec::new("engine.version", "Version", "engine").read_only().returns("{version, protocol, commands}").run(|_, _| async move {
        Ok(json!({
            "name": "LazyAICode",
            "version": env!("CARGO_PKG_VERSION"),
            "protocol": lazyaicode_protocol::PROTOCOL_VERSION,
            "commands": crate::registry().len(),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
        }))
    }));
    r.add(
        Spec::new("engine.commands", "List commands", "engine")
            .params(vec![p::os("filter").doc("parte do id, do rótulo ou do grupo"), p::os("group")])
            .read_only()
            .doc("Todos os comandos que o motor executa, com parâmetros. É o ponto de partida de um agente.")
            .run(|_, v| async move {
                #[derive(Deserialize)]
                struct P {
                    filter: Option<String>,
                    group: Option<String>,
                }
                let p: P = parse(v)?;
                let list: Vec<_> = crate::registry()
                    .infos()
                    .filter(|c| p.group.as_deref().is_none_or(|g| c.group == g))
                    .filter(|c| p.filter.as_deref().is_none_or(|f| c.matches(f, &c.label)))
                    .cloned()
                    .collect();
                json_of(list)
            }),
    );
    r.add(Spec::new("engine.describe", "Describe a command", "engine").params(vec![p::s("id")]).read_only().run(|_, v| async move {
        #[derive(Deserialize)]
        struct P {
            id: String,
        }
        let p: P = parse(v)?;
        crate::find_command(&p.id).map(json_of).unwrap_or_else(|| Err(EngineError::UnknownCommand(p.id)))
    }));
    r.add(Spec::new("engine.groups", "Command groups", "engine").read_only().run(|_, _| async move {
        let mut groups: Vec<(String, usize)> = Vec::new();
        for c in crate::registry().infos() {
            match groups.iter_mut().find(|(g, _)| *g == c.group) {
                Some((_, n)) => *n += 1,
                None => groups.push((c.group.clone(), 1)),
            }
        }
        Ok(json!(groups.into_iter().map(|(g, n)| json!({"group": g, "count": n})).collect::<Vec<_>>()))
    }));
    r.add(Spec::new("engine.events", "Event names", "engine").read_only().run(|_, _| async move { Ok(json!(events::ALL)) }));
    r.add(
        Spec::new("engine.journal", "Session journal", "engine")
            .params(vec![p::oi("last")])
            .read_only()
            .doc("Os últimos comandos que mudaram algo nesta sessão, com resultado e duração (segredos ocultos).")
            .run(|e, v| async move {
                let last = v.get("last").and_then(Value::as_u64).unwrap_or(50) as usize;
                let all = e.journal();
                let skip = all.len().saturating_sub(last);
                json_of(all.into_iter().skip(skip).collect::<Vec<_>>())
            }),
    );
    r.add(
        Spec::new("engine.state", "Session state", "engine")
            .read_only()
            .doc("O estado inteiro (workspaces, projetos, threads com status, preferências).")
            .run(|e, _| async move {
                let state = e.core.store.lock().state.clone();
                let threads: Vec<_> = state.threads.iter().map(|t| e.view(t)).collect();
                Ok(json!({
                    "workspaces": state.workspaces,
                    "activeWorkspace": state.active_workspace,
                    "projects": state.projects,
                    "threads": threads,
                    "settings": state.settings,
                }))
            }),
    );
    r.add(
        Spec::new("engine.batch", "Run several commands", "engine")
            .params(vec![p::json("steps").doc("[{command, params}] (até 256)"), p::ob("stopOnError").doc("padrão: true")])
            .doc("Executa comandos em sequência e devolve um resultado por passo.")
            .run(|e, v| async move { batch(e, v).await }),
    );
}

async fn batch(e: Engine, v: Value) -> Result<Value, EngineError> {
    #[derive(Deserialize)]
    struct Step {
        command: String,
        #[serde(default)]
        params: Value,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct P {
        steps: Vec<Step>,
        stop_on_error: Option<bool>,
    }
    let p: P = parse(v)?;
    if p.steps.len() > MAX_BATCH_STEPS {
        return Err(EngineError::BadParams(format!("no máximo {MAX_BATCH_STEPS} passos")));
    }
    let stop = p.stop_on_error.unwrap_or(true);
    let mut out = Vec::with_capacity(p.steps.len());
    for s in p.steps {
        if s.command == "engine.batch" {
            return Err(EngineError::BadParams("engine.batch não pode conter outro engine.batch".into()));
        }
        // Box: run → batch → run é recursivo
        let r: std::pin::Pin<Box<dyn std::future::Future<Output = _> + Send>> = Box::pin(e.run(&s.command, s.params));
        match r.await {
            Ok(v) => out.push(json!({"command": s.command, "ok": true, "result": v})),
            Err(err) => {
                out.push(json!({"command": s.command, "ok": false, "error": {"code": err.code(), "message": err.to_string()}}));
                if stop {
                    break;
                }
            }
        }
    }
    Ok(Value::Array(out))
}

//! Testes do motor pelo mesmo caminho que a UI, a CLI e o MCP usam:
//! `Engine::run(id, params)`.

use super::*;
use serde_json::json;
use std::path::Path;

fn engine(dir: &Path) -> Engine {
    Engine::new(EngineOpts { data_dir: Some(dir.join("data")), ephemeral: false })
}

fn git(dir: &Path, args: &[&str]) {
    let ok = std::process::Command::new("git").args(args).current_dir(dir).output().map(|o| o.status.success()).unwrap_or(false);
    assert!(ok, "git {args:?}");
}

fn init_repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@example.com"]);
    git(dir, &["config", "user.name", "T"]);
    git(dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(dir.join("a.txt"), "um\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "first"]);
}

#[test]
fn every_command_is_well_formed_and_unique() {
    let cmds = commands();
    assert!(cmds.len() >= 160, "só {} comandos", cmds.len());
    let mut ids = std::collections::HashSet::new();
    for c in &cmds {
        assert!(lazyaicode_protocol::valid_command_id(&c.id), "id inválido: {}", c.id);
        assert!(ids.insert(c.id.clone()), "id repetido: {}", c.id);
        assert!(!c.label.is_empty() && !c.group.is_empty(), "{}", c.id);
        // a paleta mostra o rótulo: um id no lugar dele não é rótulo
        assert!(c.label.contains(' ') || !c.label.contains('.'), "{}: rótulo parece id: {}", c.id, c.label);
        let mut names = std::collections::HashSet::new();
        for p in &c.params {
            assert!(names.insert(p.name.clone()), "{}: parâmetro repetido {}", c.id, p.name);
            assert!(!p.name.contains('_'), "{}: parâmetro em snake_case {}", c.id, p.name);
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_commands_and_bad_params_are_errors_not_panics() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path());
    assert!(matches!(e.run("nao.existe", Value::Null).await, Err(EngineError::UnknownCommand(_))));
    assert!(matches!(e.run("thread.get", json!({})).await, Err(EngineError::BadParams(_))));
    assert!(matches!(e.run("thread.get", json!({"id": 1})).await, Err(EngineError::BadParams(_))));
    assert!(matches!(e.run("thread.get", json!({"id": "x", "extra": 1})).await, Err(EngineError::BadParams(_))));
    assert!(matches!(e.run("thread.get", json!({"id": "x"})).await, Err(EngineError::NotFound(_))));
    assert_eq!(e.run("engine.ping", Value::Null).await.unwrap(), json!("pong"));
}

#[tokio::test(flavor = "multi_thread")]
async fn workspace_project_isolated_thread_review_and_land() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path().join("ws");
    let repo = ws.join("app");
    init_repo(&repo);
    std::fs::create_dir_all(ws.join("docs")).unwrap();
    let e = engine(tmp.path());

    let w = e.run("workspace.create", json!({"path": ws.to_string_lossy()})).await.unwrap();
    let wid = w["id"].as_str().unwrap().to_string();
    let projects = e.run("project.list", json!({"workspaceId": wid})).await.unwrap();
    let names: Vec<&str> = projects.as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["app", "docs"]);
    let app = projects.as_array().unwrap().iter().find(|p| p["name"] == "app").unwrap();
    assert_eq!(app["is_git"], json!(true));

    let t = e.run("thread.create", json!({"workspaceId": wid, "projectId": app["id"], "isolated": true})).await.unwrap();
    let tid = t["id"].as_str().unwrap().to_string();
    assert_eq!(t["isolated"], json!(true));
    assert_eq!(t["status"], json!("idle"));
    let cwd = PathBuf::from(t["cwd"].as_str().unwrap());
    assert!(cwd.join("a.txt").is_file(), "checkout isolado criado");

    // o "agente" muda um arquivo no checkout da thread
    std::fs::write(cwd.join("a.txt"), "um\ndois\n").unwrap();
    let ch = e.run("thread.changes", json!({"id": tid, "base": "upstream"})).await.unwrap();
    let files = ch["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], json!("a.txt"));
    assert_eq!(files[0]["seen"], json!(false));
    let fp = files[0]["fingerprint"].clone();
    e.run("thread.set-seen", json!({"id": tid, "entries": [["a.txt", fp]]})).await.unwrap();
    let ch = e.run("thread.changes", json!({"id": tid, "base": "upstream"})).await.unwrap();
    assert_eq!(ch["files"][0]["seen"], json!(true));
    let st = e.run("thread.diff-stats", json!({"id": tid})).await.unwrap();
    assert_eq!((st["additions"].as_u64(), st["files"].as_u64()), (Some(1), Some(1)));

    // integra no checkout principal como mudança não commitada
    e.run("thread.land", json!({"id": tid, "mode": "apply"})).await.unwrap();
    assert_eq!(std::fs::read_to_string(repo.join("a.txt")).unwrap(), "um\ndois\n");

    // arquivar remove o checkout e mantém a branch; desarquivar recria
    e.run("thread.archive", json!({"id": tid, "archived": true})).await.unwrap();
    assert!(!cwd.exists());
    e.run("thread.archive", json!({"id": tid, "archived": false})).await.unwrap();
    assert!(cwd.join("a.txt").exists());

    // o journal guardou as mudanças, não as leituras
    let j: Vec<String> = e.journal().into_iter().map(|j| j.command).collect();
    assert!(j.contains(&"thread.create".to_string()) && !j.contains(&"thread.changes".to_string()), "{j:?}");

    // estado sobrevive a reabrir o motor
    drop(e);
    let e = engine(tmp.path());
    let list = e.run("thread.list", json!({"workspaceId": wid})).await.unwrap();
    assert_eq!(list.as_array().map(Vec::len), Some(1));

    e.run("thread.delete", json!({"id": tid})).await.unwrap();
    assert!(!cwd.exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn git_commands_drive_the_repository_actor() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("r");
    init_repo(&repo);
    let e = engine(tmp.path());
    let path = repo.to_string_lossy().to_string();
    let summary = e.run("git.repo.open", json!({"path": path})).await.unwrap();
    let rid = summary["repoId"].as_str().unwrap().to_string();
    std::fs::write(repo.join("b.txt"), "novo\n").unwrap();
    let st = e.run("git.working.status", json!({"repoId": rid})).await.unwrap();
    assert!(st.to_string().contains("b.txt"), "{st}");
    e.run("git.working.stage-all", json!({"repoId": rid})).await.unwrap();
    let branches = e.run("git.branch.list", json!({"repoId": rid})).await.unwrap();
    assert!(branches.to_string().contains("main"), "{branches}");
    let log = e
        .run(
            "engine.batch",
            json!({"steps": [
                {"command": "git.working.status", "params": {"repoId": rid}},
                {"command": "nao.existe"},
                {"command": "engine.ping"},
            ]}),
        )
        .await
        .unwrap();
    let steps = log.as_array().unwrap();
    assert_eq!(steps.len(), 2, "parou no erro");
    assert_eq!(steps[1]["ok"], json!(false));
}

/// Um "claude" falso que fala stream-json: anuncia a sessão, responde e
/// fecha o turno. Exercita o caminho inteiro de `thread.send` até os eventos.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn send_streams_agent_events_and_updates_the_thread() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path().join("ws");
    init_repo(&ws.join("app"));
    let bin = tmp.path().join("fake-claude");
    std::fs::write(
        &bin,
        "#!/bin/sh\necho '{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"sess-1\"}'\n\
         while read -r l; do\n\
           case \"$l\" in\n\
           *'\"subtype\":\"interrupt\"'*)\n\
             echo '{\"type\":\"control_cancel_request\",\"request_id\":\"p1\"}'\n\
             echo '{\"type\":\"result\",\"subtype\":\"error_during_execution\",\"is_error\":true,\"terminal_reason\":\"aborted_streaming\",\"errors\":[\"[ede_diagnostic] result_type=user\"]}';;\n\
           *'peça permissão'*)\n\
             printf '%s\\n' \"$l\" | sed 's/^{/{\"isReplay\":true,/'\n\
             echo '{\"type\":\"control_request\",\"request_id\":\"p1\",\"request\":{\"subtype\":\"can_use_tool\",\"tool_name\":\"Bash\",\"input\":{\"command\":\"make\"}}}';;\n\
           *'devagar'*)\n\
             printf '%s\\n' \"$l\" | sed 's/^{/{\"isReplay\":true,/'\n\
             sleep 1\n\
             echo '{\"type\":\"result\",\"is_error\":false,\"duration_ms\":5,\"total_cost_usd\":0}';;\n\
           *'\"type\":\"user\"'*)\n\
             printf '%s\\n' \"$l\" | sed 's/^{/{\"isReplay\":true,/'\n\
             echo '{\"type\":\"assistant\",\"message\":{\"id\":\"m1\",\"content\":[{\"type\":\"text\",\"text\":\"feito\"}]}}'\n\
             echo '{\"type\":\"result\",\"is_error\":false,\"duration_ms\":5,\"total_cost_usd\":0}';;\n\
           esac\n\
         done\n",
    )
    .unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("LAZYAICODE_CLAUDE_BIN", &bin);

    let e = engine(tmp.path());
    let mut rx = e.subscribe();
    let w = e.run("workspace.create", json!({"path": ws.to_string_lossy()})).await.unwrap();
    let projects = e.run("project.list", json!({"workspaceId": w["id"]})).await.unwrap();
    let t = e.run("thread.create", json!({"workspaceId": w["id"], "projectId": projects[0]["id"], "isolated": false})).await.unwrap();
    let tid = t["id"].as_str().unwrap().to_string();
    e.run("thread.send", json!({"id": tid, "text": "faça algo\nsegunda linha"})).await.unwrap();

    let mut transcript = lazyaicode_doc::transcript::Transcript::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let ev = tokio::time::timeout_at(deadline, rx.recv()).await.expect("sem eventos").unwrap();
        if ev.event == events::AGENT_EVENT {
            transcript.apply(&ev.payload["msg"]);
        }
        if ev.event == events::THREAD_STATUS && ev.payload["status"] == "idle" {
            break;
        }
    }
    assert!(transcript.items.iter().any(|i| matches!(i, lazyaicode_doc::transcript::Item::Text { text, .. } if text == "feito")));
    let t = e.run("thread.get", json!({"id": tid})).await.unwrap();
    assert_eq!(t["session_id"], json!("sess-1"));
    assert_eq!(t["title"], json!("faça algo"));
    assert_eq!(t["unread"], json!(true));
    // terminou sem ser lida: aparece na atividade de todos os workspaces
    let act = e.run("thread.activity", json!({})).await.unwrap();
    let act = act.as_array().unwrap();
    assert_eq!(act.len(), 1);
    assert_eq!(act[0]["id"], json!(tid));
    assert_eq!(act[0]["workspace_id"], w["id"]);
    e.run("thread.update", json!({"id": tid, "unread": false})).await.unwrap();
    assert_eq!(e.run("thread.activity", json!({})).await.unwrap(), json!([]));

    // interromper (Esc) enquanto o agente espera uma permissão: o CLI cancela
    // o pedido e fecha o turno com `error_during_execution` — a thread volta
    // a ficar parada, não com erro, e sem pedido pendente
    e.run("thread.send", json!({"id": tid, "text": "peça permissão"})).await.unwrap();
    let mut last = String::new();
    while last != "awaiting" {
        let ev = tokio::time::timeout_at(deadline, rx.recv()).await.expect("não pediu permissão").unwrap();
        if ev.event == events::THREAD_STATUS {
            last = ev.payload["status"].as_str().unwrap_or("").to_string();
        }
    }
    assert_eq!(e.run("thread.pending-permission", json!({"id": tid})).await.unwrap()["requestId"], json!("p1"));
    e.run("thread.interrupt", json!({"id": tid})).await.unwrap();
    let mut seen = vec![];
    loop {
        let ev = tokio::time::timeout_at(deadline, rx.recv()).await.expect("o turno não fechou").unwrap();
        if ev.event == events::THREAD_STATUS {
            let s = ev.payload["status"].as_str().unwrap_or("").to_string();
            seen.push(s.clone());
            if s != "running" && s != "awaiting" {
                break;
            }
        }
    }
    assert_eq!(seen.last().map(String::as_str), Some("idle"), "status depois do Esc: {seen:?}");
    assert_eq!(e.run("thread.pending-permission", json!({"id": tid})).await.unwrap(), Value::Null);

    // uma mensagem mandada com o turno rodando entra na fila do CLI: o fim do
    // primeiro turno não é o fim — a thread só para depois de responder a ela
    e.run("thread.send", json!({"id": tid, "text": "devagar"})).await.unwrap();
    e.run("thread.send", json!({"id": tid, "text": "e mais isto"})).await.unwrap();
    let (mut results, mut seen) = (0, vec![]);
    loop {
        let ev = tokio::time::timeout_at(deadline, rx.recv()).await.expect("a fila não terminou").unwrap();
        if ev.event == events::AGENT_EVENT && ev.payload["msg"]["type"] == "result" {
            results += 1;
        }
        if ev.event == events::THREAD_STATUS {
            let s = ev.payload["status"].as_str().unwrap_or("").to_string();
            seen.push(s.clone());
            if s == "idle" {
                break;
            }
        }
    }
    assert_eq!(results, 2, "parou depois do primeiro turno, com uma mensagem na fila: {seen:?}");
    e.shutdown().await;
}

/// Todo comando, com parâmetros do tipo certo mas valores vazios ou
/// absurdos, devolve `Ok` ou `Err` — nunca entra em pânico. Comandos que
/// abrem janelas, navegador ou processos externos ficam de fora.
#[tokio::test(flavor = "multi_thread")]
async fn panic_hunt_every_command_with_hostile_params() {
    use lazyaicode_protocol::ParamType;
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path());
    let skip = [
        "provider.",
        "system.open-path",
        "git.config.open-in-editor",
        "terminal.spawn",
        "wsl.",
        "git.lfs.",
        "git.ssh.",
        "git.https.",
        "ai.",
        "capabilities.get",
        "thread.send",
        "git.remote.",
        "git.pull.",
        "github.",
        "gitlab.",
        "system.fonts",
        "system.check-update",
        "git.submodule.update",
    ];
    for c in commands() {
        if skip.iter().any(|s| c.id.starts_with(s)) {
            continue;
        }
        for variant in 0..3 {
            let mut params = serde_json::Map::new();
            for p in &c.params {
                let v = match (&p.ty, variant) {
                    (ParamType::String | ParamType::Path, 0) => json!(""),
                    (ParamType::String | ParamType::Path, 1) => json!("../../../etc/passwd"),
                    (ParamType::String | ParamType::Path, _) => json!("\u{0}💥".repeat(50)),
                    (ParamType::Bool, v) => json!(v == 1),
                    (ParamType::Int, 0) => json!(0),
                    (ParamType::Int, 1) => json!(u64::MAX),
                    (ParamType::Int, _) => json!(-1),
                    (ParamType::Number, _) => json!(-1.5),
                    (ParamType::Enum(o), _) => json!(o.first()),
                    (ParamType::Strings, _) => json!(["", "x"]),
                    (ParamType::Json, 0) => json!({}),
                    (ParamType::Json, 1) => json!([]),
                    (ParamType::Json, _) => json!(null),
                };
                if p.required || variant > 0 {
                    params.insert(p.name.clone(), v);
                }
            }
            let r = e.run(&c.id, Value::Object(params)).await;
            assert!(!matches!(r, Err(EngineError::Panicked(_))), "{} entrou em pânico (variante {variant}): {r:?}", c.id);
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_merge_conflict_is_listed_resolved_and_continued_through_commands() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("r");
    init_repo(&repo);
    git(&repo, &["checkout", "-qb", "outro"]);
    std::fs::write(repo.join("a.txt"), "deles\n").unwrap();
    git(&repo, &["commit", "-qam", "deles"]);
    git(&repo, &["checkout", "-q", "main"]);
    std::fs::write(repo.join("a.txt"), "nosso\n").unwrap();
    git(&repo, &["commit", "-qam", "nosso"]);
    let e = engine(tmp.path());
    let s = e.run("git.repo.open", json!({"path": repo.to_string_lossy()})).await.unwrap();
    let rid = s["repoId"].as_str().unwrap().to_string();
    let m = e.run("git.branch.merge", json!({"repoId": rid, "name": "outro"})).await.unwrap();
    assert!(m.to_string().contains("onflict"), "{m}");
    let st = e.run("git.repo.status", json!({"repoId": rid})).await.unwrap();
    assert_eq!(st["inProgressOp"], json!("Merge"));
    let rs = e.run("git.rebase.status", json!({"repoId": rid})).await.unwrap();
    assert_eq!(rs["conflicts"][0]["path"], json!("a.txt"));
    assert_eq!(rs["conflicts"][0]["theirsContent"], json!("deles\n"));
    let after = e
        .run(
            "git.rebase.resolve-conflict",
            json!({"repoId": rid, "path": "a.txt", "resolution": {"kind": "UseManualContent", "content": "os dois\n"}}),
        )
        .await
        .unwrap();
    assert_eq!(after["conflicts"], json!([]));
    e.run("git.rebase.continue", json!({"repoId": rid})).await.unwrap();
    assert_eq!(std::fs::read_to_string(repo.join("a.txt")).unwrap(), "os dois\n");
    let st = e.run("git.repo.status", json!({"repoId": rid})).await.unwrap();
    assert!(st["inProgressOp"].is_null(), "{st}");
}

#[tokio::test(flavor = "multi_thread")]
async fn line_staging_puts_only_the_picked_lines_in_the_index() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("r");
    init_repo(&repo);
    std::fs::write(repo.join("a.txt"), "um\ndois\ntres\n").unwrap();
    let e = engine(tmp.path());
    let s = e.run("git.repo.open", json!({"path": repo.to_string_lossy()})).await.unwrap();
    let rid = s["repoId"].as_str().unwrap().to_string();
    // só a linha 2 ("dois") entra no staging
    e.run(
        "git.working.apply-lines",
        json!({"repoId": rid, "path": "a.txt", "action": "stage", "selection": {"oldLines": [], "newLines": [2], "newTexts": ["dois"]}}),
    )
    .await
    .unwrap();
    let staged = std::process::Command::new("git").args(["diff", "--cached"]).current_dir(&repo).output().unwrap();
    let staged = String::from_utf8_lossy(&staged.stdout).to_string();
    assert!(staged.contains("+dois") && !staged.contains("+tres"), "{staged}");
}

/// Caminho relativo seria resolvido a partir da pasta do processo: um
/// `git.path.commit-all {path: "../x"}` commitava no repositório onde o app
/// (ou o teste) foi aberto. Todo caminho que entra precisa ser absoluto.
#[tokio::test(flavor = "multi_thread")]
async fn relative_paths_are_refused_instead_of_resolved_against_the_process_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path());
    for (id, params) in [
        ("git.path.commit-all", json!({"path": "../../../etc/passwd", "message": "x"})),
        ("git.path.status", json!({"path": "."})),
        ("git.repo.open", json!({"path": "."})),
        ("account.status", json!({"claudeDir": "../conta"})),
        ("workspace.create", json!({"path": "."})),
        ("terminal.spawn", json!({"termId": "t", "cwd": "..", "cols": 80, "rows": 24})),
        ("system.read-image", json!({"path": "img.png"})),
    ] {
        let r = e.run(id, params).await;
        assert!(matches!(r, Err(EngineError::BadParams(_))), "{id} aceitou caminho relativo: {r:?}");
    }
    let r = e.run("thread.set-options", json!({"id": "x", "addDirs": ["rel"]})).await;
    assert!(matches!(r, Err(EngineError::BadParams(_))), "addDirs relativo: {r:?}");
}

#[tokio::test]
async fn author_photo_comes_from_gravatar_and_is_remembered() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path());
    // o hash é do e-mail normalizado (sem espaços, minúsculo)
    let hash = crate::extra_cmds::gravatar_hash("demo@example.com").unwrap();
    assert_eq!(crate::extra_cmds::gravatar_hash("  Demo@Example.COM "), Some(hash.clone()));
    assert_eq!(hash, format!("{:x}", md5::compute("demo@example.com")));
    for bad in ["", "sem-arroba", "@x", "a@", "a b@c"] {
        assert!(crate::extra_cmds::gravatar_hash(bad).is_none(), "{bad}");
    }
    std::env::set_var("LAZYAICODE_GRAVATAR_URL", lazyaicode_testkit::fakes::fake_gravatar_server(&hash, b"PNGDATA".to_vec()));
    let got = e.run("system.avatar", json!({"email": "Demo@Example.com"})).await.unwrap();
    assert_eq!(got["image"], json!("UE5HREFUQQ=="));
    assert_eq!(e.run("system.avatar", json!({"email": "ninguem@example.com"})).await.unwrap(), Value::Null);
    // já sabidos: nem o servidor fora do ar muda a resposta
    std::env::set_var("LAZYAICODE_GRAVATAR_URL", "http://127.0.0.1:9/avatar");
    assert_eq!(e.run("system.avatar", json!({"email": "demo@example.com"})).await.unwrap()["image"], json!("UE5HREFUQQ=="));
    assert_eq!(e.run("system.avatar", json!({"email": "ninguem@example.com"})).await.unwrap(), Value::Null);
    // desligado
    std::env::set_var("LAZYAICODE_GRAVATAR_URL", "");
    assert_eq!(e.run("system.avatar", json!({"email": "outro@example.com"})).await.unwrap(), Value::Null);
    assert!(e.run("system.avatar", json!({"email": "nada"})).await.is_err());
}

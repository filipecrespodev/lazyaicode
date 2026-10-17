//! Ponta a ponta de todos os comandos de tela (`ui.*`): o app montado como
//! no `--screenshot` (motor real no processo, `LocalClient`, `LazyApp`, um
//! contexto egui rodando quadros), com um workspace de verdade (repositório
//! git com remoto, branch, tag, mudanças) e um `claude` falso. Cada comando
//! entra pela inbox, como pelo canal de controle; o teste espera a resposta,
//! as chamadas ao motor que ele disparou, desenha quadros e confere que nada
//! entrou em pânico nem virou aviso de erro.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeSet, HashSet};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use base64::Engine as _;
use lazyaicode_engine::{Engine, EngineOpts};
use lazyaicode_testkit::git;
use lazyaicode_ui_egui::{Host, Inbox, LazyApp, UiMsg};
use serde_json::{json, Value};

use crate::local;

/// A plataforma de mentira: os seletores "escolhem" arquivos do teste na
/// hora, a área de transferência tem uma imagem e nada abre janela.
struct TestHost {
    inbox: Inbox,
    png: PathBuf,
    folder: PathBuf,
}

impl Host for TestHost {
    fn pick_folder(&self, _: &str, then: &str, mut params: Value) {
        if !params.is_object() {
            params = json!({});
        }
        params["path"] = json!(self.folder.to_string_lossy());
        self.inbox.push(then, params);
    }
    fn pick_images(&self, then: &str, mut params: Value) {
        if !params.is_object() {
            params = json!({});
        }
        params["paths"] = json!([self.png.to_string_lossy()]);
        self.inbox.push(then, params);
    }
    fn clipboard_image(&self) -> Option<Vec<u8>> {
        std::fs::read(&self.png).ok()
    }
    fn notify(&self, _: &str, _: &str) {}
    fn native_decorations(&self) -> bool {
        false
    }
    fn is_web(&self) -> bool {
        false
    }
    fn platform(&self) -> String {
        "test".into()
    }
    fn now_ms(&self) -> i64 {
        1_800_000_000_000
    }
}

/// Passo do roteiro que não é comando: espera a conversa aberta ter (ou não)
/// um pedido de permissão pendente.
const WAIT_PERMISSION: &str = "@wait-permission";
/// Passo que confere um valor do `ui.inspect` (`{"path": ponteiro JSON, "equals": valor}`).
const CHECK_INSPECT: &str = "@inspect";
/// Passo que confere a altura mínima (px) da paleta aberta.
const CHECK_PALETTE_HEIGHT: &str = "@palette-height";

/// O que se espera de um comando no roteiro.
enum Expect {
    /// responde `Ok` e não deixa aviso de erro
    Ok,
    /// falha (resposta `Err` ou aviso de erro), pelo motivo dado
    Fails(&'static str),
}

struct Harness {
    app: LazyApp,
    ctx: egui::Context,
    rt: tokio::runtime::Runtime,
    engine: Engine,
    tex: lazyaicode_render_cpu::Textures,
    frame: u64,
    /// `UI_E2E_SHOTS=pasta` grava a tela depois de cada comando
    shots: Option<PathBuf>,
    last_png: Option<Vec<u8>>,
}

const SIZE: (u32, u32) = (1280, 800);

impl Harness {
    /// Um quadro: esvazia respostas e inbox, desenha, e atende capturas.
    fn frame(&mut self, raster: bool) -> Result<(), String> {
        let (app, ctx, tex, i, last) = (&mut self.app, &self.ctx, &mut self.tex, self.frame, &mut self.last_png);
        self.frame += 1;
        std::panic::catch_unwind(AssertUnwindSafe(|| {
            app.pump();
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(SIZE.0 as f32, SIZE.1 as f32));
            let input = egui::RawInput { screen_rect: Some(rect), time: Some(i as f64 * 0.05), focused: true, ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                egui::CentralPanel::no_frame().show(ui, |ui| app.ui(ui));
            });
            tex.apply(&out.textures_delta);
            out.textures_delta.clear();
            let prims = ctx.tessellate(out.shapes, 1.0);
            let (waiters, _) = app.take_screenshot_waiters();
            if raster || !waiters.is_empty() {
                let bg = ctx.global_style().visuals.panel_fill;
                let png = lazyaicode_render_cpu::render_png(&prims, tex, [SIZE.0, SIZE.1], 1.0, bg);
                last.clone_from(&png);
                for w in waiters {
                    let _ = w.send(match &png {
                        Some(p) => Ok(json!({ "png": base64::engine::general_purpose::STANDARD.encode(p), "renderer": "cpu" })),
                        None => Err(lazyaicode_protocol::ErrorBody { code: "failed".into(), message: "png".into(), data: None }),
                    });
                }
            }
        }))
        .map_err(|e| panic_text(&e))
    }

    /// Quadros até `done` ou o prazo.
    fn frames_until(&mut self, secs: u64, mut done: impl FnMut(&mut Self) -> bool) -> Result<bool, String> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < deadline {
            self.frame(false)?;
            if done(self) {
                return Ok(true);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(false)
    }

    fn inspect(&mut self) -> Value {
        lazyaicode_ui_egui::commands::run_ui(&mut self.app, "ui.inspect", &Value::Null).unwrap_or(Value::Null)
    }

    /// Roda um comando como o canal de controle roda: pela inbox, com resposta.
    fn run(&mut self, id: &str, params: Value) -> Outcome {
        let _ = lazyaicode_ui_egui::commands::run_ui(&mut self.app, "ui.toasts.clear", &Value::Null);
        let (tx, rx) = std::sync::mpsc::channel();
        self.app.inbox().push_msg(UiMsg { command: id.into(), params, reply: Some(tx) });
        let mut reply = None;
        let got = self.frames_until(20, |_| match rx.try_recv() {
            Ok(r) => {
                reply = Some(r);
                true
            }
            Err(_) => false,
        });
        match got {
            Err(p) => return Outcome::Panic(p),
            Ok(false) => return Outcome::Timeout,
            Ok(true) => {}
        }
        // as chamadas ao motor que o comando disparou (e as que elas dispararam)
        let mut quiet = 0;
        let settled = self.frames_until(30, |h| {
            quiet = if h.app.has_pending() { 0 } else { quiet + 1 };
            quiet >= 5
        });
        match settled {
            Err(p) => return Outcome::Panic(p),
            Ok(false) => return Outcome::Timeout,
            Ok(true) => {}
        }
        if let Err(p) = self.frame(true) {
            return Outcome::Panic(p);
        }
        if let (Some(dir), Some(png)) = (&self.shots, &self.last_png) {
            let _ = std::fs::write(dir.join(format!("{:06}-{id}.png", self.frame)), png);
        }
        let errors: Vec<String> =
            self.inspect()["errors"].as_array().into_iter().flatten().filter_map(|e| e.as_str().map(str::to_string)).collect();
        match reply {
            Some(Err(e)) => Outcome::Err(e.message),
            _ if !errors.is_empty() => Outcome::Err(errors.join(" | ")),
            Some(Ok(_)) => Outcome::Ok,
            None => Outcome::Timeout,
        }
    }
}

enum Outcome {
    Ok,
    Err(String),
    Panic(String),
    Timeout,
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Outcome::Ok => write!(f, "Ok"),
            Outcome::Err(e) => write!(f, "erro: {e}"),
            Outcome::Panic(p) => write!(f, "PÂNICO: {p}"),
            Outcome::Timeout => write!(f, "sem resposta (timeout)"),
        }
    }
}

fn panic_text(e: &Box<dyn std::any::Any + Send>) -> String {
    e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "pânico".into())
}

#[cfg(unix)]
fn script(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn origin_dir(root: &Path) -> PathBuf {
    root.join("origin.git")
}

/// Workspace `ws/` com o projeto git `app` (remoto bare `origin`, branch
/// `feature`, tag `v1`, um arquivo modificado e um novo) e a pasta `docs`.
fn fixture(root: &Path) -> (PathBuf, PathBuf) {
    let ws = root.join("ws");
    let repo = ws.join("app");
    let origin = origin_dir(root);
    lazyaicode_testkit::init_repo(&repo);
    std::fs::create_dir_all(ws.join("docs")).unwrap();
    std::fs::write(ws.join("docs/leia.md"), "# docs\n").unwrap();
    git(root, &["init", "-q", "--bare", "-b", "main", &origin.to_string_lossy()]);
    git(&repo, &["remote", "add", "origin", &origin.to_string_lossy()]);
    git(&repo, &["push", "-q", "-u", "origin", "main"]);
    git(&repo, &["checkout", "-qb", "feature"]);
    std::fs::write(repo.join("f.txt"), "feature\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "feat: f"]);
    git(&repo, &["checkout", "-q", "main"]);
    git(&repo, &["tag", "v1"]);
    std::fs::write(repo.join("a.txt"), "um\ndois\n").unwrap();
    std::fs::write(repo.join("novo.txt"), "novo\n").unwrap();
    (ws, repo)
}

/// Um `claude` falso que cobre os três usos do app: a conversa
/// (stream-json), a sondagem de capacidades (`control_request`) e as
/// respostas de uma vez da IA do git (`-p --output-format json`).
const FAKE_CLAUDE: &str = r#"#!/bin/sh
case " $* " in *" --output-format json "*)
  cat >/dev/null
  printf '%s\n' '{"type":"result","is_error":false,"result":"feat: mensagem gerada\n\ncorpo gerado"}'
  exit 0;;
esac
printf '%s\n' '{"type":"system","subtype":"init","session_id":"sess-1"}'
n=0
say() {
  printf '{"type":"assistant","message":{"id":"m%s","content":[{"type":"text","text":"%s"}]}}\n' "$n" "$1"
  printf '%s\n' '{"type":"result","is_error":false,"duration_ms":5,"total_cost_usd":0}'
}
while read -r l; do
  case "$l" in
    *'"control_response"'*)
      case "$l" in *'"behavior":"allow"'*) say permitido;; *) say negado;; esac;;
    *'"control_request"'*)
      id=$(printf '%s' "$l" | sed -n 's/.*"request_id":"\([^"]*\)".*/\1/p')
      printf '{"type":"control_response","response":{"subtype":"success","request_id":"%s","response":{"commands":[],"agents":[],"models":[],"mcpServers":[]}}}\n' "$id";;
    *'"type":"user"'*)
      n=$((n+1))
      case "$l" in
        *pedir*) printf '{"type":"control_request","request_id":"perm-%s","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{"command":"ls"},"tool_use_id":"tu-%s"}}\n' "$n" "$n";;
        *) say feito;;
      esac;;
  esac
done
"#;

/// A tabela: todo comando de tela, na ordem em que roda, com parâmetros e
/// o que se espera. A ordem importa (o estado de um passo é o do anterior).
fn table(ctx: &Ctx) -> Vec<(&'static str, Value, Expect)> {
    use Expect::{Fails, Ok as Y};
    let v = |x: Value| x;
    let e = || json!({});
    vec![
        // ---------- automação ----------
        ("ui.inspect", e(), Y),
        ("ui.screenshot", json!({"cpu": true}), Y),
        ("ui.run", json!({"command": "ui.tab.chat"}), Y),
        ("ui.state.set", json!({"patch": {"sidebarOpen": true}}), Y),
        ("ui.state.set", json!({"patch": {"sidebar_open": true}}), Fails("campo com nome errado")),
        ("ui.dialog.cancel", e(), Fails("nenhum diálogo aberto")),
        ("ui.dialog.confirm", e(), Fails("nenhum diálogo aberto")),
        ("ui.debug.state", e(), Y),
        ("ui.dialog.cancel", e(), Y),
        ("ui.debug.health", e(), Y),
        ("ui.dialog.cancel", e(), Y),
        ("ui.debug.journal", e(), Y),
        ("ui.dialog.cancel", e(), Y),
        ("ui.debug.reload-commands", e(), Y),
        ("ui.debug.reload-settings", e(), Y),
        ("ui.debug.clear-caches", e(), Y),
        // ---------- workspace ----------
        ("ui.workspace.open", json!({"id": ctx.wid}), Y),
        ("ui.workspace.refresh", e(), Y),
        ("ui.workspace.next", e(), Y),
        ("ui.workspace.prev", e(), Y),
        ("ui.workspace.switch-1", e(), Y),
        ("ui.workspace.switch-2", e(), Fails("só há um workspace aberto")),
        ("ui.workspace.switch-3", e(), Fails("só há um workspace aberto")),
        ("ui.workspace.switch-4", e(), Fails("só há um workspace aberto")),
        ("ui.workspace.switch-5", e(), Fails("só há um workspace aberto")),
        ("ui.workspace.switch-6", e(), Fails("só há um workspace aberto")),
        ("ui.workspace.switch-7", e(), Fails("só há um workspace aberto")),
        ("ui.workspace.switch-8", e(), Fails("só há um workspace aberto")),
        ("ui.workspace.switch-9", e(), Fails("só há um workspace aberto")),
        ("ui.workspace.edit", e(), Y),
        ("ui.settings.close", e(), Y),
        ("ui.workspace.new", e(), Y),
        ("ui.workspace.set-path", json!({"path": ctx.ws}), Y),
        ("ui.state.set", json!({"patch": {"newWorkspaceOpen": false}}), Y),
        ("ui.workspace.pick", e(), Y),
        ("ui.state.set", json!({"patch": {"pickingWorkspaces": false}}), Y),
        // ---------- painéis ----------
        ("ui.palette.toggle", e(), Y),
        ("ui.palette.toggle", e(), Y),
        ("ui.palette.commands", e(), Y),
        // a lista de comandos não fica presa no tamanho da busca anterior
        (CHECK_PALETTE_HEIGHT, json!(300.0), Y),
        ("ui.palette.open", json!({"query": "tema"}), Y),
        ("ui.palette.threads", e(), Y),
        ("ui.palette.toggle", e(), Y),
        ("ui.settings.open", e(), Y),
        ("ui.settings.workspaces", e(), Y),
        ("ui.settings.appearance", e(), Y),
        ("ui.settings.defaults", e(), Y),
        ("ui.settings.accounts", e(), Y),
        ("ui.settings.providers", e(), Y),
        ("ui.settings.skills", e(), Y),
        ("ui.settings.agents", e(), Y),
        ("ui.settings.mcp", e(), Y),
        ("ui.settings.git", e(), Y),
        ("ui.settings.integrations", e(), Y),
        ("ui.settings.shortcuts", e(), Y),
        ("ui.settings.about", e(), Y),
        ("ui.help.shortcuts", e(), Y),
        ("ui.help.about", e(), Y),
        ("ui.settings.toggle", e(), Y),
        ("ui.settings.toggle", e(), Y),
        ("ui.settings.close", e(), Y),
        ("ui.sidebar.toggle", e(), Y),
        ("ui.sidebar.toggle", e(), Y),
        ("ui.sidebar.mode.recent", e(), Y),
        ("ui.sidebar.mode.project", e(), Y),
        ("ui.sidebar.mode.toggle", e(), Y),
        ("ui.sidebar.mode.toggle", e(), Y),
        ("ui.sidebar.archived.toggle", e(), Y),
        ("ui.sidebar.archived.toggle", e(), Y),
        ("ui.sidebar.filter", json!({"text": "nada-com-isso"}), Y),
        ("ui.sidebar.clear-filter", e(), Y),
        ("ui.sidebar.collapse-all", e(), Y),
        ("ui.sidebar.expand-all", e(), Y),
        ("ui.sidebar.show-all", e(), Y),
        ("ui.sidebar.width-reset", e(), Y),
        ("ui.toasts.clear", e(), Y),
        ("ui.notifications.test", e(), Y),
        // ---------- threads ----------
        ("ui.thread.select", json!({"id": ctx.tid}), Y),
        ("ui.thread.new", e(), Y),
        ("ui.thread.new-isolated", json!({"projectId": ctx.pid}), Y),
        ("ui.thread.next", e(), Y),
        ("ui.thread.prev", e(), Y),
        ("ui.thread.first", e(), Y),
        ("ui.thread.last", e(), Y),
        ("ui.thread.open-1", e(), Y),
        ("ui.thread.open-2", e(), Y),
        ("ui.thread.open-3", e(), Y),
        ("ui.thread.open-4", e(), Y),
        ("ui.thread.open-5", e(), Fails("só há quatro threads")),
        ("ui.thread.open-6", e(), Fails("só há quatro threads")),
        ("ui.thread.open-7", e(), Fails("só há quatro threads")),
        ("ui.thread.open-8", e(), Fails("só há quatro threads")),
        ("ui.thread.open-9", e(), Fails("só há quatro threads")),
        ("ui.thread.close", e(), Y),
        ("ui.thread.select", json!({"id": ctx.tid}), Y),
        ("ui.thread.rename", json!({"title": "renomeada"}), Y),
        ("ui.thread.rename", e(), Y),
        (CHECK_INSPECT, json!({"path": "/editingTitle", "equals": "renomeada"}), Y),
        ("ui.thread.toggle-bookmark", e(), Y),
        ("ui.thread.toggle-bookmark", e(), Y),
        ("ui.thread.mark-unread", e(), Y),
        // a não lida aparece na atividade; ir para a próxima a abre e a dá por lida
        ("ui.activity.toggle", e(), Y),
        (CHECK_INSPECT, json!({"path": "/activity/0/id", "equals": ctx.tid}), Y),
        (CHECK_INSPECT, json!({"path": "/state/activityOpen", "equals": true}), Y),
        ("ui.activity.toggle", e(), Y),
        ("ui.activity.next", e(), Y),
        (CHECK_INSPECT, json!({"path": "/activity", "equals": []}), Y),
        ("ui.activity.next", e(), Fails("nada pede atenção")),
        ("ui.composer.focus", e(), Y),
        ("ui.composer.insert", json!({"text": "rascunho"}), Y),
        ("ui.composer.slash", json!({"name": "help"}), Y),
        ("ui.composer.clear", e(), Y),
        ("ui.composer.attach-paths", json!({"paths": [ctx.png]}), Y),
        ("ui.composer.attach-data", json!({"mime": "image/png", "data": ctx.png_b64, "name": "p.png"}), Y),
        ("ui.composer.attach", e(), Y),
        ("ui.composer.paste-image", e(), Y),
        ("ui.composer.clear", e(), Y),
        // o provedor só troca antes da primeira mensagem
        ("ui.thread.provider.codex", e(), Y),
        ("ui.thread.provider.gemini", e(), Y),
        ("ui.thread.provider.claude", e(), Y),
        ("ui.thread.send", json!({"text": "faça algo"}), Y),
        ("ui.thread.provider.codex", e(), Fails("a thread já tem conversa")),
        ("ui.thread.copy-last-message", e(), Y),
        ("ui.composer.recall", e(), Y),
        ("ui.composer.clear", e(), Y),
        ("ui.thread.copy-path", e(), Y),
        ("ui.thread.copy-branch", e(), Y),
        ("ui.thread.copy-session", e(), Y),
        ("ui.thread.copy-transcript", e(), Y),
        ("ui.thread.copy-id", e(), Y),
        ("ui.thread.open-folder", e(), Y),
        ("ui.thread.reload", e(), Y),
        ("ui.thread.scroll-bottom", e(), Y),
        ("ui.thread.scroll-top", e(), Y),
        ("ui.thread.expand-tools", e(), Y),
        ("ui.thread.collapse-tools", e(), Y),
        ("ui.thread.expand-thinking", e(), Y),
        ("ui.thread.collapse-thinking", e(), Y),
        ("ui.thread.interrupt", e(), Y),
        ("ui.permission.allow", e(), Fails("nenhuma permissão pendente")),
        // o agente pede permissão; cada decisão responde ao CLI e o turno termina
        ("ui.thread.send", json!({"text": "pedir permissão"}), Y),
        (WAIT_PERMISSION, json!(true), Y),
        ("ui.permission.allow", e(), Y),
        (WAIT_PERMISSION, json!(false), Y),
        ("ui.thread.send", json!({"text": "pedir de novo"}), Y),
        (WAIT_PERMISSION, json!(true), Y),
        ("ui.permission.always", e(), Y),
        (WAIT_PERMISSION, json!(false), Y),
        ("ui.thread.send", json!({"text": "pedir mais uma"}), Y),
        (WAIT_PERMISSION, json!(true), Y),
        // relida do histórico (aberta depois do pedido, de outra janela), a
        // conversa ainda mostra o que responder
        ("ui.debug.clear-caches", e(), Y),
        (WAIT_PERMISSION, json!(true), Y),
        ("ui.permission.deny", e(), Y),
        (WAIT_PERMISSION, json!(false), Y),
        ("ui.permission.deny", e(), Fails("nenhuma permissão pendente")),
        ("ui.thread.model.default", e(), Y),
        ("ui.thread.model.opus", e(), Y),
        ("ui.thread.model.opus-1m", e(), Y),
        ("ui.thread.model.sonnet", e(), Y),
        ("ui.thread.model.sonnet-1m", e(), Y),
        ("ui.thread.model.haiku", e(), Y),
        ("ui.thread.model.fable", e(), Y),
        ("ui.thread.mode.default", e(), Y),
        ("ui.thread.mode.manual", e(), Y),
        ("ui.thread.mode.acceptedits", e(), Y),
        ("ui.thread.mode.auto", e(), Y),
        ("ui.thread.mode.plan", e(), Y),
        ("ui.thread.mode.bypasspermissions", e(), Y),
        ("ui.thread.mode.cycle", e(), Y),
        ("ui.thread.effort.default", e(), Y),
        ("ui.thread.effort.low", e(), Y),
        ("ui.thread.effort.medium", e(), Y),
        ("ui.thread.effort.high", e(), Y),
        ("ui.thread.effort.xhigh", e(), Y),
        ("ui.thread.effort.max", e(), Y),
        ("ui.thread.skip-permissions.toggle", e(), Y),
        ("ui.thread.skip-permissions.toggle", e(), Y),
        ("ui.thread.duplicate", e(), Y),
        // ---------- revisão (na thread isolada, que tem checkout próprio) ----------
        ("ui.thread.select", json!({"id": ctx.iso}), Y),
        // o título em edição era da outra thread: não pode seguir aberto nesta
        (CHECK_INSPECT, json!({"path": "/editingTitle", "equals": null}), Y),
        ("ui.history.review", e(), Y),
        ("ui.review.base.upstream", e(), Y),
        ("ui.review.base.head", e(), Y),
        ("ui.review.base.turn", e(), Y),
        ("ui.review.base.upstream", e(), Y),
        ("ui.review.refresh", e(), Y),
        ("ui.review.first-file", e(), Y),
        ("ui.review.next-file", e(), Y),
        ("ui.review.prev-file", e(), Y),
        ("ui.review.last-file", e(), Y),
        ("ui.review.open-file", json!({"path": "iso.txt"}), Y),
        ("ui.review.copy-patch", e(), Y),
        ("ui.review.toggle-seen", e(), Y),
        ("ui.review.mark-all-seen", e(), Y),
        ("ui.review.mark-none-seen", e(), Y),
        ("ui.review.discard-file", json!({"path": "iso.txt"}), Y),
        ("ui.dialog.cancel", e(), Y),
        ("ui.thread.commit", json!({"message": "feat: iso"}), Y),
        ("ui.thread.push", e(), Y),
        ("ui.thread.update-from-base", e(), Y),
        ("ui.thread.land-merge", e(), Fails("o checkout principal tem mudanças não commitadas")),
        ("ui.thread.land-apply", e(), Y),
        ("ui.thread.land-pr", e(), Y),
        ("ui.thread.terminal", e(), Y),
        // ---------- abas ----------
        ("ui.thread.select", json!({"id": ctx.tid}), Y),
        ("ui.tab.chat", e(), Y),
        ("ui.tab.history", e(), Y),
        ("ui.tab.tree", e(), Y),
        ("ui.tab.reflog", e(), Y),
        ("ui.tab.stats", e(), Y),
        ("ui.tab.terminal", e(), Y),
        ("ui.tab.next", e(), Y),
        ("ui.tab.prev", e(), Y),
        ("ui.history.graph", e(), Y),
        // ---------- terminal ----------
        ("ui.tab.terminal", e(), Y),
        ("ui.terminal.write", json!({"text": "echo oi\r"}), Y),
        ("ui.terminal.run", json!({"command": "echo ola"}), Y),
        ("ui.terminal.clear", e(), Y),
        ("ui.terminal.interrupt", e(), Y),
        ("ui.terminal.copy-screen", e(), Y),
        ("ui.terminal.restart", e(), Y),
        // ---------- repositório e git ----------
        ("ui.repo.open", json!({"projectId": ctx.pid}), Y),
        ("ui.repo.close", e(), Y),
        ("ui.repo.open-current", e(), Fails("sem thread nem repositório aberto")),
        ("ui.thread.select", json!({"id": ctx.tid}), Y),
        ("ui.repo.open-current", e(), Y),
        ("ui.tab.history", e(), Y),
        ("ui.git.refresh", e(), Y),
        ("ui.git.select-working", e(), Y),
        ("ui.git.stage-all", e(), Y),
        ("ui.git.unstage-all", e(), Y),
        ("ui.git.stash", json!({"message": "wip"}), Y),
        ("ui.git.stash-apply", e(), Y),
        ("ui.git.stash-drop", json!({"index": 0}), Y),
        ("ui.git.stash", e(), Y),
        ("ui.git.stash-pop", e(), Y),
        ("ui.git.stage-all", e(), Y),
        ("ui.git.amend.toggle", e(), Y),
        ("ui.git.amend.toggle", e(), Y),
        ("ui.git.ai-message", e(), Y),
        ("ui.git.commit", json!({"summary": "feat: mudanças da tela", "body": "corpo"}), Y),
        ("ui.git.commit", json!({"summary": "nada"}), Fails("nada no staging")),
        ("ui.git.undo", e(), Y),
        ("ui.git.redo", e(), Y),
        ("ui.git.discard-all", e(), Y),
        ("ui.dialog.cancel", e(), Y),
        ("ui.git.fetch", e(), Y),
        ("ui.git.pull", e(), Y),
        ("ui.git.push", e(), Y),
        ("ui.git.force-push", e(), Y),
        ("ui.git.load-more", e(), Y),
        ("ui.git.toggle-refs", e(), Y),
        ("ui.git.toggle-refs", e(), Y),
        ("ui.git.commit.copy-sha", e(), Fails("nenhum commit selecionado")),
        ("ui.git.select-commit", json!({"sha": ctx.v1}), Y),
        ("ui.git.commit.copy-sha", e(), Y),
        ("ui.git.commit.branch-here", e(), Y),
        ("ui.git.commit.tag-here", e(), Y),
        ("ui.git.commit.cherry-pick", json!({"sha": ctx.feature}), Y),
        ("ui.git.commit.revert", json!({"sha": ctx.feature}), Y),
        ("ui.git.commit.checkout", e(), Y),
        ("ui.git.checkout", json!({"name": "main"}), Y),
        ("ui.git.commit.reset-mixed", e(), Y),
        ("ui.git.commit.reset-hard", e(), Y),
        ("ui.dialog.confirm", e(), Y),
        ("ui.git.new-branch", e(), Y),
        ("ui.git.new-branch", json!({"name": "nova"}), Y),
        ("ui.git.checkout", json!({"name": "main"}), Y),
        ("ui.git.checkout", json!({"name": "nao-existe"}), Fails("branch inexistente")),
        ("ui.git.checkout", json!({"nome": "main"}), Fails("parâmetro com nome errado")),
        ("ui.git.merge", json!({"name": "feature"}), Y),
        ("ui.git.rebase-onto", json!({"name": "nova"}), Y),
        ("ui.git.delete-branch", json!({"name": "nova"}), Y),
        ("ui.dialog.confirm", e(), Y),
        ("ui.git.rebase-continue", e(), Fails("não há rebase em andamento")),
        ("ui.git.rebase-abort", e(), Y),
        ("ui.git.conflicts.refresh", e(), Y),
        ("ui.git.blame", json!({"path": "a.txt"}), Y),
        ("ui.git.file-history", json!({"path": "a.txt"}), Y),
        ("ui.git.compare", json!({"b": "feature"}), Y),
        ("ui.git.search", json!({"query": "feat"}), Y),
        ("ui.git.worktrees", e(), Y),
        ("ui.git.worktree-prune", e(), Y),
        ("ui.git.remotes", e(), Y),
        ("ui.git.submodules", e(), Y),
        ("ui.git.lfs-pull", e(), if ctx.lfs { Y } else { Fails("git-lfs não está instalado") }),
        ("ui.git.interactive-rebase", json!({"sha": ctx.v1}), Y),
        ("ui.git.open-pr", e(), Y),
        // ---------- aparência ----------
        ("ui.theme.system", e(), Y),
        ("ui.theme.dark", e(), Y),
        ("ui.theme.tokyo", e(), Y),
        ("ui.theme.dracula", e(), Y),
        ("ui.theme.one-dark", e(), Y),
        ("ui.theme.nord", e(), Y),
        ("ui.theme.gruvbox", e(), Y),
        ("ui.theme.solarized", e(), Y),
        ("ui.theme.catppuccin", e(), Y),
        ("ui.theme.rose-pine", e(), Y),
        ("ui.theme.light", e(), Y),
        ("ui.theme.catppuccin-latte", e(), Y),
        ("ui.theme.rose-pine-dawn", e(), Y),
        ("ui.theme.solarized-light", e(), Y),
        ("ui.theme.next", e(), Y),
        ("ui.density.compact", e(), Y),
        ("ui.density.normal", e(), Y),
        ("ui.density.comfortable", e(), Y),
        ("ui.density.toggle", e(), Y),
        ("ui.width.narrow", e(), Y),
        ("ui.width.normal", e(), Y),
        ("ui.width.wide", e(), Y),
        ("ui.width.full", e(), Y),
        ("ui.zoom.in", e(), Y),
        ("ui.zoom.in-alt", e(), Y),
        ("ui.zoom.out", e(), Y),
        ("ui.zoom.set", json!({"percent": 130}), Y),
        ("ui.zoom.reset", e(), Y),
        ("ui.font.text.bigger", e(), Y),
        ("ui.font.text.smaller", e(), Y),
        ("ui.font.code.bigger", e(), Y),
        ("ui.font.code.smaller", e(), Y),
        ("ui.font.terminal.bigger", e(), Y),
        ("ui.font.terminal.smaller", e(), Y),
        ("ui.appearance.reset", e(), Y),
        ("ui.language.en", e(), Y),
        ("ui.language.es", e(), Y),
        ("ui.language.zh", e(), Y),
        ("ui.language.next", e(), Y),
        ("ui.language.pt", e(), Y),
        // ---------- janela ----------
        ("ui.window.fullscreen", e(), Y),
        ("ui.window.minimize", e(), Y),
        ("ui.window.maximize", e(), Y),
        ("ui.window.always-on-top", e(), Y),
        // ---------- o que destrói vem por último ----------
        ("ui.thread.select", json!({"id": ctx.tid}), Y),
        ("ui.thread.archive", e(), Y),
        ("ui.thread.archive", e(), Y),
        ("ui.thread.delete", e(), Y),
        ("ui.dialog.confirm", e(), Y),
        ("ui.workspace.close", e(), Y),
        ("ui.app.quit", e(), Y),
        ("ui.inspect", v(json!({})), Y),
    ]
}

struct Ctx {
    ws: String,
    /// `main` no início (a tag `v1`) e o commit da branch `feature`
    v1: String,
    feature: String,
    lfs: bool,
    wid: String,
    pid: String,
    tid: String,
    iso: String,
    png: String,
    png_b64: String,
}

#[cfg(unix)]
#[test]
fn every_screen_command_runs_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    // git, editores e navegador isolados da máquina
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let opened = root.join("opened.log");
    for name in ["xdg-open", "open", "wslview", "notify-send", "code"] {
        script(&bin.join(name), &format!("#!/bin/sh\necho \"{name} $*\" >> '{}'\n", opened.display()));
    }
    let claude = bin.join("claude");
    script(&claude, FAKE_CLAUDE);
    let gitconfig = root.join("gitconfig");
    std::fs::write(&gitconfig, "[user]\n\tname = Demo\n\temail = demo@example.com\n[init]\n\tdefaultBranch = main\n").unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", path);
    std::env::set_var("LAZYAICODE_CLAUDE_BIN", &claude);
    std::env::set_var("GIT_CONFIG_GLOBAL", &gitconfig);
    std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
    std::env::set_var("LAZYAICODE_CODEX_ACP", bin.join("sem-codex"));
    std::env::set_var("LAZYAICODE_GEMINI", bin.join("sem-gemini"));

    let (ws, repo) = fixture(root);
    let v1 = git(&repo, &["rev-parse", "v1"]).trim().to_string();
    let feature = git(&repo, &["rev-parse", "feature"]).trim().to_string();
    let lfs = std::process::Command::new("git").args(["lfs", "version"]).output().is_ok_and(|o| o.status.success());
    let png = root.join("p.png");
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([200, 30, 30, 255]));
    img.save(&png).unwrap();
    let png_b64 = base64::engine::general_purpose::STANDARD.encode(std::fs::read(&png).unwrap());

    lazyaicode_engine::workspace::init_platform();
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let engine = {
        let _g = rt.enter();
        Engine::new(EngineOpts { data_dir: Some(root.join("data")), ephemeral: false })
    };
    let (wid, pid, tid, iso) = rt.block_on(async {
        let w = engine.run("workspace.create", json!({"path": ws.to_string_lossy()})).await.unwrap();
        let wid = w["id"].as_str().unwrap().to_string();
        engine.run("workspace.activate", json!({"id": wid})).await.unwrap();
        let projects = engine.run("project.list", json!({"workspaceId": wid})).await.unwrap();
        let app = projects.as_array().unwrap().iter().find(|p| p["name"] == "app").unwrap();
        let pid = app["id"].as_str().unwrap().to_string();
        let t = engine.run("thread.create", json!({"workspaceId": wid, "projectId": pid, "isolated": false})).await.unwrap();
        let i = engine.run("thread.create", json!({"workspaceId": wid, "projectId": pid, "isolated": true})).await.unwrap();
        (wid, pid, t["id"].as_str().unwrap().to_string(), i["id"].as_str().unwrap().to_string())
    });
    // a thread isolada muda um arquivo no checkout dela
    let iso_cwd = rt.block_on(engine.run("thread.get", json!({"id": iso}))).unwrap()["cwd"].as_str().unwrap().to_string();
    std::fs::write(Path::new(&iso_cwd).join("iso.txt"), "isolado\n").unwrap();

    let wake: local::Wake = Arc::new(OnceLock::new());
    let ctx = egui::Context::default();
    let _ = wake.set(ctx.clone());
    lazyaicode_ui_egui::install(&ctx);
    let client = local::LocalClient::new(engine.clone(), rt.handle().clone(), wake);
    let inbox = Inbox::default();
    let host = TestHost { inbox: inbox.clone(), png: png.clone(), folder: ws.clone() };
    let app = LazyApp::new(Box::new(client), Box::new(host), inbox);
    let mut h = Harness {
        app,
        ctx,
        rt,
        engine,
        tex: lazyaicode_render_cpu::Textures::new(),
        frame: 0,
        shots: std::env::var_os("UI_E2E_SHOTS").map(PathBuf::from),
        last_png: None,
    };
    // a abertura carrega comandos, workspaces e abre o único workspace
    let booted = h.frames_until(30, |h| {
        let s = h.inspect();
        s["workspace"]["id"] == json!(wid) && s["counts"]["threads"].as_u64() == Some(2) && !h.app.has_pending()
    });
    assert_eq!(booted, Ok(true), "a abertura não carregou o workspace: {}", h.inspect());

    let c = Ctx { ws: ws.to_string_lossy().into(), v1, feature, lfs, wid, pid, tid, iso, png: png.to_string_lossy().into(), png_b64 };
    let cases = table(&c);

    // cobertura: todo comando de tela está no roteiro
    let all: BTreeSet<String> = lazyaicode_ui_egui::commands::ui_command_infos().into_iter().map(|c| c.id).collect();
    let covered: HashSet<&str> = cases.iter().map(|c| c.0).filter(|id| !id.starts_with('@')).collect();
    let missing: Vec<&String> = all.iter().filter(|id| !covered.contains(id.as_str())).collect();
    let unknown: Vec<&&str> = covered.iter().filter(|id| !all.contains(**id)).collect();
    assert!(missing.is_empty() && unknown.is_empty(), "comandos fora do roteiro: {missing:?}; ids que não existem: {unknown:?}");

    let mut failures = Vec::new();
    for (id, params, expect) in cases {
        let out = if id == WAIT_PERMISSION {
            match h.frames_until(20, |h| h.inspect()["thread"]["pendingPermission"] == params) {
                Ok(true) => Outcome::Ok,
                Ok(false) => Outcome::Timeout,
                Err(p) => Outcome::Panic(p),
            }
        } else if id == CHECK_INSPECT {
            let got = h.inspect().pointer(params["path"].as_str().unwrap_or("")).cloned().unwrap_or(Value::Null);
            if got == params["equals"] {
                Outcome::Ok
            } else {
                Outcome::Err(format!("ui.inspect{} = {got}", params["path"].as_str().unwrap_or("")))
            }
        } else if id == CHECK_PALETTE_HEIGHT {
            let height = h.ctx.memory(|m| m.area_rect(egui::Id::new("palette"))).map_or(0.0, |r| r.height());
            if f64::from(height) >= params.as_f64().unwrap_or(0.0) {
                Outcome::Ok
            } else {
                Outcome::Err(format!("a paleta tem {height}px"))
            }
        } else {
            h.run(id, params.clone())
        };
        let bad = !matches!((&out, &expect), (Outcome::Ok, Expect::Ok) | (Outcome::Err(_), Expect::Fails(_)));
        if std::env::var("UI_E2E_VERBOSE").is_ok() {
            eprintln!("{id} {params} → {out}");
        }
        if bad {
            let want = match expect {
                Expect::Ok => "Ok".to_string(),
                Expect::Fails(why) => format!("falha ({why})"),
            };
            failures.push(format!("{id} {params}: esperado {want}, veio {out}"));
        }
    }
    // os comandos tiveram efeito de verdade
    let opened = std::fs::read_to_string(&opened).unwrap_or_default();
    if !opened.contains("open") {
        failures.push(format!("ui.thread.open-folder não abriu a pasta: {opened:?}"));
    }
    let log = git(&repo, &["log", "--format=%s", "main"]);
    // o reset-hard voltou a main para a v1; o merge trouxe a feature
    if !log.contains("feat: f") {
        failures.push(format!("ui.git.merge não trouxe a feature para a main:\n{log}"));
    }
    if git(&repo, &["branch", "--list", "nova"]).contains("nova") {
        failures.push("ui.git.delete-branch + ui.dialog.confirm não apagou a branch".into());
    }
    let pushed = git(&origin_dir(root), &["log", "--format=%s", "main"]);
    if !pushed.contains("feat: mudanças da tela") {
        failures.push(format!("ui.git.push não chegou ao remoto:\n{pushed}"));
    }
    let threads = h.rt.block_on(h.engine.run("thread.list", json!({"workspaceId": c.wid}))).unwrap_or_default();
    let iso_title = threads.as_array().into_iter().flatten().find(|t| t["id"] == json!(c.iso)).map(|t| t["title"].clone());
    if iso_title == Some(json!("renomeada")) {
        failures.push("o título em edição de uma thread foi gravado em outra".into());
    }
    if threads.to_string().contains(&c.tid) {
        failures.push("ui.thread.delete + ui.dialog.confirm não apagou a thread".into());
    }
    let Harness { rt, engine, .. } = h;
    rt.block_on(engine.shutdown());
    assert!(failures.is_empty(), "{} comandos de tela falharam:\n{}", failures.len(), failures.join("\n"));
}

/// Janela por workspace: com a fábrica de janelas (como no desktop), abrir
/// outro workspace abre outra janela no mesmo motor em vez de outra aba;
/// abrir de novo foca a que já existe; a atividade de todos os workspaces
/// aparece em qualquer janela e "ir para a próxima" abre a thread na janela
/// do workspace dela; os workspaces abertos ficam gravados para a próxima vez.
#[cfg(unix)]
#[test]
fn each_workspace_opens_in_its_own_window() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let gitconfig = root.join("gitconfig");
    std::fs::write(&gitconfig, "[user]\n\tname = Demo\n\temail = demo@example.com\n[init]\n\tdefaultBranch = main\n").unwrap();
    std::env::set_var("GIT_CONFIG_GLOBAL", &gitconfig);
    std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
    let (ws1, ws2) = (root.join("um"), root.join("dois"));
    lazyaicode_testkit::init_repo(&ws1.join("app"));
    lazyaicode_testkit::init_repo(&ws2.join("api"));
    let png = root.join("p.png");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 0, 255])).save(&png).unwrap();

    lazyaicode_engine::workspace::init_platform();
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let engine = {
        let _g = rt.enter();
        Engine::new(EngineOpts { data_dir: Some(root.join("data")), ephemeral: false })
    };
    let (w1, w2, t2) = rt.block_on(async {
        let a = engine.run("workspace.create", json!({"path": ws1.to_string_lossy()})).await.unwrap();
        let b = engine.run("workspace.create", json!({"path": ws2.to_string_lossy()})).await.unwrap();
        let projects = engine.run("project.list", json!({"workspaceId": b["id"]})).await.unwrap();
        let t =
            engine.run("thread.create", json!({"workspaceId": b["id"], "projectId": projects[0]["id"], "isolated": false})).await.unwrap();
        (a["id"].as_str().unwrap().to_string(), b["id"].as_str().unwrap().to_string(), t["id"].as_str().unwrap().to_string())
    });
    // da última vez, os dois estavam abertos (o segundo primeiro)
    rt.block_on(engine.run("prefs.set", json!({"key": "ui.openWorkspaces", "value": [w2, w1]}))).unwrap();

    let wake: local::Wake = Arc::new(OnceLock::new());
    let ctx = egui::Context::default();
    let _ = wake.set(ctx.clone());
    lazyaicode_ui_egui::install(&ctx);
    let inbox = Inbox::default();
    let host = TestHost { inbox: inbox.clone(), png: png.clone(), folder: ws1.clone() };
    let mut app = LazyApp::new(Box::new(local::LocalClient::new(engine.clone(), rt.handle().clone(), wake.clone())), Box::new(host), inbox);
    let (e2, h2, wk2, png2, f2) = (engine.clone(), rt.handle().clone(), wake.clone(), png.clone(), ws1.clone());
    app.set_window_factory(Box::new(move || {
        let inbox = Inbox::default();
        lazyaicode_ui_egui::NewWindow {
            client: Box::new(local::LocalClient::new(e2.clone(), h2.clone(), wk2.clone())),
            host: Box::new(TestHost { inbox: inbox.clone(), png: png2.clone(), folder: f2.clone() }),
            inbox,
            builder: egui::ViewportBuilder::default().with_title("LazyAICode"),
        }
    }));
    let mut h = Harness {
        app,
        ctx,
        rt,
        engine,
        tex: lazyaicode_render_cpu::Textures::new(),
        frame: 0,
        shots: std::env::var_os("UI_E2E_SHOTS").map(PathBuf::from),
        last_png: None,
    };
    // dois workspaces: a abertura pergunta quais abrir
    let booted = h.frames_until(30, |h| h.inspect()["state"]["pickingWorkspaces"] == json!(true) && !h.app.has_pending());
    assert_eq!(booted, Ok(true), "{}", h.inspect());
    assert_eq!(h.inspect()["windows"]["multi"], json!(true));

    // o primeiro fica nesta janela (ela estava vazia)
    assert!(matches!(h.run("ui.workspace.open", json!({"id": w1})), Outcome::Ok));
    let s = h.inspect();
    assert_eq!(s["workspace"]["id"], json!(w1), "{s}");
    assert_eq!(s["windows"]["children"], json!([]));
    assert_eq!(s["state"]["openWorkspaces"], json!([w1]), "a lista da última vez não vira aba: {s}");

    // o segundo abre outra janela, com as threads dele
    assert!(matches!(h.run("ui.workspace.open", json!({"id": w2})), Outcome::Ok));
    let opened = h.frames_until(20, |h| h.inspect()["windows"]["children"][0]["threads"] == json!(1));
    assert_eq!(opened, Ok(true), "{}", h.inspect());
    let s = h.inspect();
    assert_eq!(s["workspace"]["id"], json!(w1), "a janela principal continua no primeiro: {s}");
    assert_eq!(s["state"]["openWorkspaces"], json!([w1]), "nada de aba nova: {s}");
    assert_eq!(s["windows"]["children"][0]["workspace"], json!(w2));

    // abrir de novo foca a janela que já existe
    assert!(matches!(h.run("ui.workspace.open", json!({"id": w2})), Outcome::Ok));
    assert_eq!(h.inspect()["windows"]["children"].as_array().map(Vec::len), Some(1));
    let _ = h.frames_until(2, |_| false);
    assert_eq!(h.inspect()["windows"]["others"], json!([w2]));

    // terminou sem ser lida no outro workspace: a principal vê e leva até ela
    h.rt.block_on(h.engine.run("thread.update", json!({"id": t2, "unread": true}))).unwrap();
    let seen = h.frames_until(10, |h| h.inspect()["activity"][0]["id"] == json!(t2));
    assert_eq!(seen, Ok(true), "{}", h.inspect());
    assert!(matches!(h.run("ui.activity.next", json!({})), Outcome::Ok));
    let s = h.inspect();
    assert_eq!(s["workspace"]["id"], json!(w1), "{s}");
    assert_eq!(s["windows"]["children"][0]["selected"], json!(t2), "a thread abre na janela do workspace dela: {s}");
    let read = h.frames_until(10, |h| h.inspect()["activity"] == json!([]));
    assert_eq!(read, Ok(true), "abrir a thread a dá por lida: {}", h.inspect());

    // na próxima abertura, os dois vêm marcados
    let prefs = h.rt.block_on(h.engine.run("prefs.get", json!({}))).unwrap();
    assert_eq!(prefs["ui.openWorkspaces"], json!([w1, w2]), "{prefs}");

    let Harness { rt, engine, .. } = h;
    rt.block_on(engine.shutdown());
}

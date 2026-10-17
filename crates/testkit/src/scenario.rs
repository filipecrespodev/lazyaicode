//! O cenário de todas as funcionalidades do motor: uma sequência que roda
//! **cada** comando do registro num ambiente real (repositório git com
//! remoto, workspace, threads com o `claude` falso, terminal) e confere o
//! efeito. É o mesmo cenário para todos os caminhos: em processo
//! (`Engine::run`), pelo canal de controle TCP de um `lazyaicode-cli serve`
//! e pelo MCP — basta um [`Runner`].
//!
//! Ambiente hermético: `HOME`, configuração global do git, pasta de dados e
//! `PATH` apontam para o tempdir ([`Fixture::env`]); nada abre janela,
//! navegador nem gasta token.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// Erro de um comando como os clientes veem (`ErrorBody`).
#[derive(Debug, Clone)]
pub struct Fail {
    pub code: String,
    pub message: String,
}

/// Um caminho até o motor.
pub trait Runner {
    fn call(&mut self, id: &str, params: Value) -> Result<Value, Fail>;
}

pub struct Fixture {
    pub root: PathBuf,
    pub home: PathBuf,
    pub data: PathBuf,
    pub bin: PathBuf,
    /// workspace: `app` (git, com `origin`) e `docs` (pasta comum)
    pub ws: PathBuf,
    pub repo: PathBuf,
    pub remote: PathBuf,
    pub open_log: PathBuf,
}

impl Fixture {
    /// Monta tudo em `root` (um tempdir).
    #[cfg(unix)]
    pub fn new(root: &Path) -> Self {
        let home = root.join("home");
        let data = root.join("data");
        // duas contas: `~/.claude` sem login e `~/.claude-conta`, a ativa ($CLAUDE_CONFIG_DIR)
        std::fs::create_dir_all(home.join(".claude").join("projects")).unwrap();
        std::fs::create_dir_all(home.join(".claude-conta").join("projects")).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(
            home.join(".gitconfig"),
            "[user]\n\tname = Demo\n\temail = demo@example.com\n[init]\n\tdefaultBranch = main\n[core]\n\tautocrlf = false\n",
        )
        .unwrap();
        let bin = crate::fakes::fake_bin_dir(root);
        let ws = root.join("ws");
        let repo = ws.join("app");
        let remote = root.join("remote.git");
        let fx = Self { root: root.to_path_buf(), home, data, bin, ws, repo, remote, open_log: root.join("open.log") };
        let envs = fx.env();
        let g = |dir: &Path, args: &[&str]| {
            let out =
                std::process::Command::new("git").args(args).current_dir(dir).envs(envs.iter().map(|(k, v)| (k, v))).output().unwrap();
            assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        };
        std::fs::create_dir_all(&fx.remote).unwrap();
        g(&fx.remote, &["init", "-q", "--bare", "-b", "main"]);
        crate::init_repo(&fx.repo);
        std::fs::write(fx.repo.join("b.txt"), "linha 1\nlinha 2\nlinha 3\n").unwrap();
        g(&fx.repo, &["add", "."]);
        g(&fx.repo, &["commit", "-qm", "feat: b"]);
        g(&fx.repo, &["remote", "add", "origin", &fx.remote.to_string_lossy()]);
        g(&fx.repo, &["push", "-q", "-u", "origin", "main"]);
        g(&fx.repo, &["checkout", "-qb", "feature"]);
        std::fs::write(fx.repo.join("c.txt"), "da feature\n").unwrap();
        g(&fx.repo, &["add", "."]);
        g(&fx.repo, &["commit", "-qm", "feat: c"]);
        g(&fx.repo, &["push", "-q", "-u", "origin", "feature"]);
        g(&fx.repo, &["checkout", "-q", "main"]);
        std::fs::create_dir_all(fx.ws.join("docs")).unwrap();
        std::fs::write(fx.ws.join("docs").join("leia.md"), "# docs\n").unwrap();
        // uma imagem PNG 1x1 para `system.read-image`
        std::fs::write(fx.root.join("img.png"), PNG_1X1).unwrap();
        // uma skill da conta para os catálogos
        let skill = fx.account().join("skills").join("demo");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join("SKILL.md"), "---\nname: demo\ndescription: skill de teste\n---\nfaça algo\n").unwrap();
        fx
    }

    /// A conta do Claude com login (a de `$CLAUDE_CONFIG_DIR`).
    pub fn account(&self) -> PathBuf {
        self.home.join(".claude-conta")
    }

    /// Variáveis de ambiente do processo que roda o motor.
    pub fn env(&self) -> Vec<(String, OsString)> {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![self.bin.clone()];
        paths.extend(std::env::split_paths(&path));
        let s = |p: &Path| p.as_os_str().to_os_string();
        vec![
            ("PATH".into(), std::env::join_paths(paths).unwrap_or(path)),
            ("HOME".into(), s(&self.home)),
            ("XDG_CONFIG_HOME".into(), s(&self.home.join(".config"))),
            ("XDG_DATA_HOME".into(), s(&self.home.join(".local/share"))),
            ("GIT_CONFIG_GLOBAL".into(), s(&self.home.join(".gitconfig"))),
            ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
            ("GIT_TERMINAL_PROMPT".into(), "0".into()),
            ("LAZYAICODE_DATA_DIR".into(), s(&self.data)),
            ("LAZYAICODE_CLAUDE_BIN".into(), s(&self.bin.join("claude"))),
            ("LAZYAICODE_CODEX_ACP".into(), s(&self.bin.join("codex-acp"))),
            ("LAZYAICODE_GEMINI".into(), s(&self.bin.join("gemini"))),
            ("FAKE_OPEN_LOG".into(), s(&self.open_log)),
            ("CLAUDE_CONFIG_DIR".into(), s(&self.account())),
            ("FAKE_CLAUDE_LOGGED_IN".into(), s(&self.account())),
        ]
    }

    /// Aplica [`Self::env`] ao processo atual (motor em processo).
    pub fn apply_env(&self) {
        for (k, v) in self.env() {
            std::env::set_var(k, v);
        }
    }

    /// `git` com o ambiente hermético; devolve a saída.
    pub fn git(&self, dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git").args(args).current_dir(dir).envs(self.env()).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn opened(&self) -> String {
        std::fs::read_to_string(&self.open_log).unwrap_or_default()
    }
}

const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
    0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63,
    0xf8, 0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

/// O resultado: o que foi coberto e o que falhou.
#[derive(Default, Debug)]
pub struct Report {
    pub covered: BTreeSet<String>,
    pub failures: Vec<String>,
    pub calls: usize,
}

impl Report {
    /// Falha o teste listando todas as falhas e os comandos sem cobertura.
    pub fn assert_complete(&self, all: &[String]) {
        let missing: Vec<&String> = all.iter().filter(|id| !self.covered.contains(*id)).collect();
        let mut msg = String::new();
        if !self.failures.is_empty() {
            msg.push_str(&format!("{} falha(s):\n  {}\n", self.failures.len(), self.failures.join("\n  ")));
        }
        if !missing.is_empty() {
            msg.push_str(&format!("{} comando(s) sem teste no cenário: {missing:?}\n", missing.len()));
        }
        assert!(msg.is_empty(), "{msg}");
    }
}

pub struct Sweep<'a> {
    r: &'a mut dyn Runner,
    pub fx: &'a Fixture,
    pub report: Report,
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 300 {
        format!("{}…", s.chars().take(300).collect::<String>())
    } else {
        s
    }
}

impl<'a> Sweep<'a> {
    pub fn new(r: &'a mut dyn Runner, fx: &'a Fixture) -> Self {
        Self { r, fx, report: Report::default() }
    }

    /// Roda e devolve o resultado cru. Um pânico (`internal`) é sempre falha.
    pub fn call(&mut self, id: &str, params: Value) -> Result<Value, Fail> {
        self.report.covered.insert(id.to_string());
        self.report.calls += 1;
        let r = self.r.call(id, params.clone());
        if let Err(f) = &r {
            if f.code == "internal" {
                self.fail(format!("{id} {}: PÂNICO {}", short(&params), f.message));
            }
        }
        r
    }

    pub fn fail(&mut self, msg: String) {
        eprintln!("FALHA {msg}");
        self.report.failures.push(msg);
    }

    /// Precisa dar certo; devolve o resultado (ou `null` registrando a falha).
    pub fn ok(&mut self, id: &str, params: Value) -> Value {
        match self.call(id, params.clone()) {
            Ok(v) => v,
            Err(f) => {
                if f.code != "internal" {
                    self.fail(format!("{id} {}: [{}] {}", short(&params), f.code, f.message));
                }
                Value::Null
            }
        }
    }

    /// Precisa falhar com um erro tratado (não pânico).
    pub fn err(&mut self, id: &str, params: Value, why: &str) -> Option<Fail> {
        match self.call(id, params.clone()) {
            Ok(v) => {
                self.fail(format!("{id} {}: devia falhar ({why}), deu {}", short(&params), short(&v)));
                None
            }
            Err(f) => Some(f),
        }
    }

    pub fn check(&mut self, cond: bool, what: impl std::fmt::Display) {
        if !cond {
            self.fail(format!("conferência: {what}"));
        }
    }

    /// Repete `f` até devolver `true` ou estourar o prazo.
    pub fn eventually(&mut self, what: &str, timeout: Duration, mut f: impl FnMut(&mut Self) -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if f(self) {
                return true;
            }
            if Instant::now() > deadline {
                self.fail(format!("tempo esgotado esperando: {what}"));
                return false;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn arr(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

/// Roda o cenário inteiro.
pub fn run(r: &mut dyn Runner, fx: &Fixture) -> Report {
    let mut sw = Sweep::new(r, fx);
    engine_and_settings(&mut sw);
    let ids = workspace_and_projects(&mut sw);
    threads(&mut sw, &ids);
    review(&mut sw, &ids);
    terminals(&mut sw, &ids);
    providers_and_system(&mut sw, &ids);
    git_path(&mut sw);
    git_repo(&mut sw);
    forges_and_wsl(&mut sw);
    cleanup(&mut sw, &ids);
    sw.report
}

fn engine_and_settings(sw: &mut Sweep) {
    let v = sw.ok("engine.ping", json!({}));
    sw.check(v == json!("pong"), "engine.ping = pong");
    let v = sw.ok("engine.version", json!({}));
    sw.check(!v.is_null(), "engine.version");
    let v = sw.ok("engine.commands", json!({"filter": "thread."}));
    sw.check(arr(&v).iter().all(|c| s(&c["id"]).contains("thread")) && !arr(&v).is_empty(), "engine.commands filtra");
    let v = sw.ok("engine.commands", json!({"group": "git"}));
    sw.check(arr(&v).len() > 50, "engine.commands por grupo");
    let v = sw.ok("engine.describe", json!({"id": "thread.create"}));
    sw.check(v["id"] == "thread.create", "engine.describe");
    sw.err("engine.describe", json!({"id": "nao.existe"}), "id desconhecido");
    let v = sw.ok("engine.groups", json!({}));
    sw.check(v.to_string().contains("git"), "engine.groups");
    let v = sw.ok("engine.events", json!({}));
    sw.check(v.to_string().contains("thread-status"), "engine.events lista thread-status");
    sw.ok("engine.state", json!({}));
    sw.ok("engine.models", json!({}));
    sw.ok("engine.permission-modes", json!({}));
    sw.ok("engine.efforts", json!({}));
    let v = sw.ok("engine.limits", json!({}));
    sw.check(v["maxRequestLine"].as_u64().unwrap_or(0) > 0, "engine.limits");
    let v = sw.ok(
        "engine.batch",
        json!({"steps": [{"command": "engine.ping"}, {"command": "nao.existe"}, {"command": "engine.ping"}], "stopOnError": false}),
    );
    sw.check(arr(&v).len() == 3 && v[1]["ok"] == json!(false), format!("engine.batch sem parar: {}", short(&v)));

    // preferências e configurações
    let before = sw.ok("settings.get", json!({}));
    sw.check(before["settings"].is_object(), "settings.get tem settings");
    let mut changed = before["settings"].clone();
    changed["default_model"] = json!("opus");
    sw.ok("settings.save", json!({"settings": changed}));
    let v = sw.ok("settings.get", json!({}));
    sw.check(v["settings"]["default_model"] == "opus", format!("settings.save gravou: {}", short(&v)));
    for (id, val) in [
        ("settings.default-model", json!("sonnet")),
        ("settings.default-permission-mode", json!("acceptEdits")),
        ("settings.send-with-modifier", json!(true)),
        ("settings.notify-on-finish", json!(false)),
        ("settings.sound-on-finish", json!(false)),
        ("settings.default-provider", json!("claude")),
        ("settings.default-skip-permissions", json!(false)),
        ("settings.language", json!("pt")),
    ] {
        sw.ok(id, json!({"value": val}));
    }
    let v = sw.ok("settings.get", json!({}));
    let st = &v["settings"];
    sw.check(st["default_model"] == "sonnet", format!("settings.default-model: {}", short(&v)));
    sw.check(st["default_permission_mode"] == "acceptEdits", "settings.default-permission-mode");
    sw.check(
        st["send_with_modifier"] == json!(true) || st.to_string().contains("modifier"),
        format!("settings.send-with-modifier: {}", short(st)),
    );
    sw.ok("prefs.set", json!({"key": "teste.chave", "value": "valor"}));
    let v = sw.ok("prefs.get", json!({"key": "teste.chave"}));
    sw.check(v == json!("valor"), format!("prefs.get = valor: {}", short(&v)));
    let v = sw.ok("prefs.list", json!({}));
    sw.check(v.to_string().contains("teste.chave"), "prefs.list");
    sw.ok("prefs.set", json!({"key": "teste.outra", "value": 1}));
    sw.ok("prefs.reset", json!({"prefix": "teste."}));
    let v = sw.ok("prefs.get", json!({"key": "teste.chave"}));
    sw.check(v.is_null(), format!("prefs.reset apagou: {}", short(&v)));
    sw.ok("prefs.get", json!({}));
    let v = sw.ok("settings.export", json!({}));
    sw.check(!v.is_null(), "settings.export");
    sw.ok("settings.reset", json!({}));
    let v = sw.ok("settings.get", json!({}));
    sw.check(
        v["settings"]["default_model"] == before["settings"]["default_model"],
        format!("settings.reset volta ao padrão: {}", short(&v)),
    );
    sw.ok("settings.language", json!({"value": "pt"}));
    let j = sw.ok("engine.journal", json!({"last": 5}));
    sw.check(!arr(&j).is_empty(), "engine.journal registrou mudanças");
}

pub struct Ids {
    pub ws: String,
    pub app: String,
    pub docs: String,
    pub direct: String,
    pub isolated: String,
}

fn workspace_and_projects(sw: &mut Sweep) -> Ids {
    let fx = sw.fx;
    let accts = sw.ok("account.list", json!({}));
    sw.check(accts.is_array(), "account.list");
    sw.ok("account.status", json!({}));
    let st = sw.ok("account.status", json!({"claudeDir": fx.account().to_string_lossy()}));
    sw.check(st["loggedIn"] == json!(true), format!("account.status da conta ativa: {}", short(&st)));
    let st = sw.ok("account.status", json!({"claudeDir": fx.home.join(".claude").to_string_lossy()}));
    sw.check(st["loggedIn"] == json!(false), format!("account.status de ~/.claude sem login: {}", short(&st)));
    sw.check(accts.to_string().contains(".claude-conta"), format!("account.list acha a conta: {}", short(&accts)));

    let w = sw.ok("workspace.create", json!({"path": fx.ws.to_string_lossy(), "name": "Demo"}));
    let wid = s(&w["id"]);
    sw.check(!wid.is_empty(), "workspace.create devolve id");
    // sem `claudeDir`, a conta ativa (a do $CLAUDE_CONFIG_DIR), como a tela sugere
    sw.check(w["claude_dir"] == json!(fx.account().to_string_lossy()), format!("workspace.create usa a conta ativa: {}", short(&w)));
    sw.err("workspace.create", json!({"path": fx.root.join("nao-existe").to_string_lossy()}), "pasta inexistente");
    let list = sw.ok("workspace.list", json!({}));
    sw.check(
        arr(&list["workspaces"]).iter().any(|w| s(&w["id"]) == wid) && list["active"] == json!(wid),
        format!("workspace.list: {}", short(&list)),
    );
    let g = sw.ok("workspace.get", json!({"id": wid}));
    sw.check(g["name"] == "Demo", format!("workspace.get: {}", short(&g)));
    sw.ok("workspace.activate", json!({"id": wid}));
    let a = sw.ok("workspace.active", json!({}));
    sw.check(a.to_string().contains(&wid), format!("workspace.active: {}", short(&a)));
    sw.ok("workspace.rename", json!({"id": wid, "name": "Demo 2"}));
    sw.ok("workspace.update", json!({"id": wid, "name": "Demo"}));
    let g = sw.ok("workspace.get", json!({"id": wid}));
    sw.check(g["name"] == "Demo", "workspace.update renomeou");
    sw.ok("workspace.set-account", json!({"id": wid}));
    let g = sw.ok("workspace.get", json!({"id": wid}));
    sw.check(g["claude_dir"].is_null(), format!("workspace.set-account sem conta = ~/.claude: {}", short(&g)));
    sw.ok("workspace.set-account", json!({"id": wid, "claudeDir": fx.account().to_string_lossy()}));
    let g = sw.ok("workspace.get", json!({"id": wid}));
    sw.check(g["claude_dir"] == json!(fx.account().to_string_lossy()), "workspace.set-account");
    sw.ok("workspace.ignore", json!({"id": wid, "path": "docs"}));
    let p = sw.ok("project.list", json!({"workspaceId": wid}));
    sw.check(!p.to_string().contains("\"docs\""), format!("workspace.ignore tira docs: {}", short(&p)));
    sw.ok("workspace.unignore", json!({"id": wid, "path": "docs"}));
    let p = sw.ok("project.list", json!({"workspaceId": wid}));
    sw.check(p.to_string().contains("\"docs\""), "workspace.unignore devolve docs");
    let abs = fx.ws.join("docs").to_string_lossy().to_string();
    sw.ok("workspace.ignore", json!({"id": wid, "path": abs}));
    let g = sw.ok("workspace.get", json!({"id": wid}));
    sw.check(arr(&g["ignored_paths"]).len() == 1, format!("relativo e absoluto são o mesmo: {}", short(&g)));
    sw.ok("workspace.unignore", json!({"id": wid, "path": abs}));
    let repos = sw.ok("workspace.repos", json!({"id": wid}));
    sw.check(repos.to_string().contains("app"), format!("workspace.repos: {}", short(&repos)));

    let projects = sw.ok("project.list", json!({"workspaceId": wid}));
    let find = |n: &str| arr(&projects).into_iter().find(|p| p["name"] == n).map(|p| s(&p["id"])).unwrap_or_default();
    let (app, docs) = (find("app"), find("docs"));
    sw.check(!app.is_empty() && !docs.is_empty(), format!("project.list tem app e docs: {}", short(&projects)));
    let pg = sw.ok("project.get", json!({"id": app}));
    sw.check(pg["name"] == "app", "project.get");
    sw.err("project.get", json!({"id": "nao-existe"}), "projeto inexistente");
    sw.ok("project.open-folder", json!({"id": docs}));
    sw.eventually("project.open-folder abriu a pasta", Duration::from_secs(5), |sw| sw.fx.opened().contains("docs"));

    // threads: uma direta (no projeto) e uma isolada (worktree)
    let d = sw.ok("thread.create", json!({"workspaceId": wid, "projectId": app, "isolated": false}));
    let direct = s(&d["id"]);
    let i = sw.ok(
        "thread.create",
        json!({"workspaceId": wid, "projectId": app, "isolated": true, "model": "sonnet", "permissionMode": "default"}),
    );
    let isolated = s(&i["id"]);
    sw.check(i["isolated"] == json!(true), "thread.create isolada");
    sw.check(Path::new(&s(&i["cwd"])).join("a.txt").is_file(), "checkout isolado criado");
    sw.err("thread.create", json!({"workspaceId": wid, "projectId": "nao", "isolated": false}), "projeto inexistente");
    let pt = sw.ok("project.threads", json!({"id": app}));
    sw.check(arr(&pt).len() == 2, format!("project.threads = 2: {}", short(&pt)));
    Ids { ws: wid, app, docs, direct, isolated }
}

fn wait_idle(sw: &mut Sweep, id: &str) -> Value {
    let v = sw.ok("thread.wait", json!({"id": id, "timeoutMs": 15000}));
    sw.check(v == json!("idle") || v["status"] == "idle", format!("thread.wait termina idle: {}", short(&v)));
    v
}

fn threads(sw: &mut Sweep, ids: &Ids) {
    let fx = sw.fx;
    let id = ids.direct.as_str();
    let l = sw.ok("thread.list", json!({"workspaceId": ids.ws}));
    sw.check(arr(&l).len() == 2, "thread.list = 2");
    let c = sw.ok("thread.count", json!({"workspaceId": ids.ws}));
    sw.check(c.to_string().contains('2'), format!("thread.count: {}", short(&c)));
    sw.ok("thread.count", json!({}));
    let g = sw.ok("thread.get", json!({"id": id}));
    sw.check(g["id"] == id, "thread.get");
    sw.err("thread.get", json!({"id": "nao"}), "thread inexistente");
    sw.ok("thread.status", json!({"id": id}));

    // conversa com o claude falso
    sw.ok("thread.send", json!({"id": id, "text": "olá agente"}));
    wait_idle(sw, id);
    let t = sw.ok("thread.transcript", json!({"id": id}));
    sw.check(t.to_string().contains("feito: olá agente"), format!("thread.transcript tem a resposta: {}", short(&t)));
    let lm = sw.ok("thread.last-message", json!({"id": id}));
    sw.check(lm.to_string().contains("feito: olá agente"), format!("thread.last-message: {}", short(&lm)));
    let md = sw.ok("thread.export", json!({"id": id}));
    sw.check(md.to_string().contains("olá agente"), "thread.export markdown");
    let js = sw.ok("thread.export", json!({"id": id, "format": "json"}));
    sw.check(js.to_string().contains("feito"), "thread.export json");
    let g = sw.ok("thread.get", json!({"id": id}));
    sw.check(g["session_id"] == "sess-1", format!("sessão guardada: {}", short(&g)));
    sw.check(g["title"] == "olá agente", format!("título da primeira mensagem: {}", short(&g)));

    // imagem anexada
    let img = sw.ok("system.read-image", json!({"path": fx.root.join("img.png").to_string_lossy()}));
    sw.check(img.to_string().contains("image/png"), format!("system.read-image: {}", short(&img)));
    sw.err("system.read-image", json!({"path": fx.root.join("nao.png").to_string_lossy()}), "arquivo inexistente");
    let data = img["data"].clone();
    sw.ok("thread.send", json!({"id": id, "text": "veja a imagem", "images": [{"mime": "image/png", "data": data}]}));
    wait_idle(sw, id);

    // pedido de permissão → resposta
    sw.ok("thread.send", json!({"id": id, "text": "PERMISSAO por favor"}));
    let w = sw.ok("thread.wait", json!({"id": id, "timeoutMs": 15000}));
    sw.check(w.to_string().contains("awaiting"), format!("thread.wait para no pedido de permissão: {}", short(&w)));
    let pp = sw.ok("thread.pending-permission", json!({"id": id}));
    sw.check(pp["toolName"] == "Write" && pp["toolUseId"] == "tu-1", format!("thread.pending-permission: {}", short(&pp)));
    let req = s(&pp["requestId"]);
    sw.ok(
        "thread.respond-permission",
        json!({"id": id, "requestId": req, "allow": true, "input": pp["input"], "toolUseId": pp["toolUseId"]}),
    );
    let pp = sw.ok("thread.pending-permission", json!({"id": id}));
    sw.check(pp.is_null(), "respondido, não há pedido pendente");
    wait_idle(sw, id);
    // só id + requestId + allow: o motor completa `input` e `toolUseId` com os do pedido
    sw.ok("thread.send", json!({"id": id, "text": "PERMISSAO resposta curta"}));
    let w = sw.ok("thread.wait", json!({"id": id, "timeoutMs": 15000}));
    sw.check(w.to_string().contains("awaiting"), format!("pedido de novo: {}", short(&w)));
    let pp = sw.ok("thread.pending-permission", json!({"id": id}));
    sw.ok("thread.respond-permission", json!({"id": id, "requestId": pp["requestId"], "allow": true}));
    let w = sw.ok("thread.wait", json!({"id": id, "timeoutMs": 15000}));
    sw.check(w == json!("idle"), format!("a resposta curta libera a ferramenta (sem perguntar de novo): {}", short(&w)));
    wait_idle(sw, id);
    let t = sw.ok("thread.transcript", json!({"id": id}));
    sw.check(t.to_string().contains("permissão: allow"), "o agente recebeu a permissão");
    // interromper um turno pendente
    sw.ok("thread.send", json!({"id": id, "text": "PERMISSAO de novo"}));
    sw.eventually("aguardando permissão de novo", Duration::from_secs(10), |sw| {
        sw.ok("thread.status", json!({"id": id})).to_string().contains("awaiting")
    });
    sw.ok("thread.interrupt", json!({"id": id}));
    sw.eventually("interromper encerra o turno", Duration::from_secs(10), |sw| {
        let st = sw.ok("thread.status", json!({"id": id})).to_string();
        !st.contains("awaiting") && !st.contains("running")
    });

    // opções da sessão
    sw.ok("thread.set-model", json!({"id": id, "model": "opus"}));
    sw.ok("thread.set-permission-mode", json!({"id": id, "mode": "plan"}));
    sw.ok("thread.set-options", json!({"id": id, "effort": "high", "addDirs": [fx.ws.join("docs").to_string_lossy()]}));
    sw.err("thread.set-options", json!({"id": id, "addDirs": [fx.root.join("nao-existe").to_string_lossy()]}), "pasta extra inexistente");
    let g = sw.ok("thread.get", json!({"id": id}));
    sw.check(g["model"] == "opus", format!("thread.set-model: {}", short(&g)));
    sw.check(g.to_string().contains("plan"), "thread.set-permission-mode");
    sw.ok("thread.send", json!({"id": id, "text": "depois das opções"}));
    wait_idle(sw, id);
    let m = sw.ok("thread.mcp", json!({"id": id, "action": "status"}));
    sw.check(m.to_string().contains("demo"), format!("thread.mcp status: {}", short(&m)));
    sw.ok("thread.mcp", json!({"id": id, "action": "toggle", "server": "demo", "enabled": false}));
    sw.ok("thread.mcp", json!({"id": id, "action": "reconnect", "server": "demo"}));

    // metadados
    sw.ok("thread.update", json!({"id": id, "title": "Conversa", "bookmarked": true, "unread": false}));
    sw.ok("thread.rename", json!({"id": id, "title": "Conversa principal"}));
    sw.ok("thread.bookmark", json!({"id": id}));
    let g = sw.ok("thread.get", json!({"id": id}));
    sw.check(g["title"] == "Conversa principal" && g["bookmarked"] == json!(true), format!("rename/bookmark: {}", short(&g)));
    sw.ok("thread.unbookmark", json!({"id": id}));
    sw.ok("thread.mark-unread", json!({"id": id}));
    let g = sw.ok("thread.get", json!({"id": id}));
    sw.check(g["unread"] == json!(true) && g["bookmarked"] == json!(false), "unbookmark/mark-unread");
    sw.ok("thread.mark-read", json!({"id": id}));
    let found = sw.ok("thread.search", json!({"query": "principal", "workspaceId": ids.ws}));
    sw.check(arr(&found).len() == 1, format!("thread.search: {}", short(&found)));
    sw.ok("thread.search", json!({"query": "", "archived": false}));

    // duplicar, arquivar, desarquivar, apagar
    let dup = sw.ok("thread.duplicate", json!({"id": id}));
    let dup_id = s(&dup["id"]);
    sw.check(!dup_id.is_empty() && dup_id != id, "thread.duplicate cria outra");
    sw.ok("thread.archive", json!({"id": dup_id, "archived": true}));
    let found = sw.ok("thread.search", json!({"query": "", "archived": true}));
    sw.check(found.to_string().contains(&dup_id), "arquivada aparece na busca de arquivadas");
    sw.ok("thread.unarchive", json!({"id": dup_id}));
    sw.ok("thread.delete", json!({"id": dup_id}));
    sw.err("thread.get", json!({"id": dup_id}), "apagada");

    // trocar de provedor (ACP falso) e voltar
    sw.err("thread.set-provider", json!({"id": id, "provider": "codex"}), "thread com conversa não troca de provedor");
    let n = sw.ok("thread.create", json!({"workspaceId": ids.ws, "projectId": ids.docs, "isolated": false}));
    let acp = s(&n["id"]);
    sw.ok("thread.set-provider", json!({"id": acp, "provider": "gemini"}));
    sw.ok("thread.set-provider", json!({"id": acp, "provider": "codex", "model": "m1"}));
    let g = sw.ok("thread.get", json!({"id": acp}));
    sw.check(g["provider"] == "codex" && g["model"] == "m1", format!("thread.set-provider: {}", short(&g)));
    sw.ok("thread.set-model", json!({"id": acp, "model": "m1"}));
    sw.ok("thread.send", json!({"id": acp, "text": "oi codex"}));
    wait_idle(sw, &acp);
    let t = sw.ok("thread.transcript", json!({"id": acp}));
    sw.check(t.to_string().contains("ok acp"), format!("resposta do agente ACP: {}", short(&t)));
    sw.ok("thread.export", json!({"id": acp}));
    sw.err("thread.set-provider", json!({"id": acp, "provider": "claude"}), "thread com conversa");
    sw.ok("thread.delete", json!({"id": acp}));

    // sessões do Claude Code importadas
    let n = sw.ok("thread.sync-sessions", json!({"workspaceId": ids.ws}));
    sw.check(n.is_number() || n.is_object(), format!("thread.sync-sessions: {}", short(&n)));
    sw.ok("system.session-file", json!({"id": id}));
    sw.ok("system.session-raw", json!({"id": id, "last": 5}));
}

fn review(sw: &mut Sweep, ids: &Ids) {
    let id = ids.isolated.as_str();
    let t = sw.ok("thread.get", json!({"id": id}));
    let cwd = PathBuf::from(s(&t["cwd"]));
    std::fs::write(cwd.join("a.txt"), "um\ndois\n").unwrap_or_default();
    std::fs::write(cwd.join("novo.txt"), "novo\n").unwrap_or_default();
    let ch = sw.ok("thread.changes", json!({"id": id, "base": "upstream"}));
    let files = arr(&ch["files"]);
    sw.check(files.len() == 2, format!("thread.changes = 2: {}", short(&ch)));
    sw.ok("thread.changes", json!({"id": id, "base": "head"}));
    sw.err("thread.changes", json!({"id": id, "base": "turn"}), "sem turno ainda");
    let fp = files.iter().find(|f| f["path"] == "a.txt").map(|f| f["fingerprint"].clone()).unwrap_or(Value::Null);
    sw.ok("thread.set-seen", json!({"id": id, "entries": [["a.txt", fp]]}));
    let ch = sw.ok("thread.changes", json!({"id": id, "base": "upstream"}));
    sw.check(arr(&ch["files"]).iter().any(|f| f["path"] == "a.txt" && f["seen"] == json!(true)), "thread.set-seen marcou");
    let st = sw.ok("thread.diff-stats", json!({"id": id}));
    sw.check(st["files"].as_u64() == Some(2), format!("thread.diff-stats: {}", short(&st)));
    let fd = sw.ok("thread.file-diff", json!({"id": id, "path": "a.txt"}));
    sw.check(fd.to_string().contains("dois"), format!("thread.file-diff: {}", short(&fd)));
    sw.ok("thread.file-diff", json!({"id": id, "path": "a.txt", "base": "head"}));
    sw.ok("thread.discard-file", json!({"id": id, "path": "novo.txt"}));
    sw.check(!cwd.join("novo.txt").exists(), "thread.discard-file apagou o novo");
    let bi = sw.ok("thread.branch-info", json!({"id": id}));
    sw.check(bi["dirty"] == json!(true), format!("thread.branch-info: {}", short(&bi)));
    sw.ok("thread.commit", json!({"id": id, "message": "feat: dois"}));
    let bi = sw.ok("thread.branch-info", json!({"id": id}));
    sw.check(bi["ahead"].as_u64() == Some(1) && bi["dirty"] == json!(false), format!("thread.commit: {}", short(&bi)));
    sw.ok("thread.push", json!({"id": id}));
    let branch = s(&bi["branch"]);
    let remote_has = std::process::Command::new("git")
        .args(["--git-dir", &sw.fx.remote.to_string_lossy(), "branch", "--list", &branch])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(&branch))
        .unwrap_or(false);
    sw.check(remote_has, format!("thread.push mandou {branch} ao remoto"));
    // a base anda: update-from-base traz o commit novo
    sw.fx.git(&sw.fx.repo, &["commit", "-q", "--allow-empty", "-m", "chore: base andou"]);
    sw.ok("thread.update-from-base", json!({"id": id}));
    let bi = sw.ok("thread.branch-info", json!({"id": id}));
    sw.check(bi["behind"].as_u64() == Some(0), format!("thread.update-from-base: {}", short(&bi)));
    sw.ok("thread.land", json!({"id": id, "mode": "merge"}));
    let main_a = std::fs::read_to_string(sw.fx.repo.join("a.txt")).unwrap_or_default();
    sw.check(main_a == "um\ndois\n", format!("thread.land merge trouxe a mudança: {main_a:?}"));
    // apply com uma mudança nova
    std::fs::write(cwd.join("b.txt"), "linha 1\n").unwrap_or_default();
    sw.ok("thread.land", json!({"id": id, "mode": "apply"}));
    let main_b = std::fs::read_to_string(sw.fx.repo.join("b.txt")).unwrap_or_default();
    sw.check(main_b == "linha 1\n", format!("thread.land apply: {main_b:?}"));
    sw.fx.git(&sw.fx.repo, &["checkout", "-q", "--", "b.txt"]);
}

fn terminals(sw: &mut Sweep, _ids: &Ids) {
    let cwd = sw.fx.ws.to_string_lossy().to_string();
    sw.ok("terminal.spawn", json!({"termId": "t1", "cwd": cwd, "cols": 80, "rows": 24}));
    let l = sw.ok("terminal.list", json!({}));
    sw.check(l.to_string().contains("t1"), format!("terminal.list: {}", short(&l)));
    let marker = sw.fx.root.join("term.txt");
    sw.ok("terminal.run", json!({"termId": "t1", "command": format!("echo ok > '{}'", marker.to_string_lossy())}));
    sw.eventually("terminal.run executou", Duration::from_secs(10), |_| marker.is_file());
    sw.ok("terminal.write", json!({"termId": "t1", "data": "echo a"}));
    sw.ok("terminal.tab", json!({"termId": "t1"}));
    sw.ok("terminal.enter", json!({"termId": "t1"}));
    sw.ok("terminal.interrupt", json!({"termId": "t1"}));
    sw.ok("terminal.clear", json!({"termId": "t1"}));
    sw.ok("terminal.resize", json!({"termId": "t1", "cols": 100, "rows": 30}));
    sw.ok("terminal.eof", json!({"termId": "t1"}));
    sw.ok("terminal.kill", json!({"termId": "t1"}));
    sw.err("terminal.write", json!({"termId": "nao", "data": "x"}), "terminal inexistente");
}

fn providers_and_system(sw: &mut Sweep, ids: &Ids) {
    let fx = sw.fx;
    let pl = sw.ok("provider.list", json!({}));
    sw.check(pl.to_string().contains("codex"), "provider.list");
    let st = sw.ok("provider.status", json!({"provider": "codex", "force": true}));
    sw.check(st["available"] == json!(true), format!("provider.status: {}", short(&st)));
    sw.ok("provider.status", json!({"provider": "gemini"}));
    let am = sw.ok("provider.auth-methods", json!({"provider": "codex"}));
    sw.check(am.to_string().contains("fake"), format!("provider.auth-methods: {}", short(&am)));
    sw.ok("provider.login", json!({"provider": "codex", "method": "fake"}));
    sw.ok("provider.forget", json!({}));

    // catálogos do agente (sondagem do claude falso)
    let c = sw.ok("capabilities.get", json!({"workspaceId": ids.ws, "force": true}));
    sw.check(c.to_string().contains("review"), format!("capabilities.get: {}", short(&c)));
    sw.ok("capabilities.get", json!({"workspaceId": ids.ws, "path": fx.repo.to_string_lossy()}));
    let sk = sw.ok("capabilities.skills", json!({"workspaceId": ids.ws}));
    sw.check(sk.to_string().contains("demo"), format!("capabilities.skills: {}", short(&sk)));
    sw.ok("capabilities.agents", json!({"workspaceId": ids.ws}));
    let mc = sw.ok("capabilities.mcp", json!({"workspaceId": ids.ws}));
    sw.check(mc.to_string().contains("demo"), format!("capabilities.mcp: {}", short(&mc)));
    let mo = sw.ok("capabilities.models", json!({"workspaceId": ids.ws}));
    sw.check(mo.to_string().contains("sonnet"), format!("capabilities.models: {}", short(&mo)));
    sw.ok("capabilities.refresh", json!({}));
    let skill = fx.home.join(".claude").join("skills").join("demo").join("SKILL.md");
    sw.err("capabilities.trash", json!({"workspaceId": ids.ws, "path": "/etc/passwd"}), "fora da conta");
    // a lixeira do sistema pode não existir no ambiente de teste: só não pode ser pânico
    let _ = sw.call("capabilities.trash", json!({"workspaceId": ids.ws, "path": skill.to_string_lossy()}));

    // sistema
    let h = sw.ok("system.health", json!({}));
    sw.check(h.is_object(), "system.health");
    let w = sw.ok("system.which", json!({"name": "git"}));
    sw.check(!w.is_null(), "system.which git");
    let d = sw.ok("system.data-dir", json!({}));
    sw.check(!s(&d).is_empty() || d.is_object(), "system.data-dir");
    let home = sw.ok("system.home", json!({}));
    sw.check(home.to_string().contains("home"), format!("system.home: {}", short(&home)));
    let up = sw.ok("system.user-profile", json!({}));
    sw.check(up.to_string().contains("demo@example.com"), format!("system.user-profile: {}", short(&up)));
    let target = fx.root.join("img.png").to_string_lossy().to_string();
    sw.ok("system.open-path", json!({"path": target}));
    sw.eventually("system.open-path abriu", Duration::from_secs(5), |sw| sw.fx.opened().contains("img.png"));
    sw.ok("system.open-url", json!({"url": "https://example.com/abc"}));
    sw.eventually("system.open-url abriu", Duration::from_secs(5), |sw| sw.fx.opened().contains("example.com/abc"));
    sw.err("system.open-url", json!({"url": "file:///etc/passwd"}), "só http(s)");
    let f = sw.ok("system.fonts", json!({}));
    sw.check(f.is_object(), format!("system.fonts: {}", short(&f)));
}

fn git_path(sw: &mut Sweep) {
    let p = sw.fx.repo.to_string_lossy().to_string();
    let v = sw.ok("git.path.is-repo", json!({"path": p}));
    sw.check(v == json!(true), "git.path.is-repo");
    let v = sw.ok("git.path.is-repo", json!({"path": sw.fx.ws.join("docs").to_string_lossy()}));
    sw.check(v == json!(false), "docs não é repo");
    let v = sw.ok("git.path.current-branch", json!({"path": p}));
    sw.check(v.to_string().contains("main"), format!("git.path.current-branch: {}", short(&v)));
    for id in [
        "git.path.status",
        "git.path.log",
        "git.path.diff",
        "git.path.diff-staged",
        "git.path.head",
        "git.path.root",
        "git.path.shortstat",
        "git.path.stash-list",
        "git.path.tags",
    ] {
        sw.ok(id, json!({"path": p}));
    }
    let b = sw.ok("git.path.branches", json!({"path": p}));
    sw.check(b.to_string().contains("feature"), format!("git.path.branches: {}", short(&b)));
    let r = sw.ok("git.path.remotes", json!({"path": p}));
    sw.check(r.to_string().contains("origin"), "git.path.remotes");
    std::fs::write(sw.fx.repo.join("p.txt"), "p\n").unwrap_or_default();
    let d = sw.ok("git.path.status", json!({"path": p}));
    sw.check(d.to_string().contains("p.txt"), format!("git.path.status vê p.txt: {}", short(&d)));
    sw.ok("git.path.commit-all", json!({"path": p, "message": "chore: p"}));
    let l = sw.ok("git.path.log", json!({"path": p}));
    sw.check(l.to_string().contains("chore: p"), "git.path.commit-all commitou");
}

fn head(sw: &mut Sweep, rid: &str) -> String {
    let h = sw.ok("git.log.head", json!({"repoId": rid}));
    let sha = s(&h["sha"]);
    if sha.is_empty() {
        s(&h["id"])
    } else {
        sha
    }
}

fn git_repo(sw: &mut Sweep) {
    let fx = sw.fx;
    let repo = fx.repo.clone();
    let o = sw.ok("git.repo.open", json!({"path": repo.to_string_lossy()}));
    let rid = s(&o["repoId"]);
    sw.check(!rid.is_empty(), format!("git.repo.open: {}", short(&o)));
    sw.err("git.repo.open", json!({"path": fx.ws.join("docs").to_string_lossy()}), "não é repositório");
    let r = || json!({"repoId": rid});
    let st = sw.ok("git.repo.status", r());
    sw.check(st.is_object(), "git.repo.status");
    sw.ok("git.repo.fingerprint", r());
    let ident = sw.ok("git.config.identity", json!({}));
    sw.check(ident.to_string().contains("demo@example.com"), format!("git.config.identity: {}", short(&ident)));
    sw.ok("git.config.editors", json!({}));
    let fp = sw.ok("git.config.repo-file-path", json!({"repoId": rid, "relPath": "a.txt"}));
    sw.check(fp.to_string().contains("a.txt"), "git.config.repo-file-path");
    sw.ok(
        "git.config.open-in-editor",
        json!({"command": fx.bin.join("fake-editor").to_string_lossy(), "path": repo.join("a.txt").to_string_lossy()}),
    );
    sw.eventually("git.config.open-in-editor abriu", Duration::from_secs(5), |sw| sw.fx.opened().contains("a.txt"));

    // working tree, staging, commit
    std::fs::write(repo.join("w.txt"), "w1\nw2\nw3\n").unwrap_or_default();
    let ws = sw.ok("git.working.status", r());
    sw.check(ws.to_string().contains("w.txt"), "git.working.status vê w.txt");
    sw.ok("git.working.stage-path", json!({"repoId": rid, "path": "w.txt"}));
    let d = sw.ok("git.working.diff", json!({"repoId": rid, "staged": true}));
    sw.check(d.to_string().contains("w1"), format!("git.working.diff staged: {}", short(&d)));
    sw.ok("git.working.unstage-path", json!({"repoId": rid, "path": "w.txt"}));
    sw.ok(
        "git.working.apply-lines",
        json!({"repoId": rid, "path": "w.txt", "action": "stage", "selection": {"oldLines": [], "newLines": [1, 2, 3]}}),
    );
    sw.ok("git.working.unstage-all", r());
    sw.ok("git.working.stage-all", r());
    sw.ok("git.working.diff", json!({"repoId": rid, "staged": false}));
    sw.ok("git.working.commit", json!({"repoId": rid, "summary": "feat: w", "body": "", "noVerify": false, "amend": false}));
    let h1 = head(sw, &rid);
    sw.check(!h1.is_empty(), "git.log.head");
    sw.ok("git.working.amend-message", json!({"repoId": rid, "sha": h1, "summary": "feat: w (emendado)", "body": "corpo"}));
    let h = sw.ok("git.log.head", r());
    sw.check(h.to_string().contains("emendado"), format!("git.working.amend-message: {}", short(&h)));
    let h1 = head(sw, &rid);
    sw.ok("git.working.ignore", json!({"repoId": rid, "pattern": "*.log", "local": true}));
    std::fs::write(repo.join("x.log"), "x").unwrap_or_default();
    let ws = sw.ok("git.working.status", r());
    sw.check(!ws.to_string().contains("x.log"), "git.working.ignore esconde *.log");
    std::fs::write(repo.join("a.txt"), "mudado\n").unwrap_or_default();
    sw.ok("git.working.discard-path", json!({"repoId": rid, "path": "a.txt"}));
    sw.check(std::fs::read_to_string(repo.join("a.txt")).unwrap_or_default() == "um\ndois\n", "git.working.discard-path");
    std::fs::write(repo.join("a.txt"), "mudado\n").unwrap_or_default();
    sw.ok("git.working.discard-all", r());
    sw.check(std::fs::read_to_string(repo.join("a.txt")).unwrap_or_default() == "um\ndois\n", "git.working.discard-all");

    // IA (claude falso no modo -p)
    let m = sw.ok("ai.commit-message", json!({"model": "", "diff": "diff --git a/x b/x\n+oi\n", "customInstructions": ""}));
    sw.check(m.to_string().contains("mensagem gerada"), format!("ai.commit-message: {}", short(&m)));
    let pr = sw.ok("ai.pr-description", json!({"model": "", "commits": ["feat: x"], "diff": "+x"}));
    sw.check(pr.to_string().contains("título do PR"), format!("ai.pr-description: {}", short(&pr)));
    let rc = sw.ok(
        "ai.resolve-conflict",
        json!({"model": "", "path": "a.txt", "ancestor": "a\n", "ours": "b\n", "theirs": "c\n", "customInstructions": ""}),
    );
    sw.check(rc.to_string().contains("resolvido"), format!("ai.resolve-conflict: {}", short(&rc)));
    std::fs::write(repo.join("k1.txt"), "k1\n").unwrap_or_default();
    std::fs::write(repo.join("k2.txt"), "k2\n").unwrap_or_default();
    sw.ok("git.working.stage-all", r());
    let plan = sw.ok("ai.compose-commits", json!({"repoId": rid, "model": "", "customInstructions": "", "answers": ""}));
    sw.check(!arr(&plan["commits"]).is_empty(), format!("ai.compose-commits: {}", short(&plan)));
    let commits: Vec<Value> = arr(&plan["commits"])
        .into_iter()
        .map(|c| json!({"summary": c["summary"], "description": c["description"], "changes": c["changes"]}))
        .collect();
    sw.ok("git.working.apply-composed", json!({"repoId": rid, "treeId": plan["treeId"], "commits": commits}));
    let h = sw.ok("git.log.head", r());
    sw.check(h.to_string().contains("tudo junto"), format!("git.working.apply-composed: {}", short(&h)));

    // log, busca, diff, histórico
    let log = sw.ok("git.log.list", json!({"repoId": rid, "query": {"skip": 0, "limit": 50}}));
    sw.check(arr(&log["commits"]).len() >= 4, format!("git.log.list: {}", short(&log)));
    sw.ok("git.log.signature", json!({"repoId": rid, "sha": h1}));
    let stats = sw.ok("git.log.stats", r());
    sw.check(stats.is_object(), "git.log.stats");
    let rf = sw.ok("git.reflog.list", json!({"repoId": rid, "refName": "HEAD"}));
    sw.check(!arr(&rf).is_empty() || rf.is_object(), "git.reflog.list");
    for mode in ["message", "author", "sha", "file", "diff"] {
        sw.ok("git.log.search", json!({"repoId": rid, "query": "w", "mode": mode}));
    }
    let found = sw.ok("git.log.search", json!({"repoId": rid, "query": "emendado", "mode": "message"}));
    sw.check(!arr(&found).is_empty(), format!("git.log.search acha: {}", short(&found)));
    let first = std::process::Command::new("git")
        .args(["rev-list", "--max-parents=0", "HEAD"])
        .current_dir(&repo)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let cd = sw.ok("git.diff.commit", json!({"repoId": rid, "sha": h1}));
    sw.check(cd.to_string().contains("w.txt"), "git.diff.commit");
    sw.ok("git.diff.range", json!({"repoId": rid, "fromSha": first, "toSha": h1}));
    // revisões além do hash completo: HEAD, curto, branch
    let byref = sw.ok("git.diff.commit", json!({"repoId": rid, "sha": "HEAD~3"}));
    sw.check(byref.is_object(), "git.diff.commit resolve HEAD~3");
    sw.ok("git.diff.commit", json!({"repoId": rid, "sha": &h1[..7.min(h1.len())]}));
    let e = sw.err("git.diff.commit", json!({"repoId": rid, "sha": "nao-existe"}), "commit inexistente");
    sw.check(e.is_some_and(|f| f.message.contains("commit não encontrado")), "erro nomeia o commit, não o repositório");
    let fc = sw.ok("git.diff.file-content", json!({"repoId": rid, "sha": first, "path": "a.txt"}));
    sw.check(fc == json!("dW0K"), format!("git.diff.file-content (base64 de \"um\\n\"): {}", short(&fc)));
    sw.ok("git.diff.file-content", json!({"repoId": rid, "path": "a.txt"}));
    let fh = sw.ok("git.history.file", json!({"repoId": rid, "query": {"path": "a.txt", "limit": 20}}));
    sw.check(fh.to_string().contains("first"), format!("git.history.file: {}", short(&fh)));
    sw.ok("git.history.commit-file-diff", json!({"repoId": rid, "sha": h1, "path": "w.txt"}));
    let bl = sw.ok("git.history.blame", json!({"repoId": rid, "path": "a.txt", "startLine": 1, "maxLines": 10}));
    sw.check(bl.to_string().contains("um"), format!("git.history.blame: {}", short(&bl)));
    sw.ok("git.history.compare", json!({"repoId": rid, "a": "main", "b": "feature", "limit": 10}));
    sw.ok("git.history.tree-diff", json!({"repoId": rid, "fromSha": first, "toSha": h1}));

    // branches e tags
    let bl = sw.ok("git.branch.list", r());
    sw.check(bl.to_string().contains("feature"), "git.branch.list");
    sw.ok("git.branch.create", json!({"repoId": rid, "name": "topico", "checkout": false}));
    sw.ok("git.branch.create", json!({"repoId": rid, "name": "outra", "startPoint": first, "checkout": true}));
    let cb = sw.ok("git.path.current-branch", json!({"path": repo.to_string_lossy()}));
    sw.check(cb.to_string().contains("outra"), "git.branch.create com checkout");
    sw.ok("git.branch.checkout", json!({"repoId": rid, "name": "main"}));
    sw.ok("git.branch.checkout-detached", json!({"repoId": rid, "sha": first}));
    sw.ok("git.branch.checkout", json!({"repoId": rid, "name": "main"}));
    sw.ok("git.branch.checkout-at", json!({"repoId": rid, "name": "nova-em", "targetSha": first}));
    sw.ok("git.branch.checkout", json!({"repoId": rid, "name": "main"}));
    sw.ok("git.branch.delete-local", json!({"repoId": rid, "name": "nova-em"}));
    sw.ok("git.branch.delete-local", json!({"repoId": rid, "name": "outra"}));
    let bl = sw.ok("git.branch.list", r());
    sw.check(!bl.to_string().contains("nova-em"), "git.branch.delete-local");
    sw.ok("git.tag.create", json!({"repoId": rid, "name": "v1", "sha": h1}));
    let tl = sw.ok("git.tag.list", r());
    sw.check(tl.to_string().contains("v1"), "git.tag.create");
    sw.ok("git.tag.push", json!({"repoId": rid, "remoteName": "origin", "name": "v1"}));
    let rt = sw.ok("git.tag.list-remote", json!({"repoId": rid, "remoteName": "origin"}));
    sw.check(rt.to_string().contains("v1"), format!("git.tag.push/list-remote: {}", short(&rt)));
    sw.ok("git.tag.delete-remote", json!({"repoId": rid, "remoteName": "origin", "name": "v1"}));
    sw.ok("git.tag.delete-local", json!({"repoId": rid, "name": "v1"}));
    let tl = sw.ok("git.tag.list", r());
    sw.check(!tl.to_string().contains("\"v1\""), "git.tag.delete-local");

    // remotos, fetch, push, pull
    sw.ok("git.remote.list", r());
    let u = sw.ok("git.remote.url", json!({"repoId": rid, "remoteName": "origin"}));
    sw.check(u.to_string().contains("remote.git"), "git.remote.url");
    sw.ok("git.remote.add", json!({"repoId": rid, "name": "espelho", "url": fx.remote.to_string_lossy()}));
    sw.ok("git.remote.rename", json!({"repoId": rid, "name": "espelho", "newName": "espelho2"}));
    sw.ok("git.remote.set-url", json!({"repoId": rid, "name": "espelho2", "url": fx.remote.to_string_lossy()}));
    sw.ok("git.remote.remove", json!({"repoId": rid, "name": "espelho2"}));
    sw.ok("git.remote.fetch", json!({"repoId": rid, "remoteName": "origin"}));
    sw.ok("git.remote.fetch", json!({"repoId": rid}));
    sw.ok("git.remote.push", json!({"repoId": rid, "remoteName": "origin", "branch": "main", "force": false}));
    let remote_main = sw.fx.git(&fx.remote, &["rev-parse", "main"]);
    let local_main = sw.fx.git(&repo, &["rev-parse", "main"]);
    sw.check(remote_main == local_main, "git.remote.push levou main");
    sw.ok("git.remote.push", json!({"repoId": rid, "remoteName": "origin", "branch": "topico", "force": false}));
    sw.ok("git.branch.delete-remote", json!({"repoId": rid, "remoteName": "origin", "name": "topico"}));
    sw.ok("git.remote.push", json!({"repoId": rid, "remoteName": "origin", "branch": "topico", "force": false}));
    sw.ok("git.branch.delete-both", json!({"repoId": rid, "remoteNames": ["origin"], "name": "topico"}));
    let rb = sw.fx.git(&fx.remote, &["branch", "--list", "topico"]);
    sw.check(rb.trim().is_empty(), "git.branch.delete-both apagou no remoto");
    let us = sw.ok("git.pull.upstream-status", r());
    sw.check(us.is_object() || us.is_null(), "git.pull.upstream-status");
    // um commit no remoto (por outro clone) para o pull trazer
    let other = fx.root.join("outro-clone");
    sw.fx.git(&fx.root, &["clone", "-q", &fx.remote.to_string_lossy(), &other.to_string_lossy()]);
    std::fs::write(other.join("remoto.txt"), "r\n").unwrap_or_default();
    sw.fx.git(&other, &["add", "."]);
    sw.fx.git(&other, &["commit", "-qm", "feat: remoto"]);
    sw.fx.git(&other, &["push", "-q", "origin", "main"]);
    sw.ok("git.pull.pull", json!({"repoId": rid, "strategy": "FfOnly"}));
    sw.check(repo.join("remoto.txt").is_file(), "git.pull.pull trouxe remoto.txt");
    sw.ok("git.pull.pull", r());
    let h = head(sw, &rid);
    sw.ok("git.branch.sync-to-remote", json!({"repoId": rid, "targetSha": h}));

    // merge, cherry-pick, revert, reset
    let m = sw.ok("git.branch.merge", json!({"repoId": rid, "name": "feature"}));
    sw.check(repo.join("c.txt").is_file(), format!("git.branch.merge trouxe c.txt: {}", short(&m)));
    let feature_sha = sw.fx.git(&repo, &["rev-parse", "feature"]).trim().to_string();
    sw.ok("git.branch.create", json!({"repoId": rid, "name": "cp", "startPoint": first, "checkout": true}));
    sw.ok("git.commit.cherry-pick", json!({"repoId": rid, "sha": feature_sha, "createCommit": true}));
    sw.check(repo.join("c.txt").is_file(), "git.commit.cherry-pick");
    let cp_head = head(sw, &rid);
    sw.ok("git.commit.revert", json!({"repoId": rid, "sha": cp_head}));
    sw.check(!repo.join("c.txt").exists(), "git.commit.revert");
    sw.ok("git.reset.to-commit", json!({"repoId": rid, "sha": cp_head, "hard": true}));
    sw.check(repo.join("c.txt").is_file(), "git.reset.to-commit");
    sw.ok("git.reset.to-ref", json!({"repoId": rid, "refName": "main", "hard": false}));
    sw.ok("git.working.discard-all", r());
    sw.ok("git.branch.checkout", json!({"repoId": rid, "name": "main"}));
    sw.ok("git.branch.delete-local", json!({"repoId": rid, "name": "cp"}));

    // undo/redo
    sw.ok("git.undo.status", r());
    sw.ok("git.branch.create", json!({"repoId": rid, "name": "desfazer", "checkout": false}));
    sw.ok("git.undo.undo", r());
    let bl = sw.ok("git.branch.list", r());
    sw.check(!bl.to_string().contains("desfazer"), format!("git.undo.undo desfez a branch: {}", short(&bl)));
    sw.ok("git.undo.redo", r());
    let bl = sw.ok("git.branch.list", r());
    sw.check(bl.to_string().contains("desfazer"), "git.undo.redo refez");

    // stash
    std::fs::write(repo.join("a.txt"), "guardado\n").unwrap_or_default();
    sw.ok("git.stash.save", json!({"repoId": rid, "message": "guardado", "includeUntracked": true}));
    let sl = sw.ok("git.stash.list", r());
    sw.check(sl.to_string().contains("guardado"), "git.stash.save/list");
    sw.ok("git.stash.apply", json!({"repoId": rid, "index": 0}));
    sw.ok("git.working.discard-all", r());
    sw.ok("git.stash.pop", json!({"repoId": rid, "index": 0}));
    sw.check(std::fs::read_to_string(repo.join("a.txt")).unwrap_or_default() == "guardado\n", "git.stash.pop");
    sw.ok("git.stash.save", json!({"repoId": rid, "includeUntracked": false}));
    sw.ok("git.stash.drop", json!({"repoId": rid, "index": 0}));
    let sl = sw.ok("git.stash.list", r());
    sw.check(arr(&sl).is_empty(), format!("git.stash.drop: {}", short(&sl)));

    // rebase (com conflito resolvido) e rebase interativo
    sw.ok("git.branch.create", json!({"repoId": rid, "name": "rb", "startPoint": first, "checkout": true}));
    std::fs::write(repo.join("a.txt"), "do rb\n").unwrap_or_default();
    sw.fx.git(&repo, &["commit", "-qam", "rb: a"]);
    let rs = sw.ok("git.rebase.start", json!({"repoId": rid, "ontoBranch": "main"}));
    let st = sw.ok("git.rebase.status", r());
    sw.check(st.to_string().contains("a.txt"), format!("git.rebase.start parou no conflito: {} / {}", short(&rs), short(&st)));
    sw.ok("git.rebase.resolve-conflict", json!({"repoId": rid, "path": "a.txt", "resolution": {"kind": "UseTheirs"}}));
    sw.ok("git.rebase.continue", r());
    let st = sw.ok("git.repo.status", r());
    sw.check(st["inProgressOp"].is_null(), format!("rebase concluído: {}", short(&st)));
    std::fs::write(repo.join("a.txt"), "outro rb\n").unwrap_or_default();
    sw.fx.git(&repo, &["commit", "-qam", "rb: a de novo"]);
    sw.fx.git(&repo, &["checkout", "-q", "-b", "rb2", "HEAD~1"]);
    std::fs::write(repo.join("a.txt"), "conflita\n").unwrap_or_default();
    sw.fx.git(&repo, &["commit", "-qam", "rb2: a"]);
    sw.ok("git.rebase.start", json!({"repoId": rid, "ontoBranch": "rb"}));
    sw.ok("git.rebase.abort", r());
    let st = sw.ok("git.repo.status", r());
    sw.check(st["inProgressOp"].is_null(), "git.rebase.abort");
    sw.ok("git.branch.checkout", json!({"repoId": rid, "name": "main"}));
    for i in 1..=2 {
        std::fs::write(repo.join(format!("ir{i}.txt")), "x\n").unwrap_or_default();
        sw.fx.git(&repo, &["add", "."]);
        sw.fx.git(&repo, &["commit", "-qm", &format!("ir: {i}")]);
    }
    let base = sw.fx.git(&repo, &["rev-parse", "HEAD~2"]).trim().to_string();
    let from = sw.fx.git(&repo, &["rev-parse", "HEAD~1"]).trim().to_string();
    let plan = sw.ok("git.irebase.plan", json!({"repoId": rid, "fromSha": from}));
    let items: Vec<Value> = arr(&plan["items"])
        .into_iter()
        .chain(arr(&plan["commits"]))
        .enumerate()
        .map(|(i, c)| json!({"sha": c["sha"], "action": if i == 0 { "pick" } else { "squash" }, "message": null}))
        .collect();
    sw.check(items.len() == 2, format!("git.irebase.plan tem 2 commits: {}", short(&plan)));
    sw.ok("git.irebase.start", json!({"repoId": rid, "upstream": base, "items": items}));
    let n = sw.fx.git(&repo, &["rev-list", "--count", &format!("{base}..HEAD")]);
    sw.check(n.trim() == "1", format!("git.irebase.start juntou os 2 commits: {n}"));

    // worktrees, submódulos, lfs
    let wt = fx.root.join("wt-extra");
    sw.ok("git.worktree.add", json!({"repoId": rid, "args": {"path": wt.to_string_lossy(), "branch": "wt-b", "newBranch": true}}));
    sw.check(wt.join("a.txt").is_file(), "git.worktree.add");
    let wl = sw.ok("git.worktree.list", r());
    sw.check(wl.to_string().contains("wt-b") || wl.to_string().contains("wt-extra"), format!("git.worktree.list: {}", short(&wl)));
    let name =
        arr(&wl).into_iter().find(|w| w.to_string().contains("wt-extra")).map(|w| s(&w["name"])).unwrap_or_else(|| "wt-extra".into());
    sw.ok("git.worktree.remove", json!({"repoId": rid, "name": name, "force": true}));
    sw.check(!wt.exists(), "git.worktree.remove");
    sw.ok("git.worktree.prune", r());
    sw.ok("git.submodule.list", r());
    sw.ok("git.submodule.count", r());
    sw.ok("git.submodule.update", json!({"repoId": rid, "paths": []}));
    sw.ok("git.lfs.status", r());
    // sem git-lfs instalado o pull falha com erro tratado; com ele, não há o que baixar
    let _ = sw.call("git.lfs.pull", r());

    // credenciais: nada pendente
    sw.ok("git.ssh.pending-passphrase", json!({}));
    sw.ok("git.https.pending-credentials", json!({}));
    sw.ok("git.ssh.submit-passphrase", json!({}));
    sw.ok("git.https.submit-credentials", json!({}));
    sw.ok("git.ssh.list-keys", json!({}));

    sw.ok("git.repo.close", r());
    sw.err("git.repo.status", r(), "repositório fechado");
}

fn forges_and_wsl(sw: &mut Sweep) {
    // sem rede: um endereço que recusa a conexão vira erro tratado, rápido
    let base = json!({"baseUrl": "http://127.0.0.1:9", "token": "x", "projectPath": "a/b"});
    let with = |extra: Value| {
        let mut b = base.clone();
        if let (Some(o), Some(e)) = (b.as_object_mut(), extra.as_object()) {
            o.extend(e.clone());
        }
        b
    };
    for id in ["github.pr.list", "github.assignees", "github.events", "gitlab.project", "gitlab.mr.list", "gitlab.members", "gitlab.events"]
    {
        sw.err(id, base.clone(), "sem servidor");
    }
    sw.err(
        "github.pr.create",
        with(json!({"sourceBranch": "a", "targetBranch": "main", "title": "t", "description": "d", "reviewers": []})),
        "sem servidor",
    );
    sw.err(
        "gitlab.mr.create",
        with(json!({"sourceBranch": "a", "targetBranch": "main", "title": "t", "description": "d", "reviewerIds": [], "removeSourceBranch": false, "squash": false})),
        "sem servidor",
    );
    // WSL só existe no Windows: fora dele, lista vazia ou erro tratado
    let _ = sw.call("wsl.distros", json!({}));
    let _ = sw.call("wsl.home", json!({"distro": "Ubuntu"}));
    let _ = sw.call("wsl.dirs", json!({"path": "/"}));
}

fn cleanup(sw: &mut Sweep, ids: &Ids) {
    let t = sw.ok("thread.get", json!({"id": ids.isolated}));
    let cwd = PathBuf::from(s(&t["cwd"]));
    sw.ok("thread.archive", json!({"id": ids.isolated, "archived": true}));
    sw.check(!cwd.exists(), "arquivar remove o checkout isolado");
    sw.ok("thread.archive", json!({"id": ids.isolated, "archived": false}));
    sw.check(cwd.join("a.txt").exists(), "desarquivar recria o checkout");
    sw.ok("thread.delete", json!({"id": ids.isolated}));
    sw.check(!cwd.exists(), "apagar remove o checkout");
    sw.ok("thread.delete", json!({"id": ids.direct}));
    let _ = &ids.docs;
    let _ = &ids.app;
    sw.ok("engine.journal-clear", json!({}));
    let j = sw.ok("engine.journal", json!({}));
    sw.check(arr(&j).len() <= 1, format!("engine.journal-clear: {}", short(&j)));
    sw.ok("workspace.remove", json!({"id": ids.ws}));
    let l = sw.ok("workspace.list", json!({}));
    sw.check(arr(&l).is_empty(), "workspace.remove");
}

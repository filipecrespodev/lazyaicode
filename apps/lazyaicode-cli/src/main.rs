//! `lazyaicode-cli`: o LazyAICode sem janela.
//!
//! ```sh
//! lazyaicode-cli commands --group thread          # o que dá para fazer
//! lazyaicode-cli run workspace.list               # executa um comando
//! lazyaicode-cli run thread.create workspaceId=… projectId=… isolated=true
//! lazyaicode-cli batch passos.json                # [{command, params}]
//! lazyaicode-cli mcp                              # servidor MCP no stdio
//! lazyaicode-cli serve --web 127.0.0.1:8080 --web-root dist/   # versão web
//! ```
//!
//! Sem `--connect`, o motor roda neste processo, com o mesmo `state.json` do
//! app. Com `--connect 127.0.0.1:7878 --token-file …`, os comandos vão para
//! um app aberto com `--control`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use lazyaicode_automation::{mcp, server, token, ws, Backend, ControlClient, EngineBackend};
use lazyaicode_engine::{Engine, EngineOpts};
use lazyaicode_protocol::CommandInfo;
use serde_json::{json, Value};

#[derive(Parser)]
#[command(name = "lazyaicode-cli", version, about = "LazyAICode sem janela: comandos, lotes, MCP e servidor web")]
struct Cli {
    /// Pasta de dados (padrão: a do app; ou LAZYAICODE_DATA_DIR).
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    /// Estado só em memória (nada é gravado).
    #[arg(long, global = true)]
    ephemeral: bool,
    /// Manda os comandos para um app aberto com `--control` neste endereço.
    #[arg(long, global = true)]
    connect: Option<SocketAddr>,
    /// Arquivo do token do canal de controle (com --connect / --bridge / serve).
    #[arg(long, global = true)]
    token_file: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Lista os comandos (id, grupo, parâmetros).
    Commands {
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        group: Option<String>,
        /// JSON completo em vez da tabela.
        #[arg(long)]
        json: bool,
        /// Tabela em Markdown (para a documentação).
        #[arg(long)]
        markdown: bool,
    },
    /// Descreve um comando.
    Describe { id: String },
    /// Executa um comando. Parâmetros: um objeto JSON, ou `chave=valor`
    /// (valor JSON quando for válido: `isolated=true`, `cols=80`).
    Run { command: String, params: Vec<String> },
    /// Executa um lote `[{command, params}]` de um arquivo (`-` = stdin).
    Batch {
        file: PathBuf,
        #[arg(long)]
        keep_going: bool,
    },
    /// Estado da sessão.
    State,
    /// Últimos comandos que mudaram algo.
    Journal,
    /// Servidor MCP no stdio.
    Mcp {
        /// Ponte para um app aberto com `--control` (senão, motor no processo).
        #[arg(long)]
        bridge: Option<SocketAddr>,
    },
    /// Servidor headless: canal de controle TCP e/ou versão web (WebSocket).
    Serve {
        /// Canal de controle TCP (loopback), ex.: 127.0.0.1:7878.
        #[arg(long)]
        control: Option<SocketAddr>,
        /// Versão web + WebSocket, ex.: 127.0.0.1:8080 (0.0.0.0 só atrás de um proxy com TLS).
        #[arg(long)]
        web: Option<SocketAddr>,
        /// Pasta do build web (`trunk build` em apps/lazyaicode-web → dist/).
        #[arg(long)]
        web_root: Option<PathBuf>,
        /// Usa este token em vez de gerar um (ou LAZYAICODE_TOKEN).
        #[arg(long, env = "LAZYAICODE_TOKEN")]
        token: Option<String>,
    },
}

fn engine(cli: &Cli) -> Engine {
    lazyaicode_engine::workspace::init_platform();
    Engine::new(EngineOpts { data_dir: cli.data_dir.clone(), ephemeral: cli.ephemeral })
}

async fn backend(cli: &Cli) -> Result<Arc<dyn Backend>, String> {
    match cli.connect {
        Some(addr) => {
            let tf = cli.token_file.clone().unwrap_or_else(default_token_file);
            let t = token::read(&tf).map_err(|e| format!("token {}: {e}", tf.display()))?;
            Ok(Arc::new(ControlClient::connect(addr, &t).await.map_err(|e| e.to_string())?))
        }
        None => Ok(EngineBackend::new(engine(cli))),
    }
}

fn default_token_file() -> PathBuf {
    lazyaicode_engine::store::data_dir().join("control.token")
}

/// `chave=valor` → objeto JSON; um único argumento JSON passa direto.
fn parse_params(args: &[String]) -> Result<Value, String> {
    if let [one] = args {
        if one.trim_start().starts_with('{') {
            return serde_json::from_str(one).map_err(|e| format!("JSON inválido: {e}"));
        }
    }
    let mut m = serde_json::Map::new();
    for a in args {
        let (k, v) = a.split_once('=').ok_or_else(|| format!("esperado chave=valor: {a}"))?;
        let v = serde_json::from_str(v).unwrap_or_else(|_| Value::String(v.to_string()));
        m.insert(k.to_string(), v);
    }
    Ok(if m.is_empty() { Value::Null } else { Value::Object(m) })
}

fn print(v: &Value) {
    match v {
        Value::String(s) => println!("{s}"),
        Value::Null => {}
        other => println!("{}", serde_json::to_string_pretty(other).unwrap_or_default()),
    }
}

fn markdown_table(list: &[CommandInfo]) -> String {
    let mut out = String::from("| Comando | Grupo | Parâmetros | Atalho |\n|---|---|---|---|\n");
    for c in list {
        let params: Vec<String> = c.params.iter().map(|p| format!("`{}`{}", p.name, if p.required { "" } else { "?" })).collect();
        out.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            c.id,
            c.group,
            params.join(", "),
            c.shortcut.as_deref().map(|s| format!("`{s}`")).unwrap_or_default()
        ));
    }
    out
}

async fn run(cli: Cli) -> Result<(), String> {
    match &cli.cmd {
        Cmd::Commands { filter, group, json, markdown } => {
            let b = backend(&cli).await?;
            let v = b.call("engine.commands".into(), json!({ "filter": filter, "group": group })).await.map_err(|e| e.message)?;
            let list: Vec<CommandInfo> = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
            if *json {
                print(&v);
            } else if *markdown {
                print!("{}", markdown_table(&list));
            } else {
                for c in &list {
                    let params: Vec<String> = c.params.iter().map(|p| format!("{}{}", p.name, if p.required { "" } else { "?" })).collect();
                    println!("{:<42} {:<12} {}", c.id, c.group, params.join(" "));
                }
                eprintln!("{} comandos", list.len());
            }
        }
        Cmd::Describe { id } => {
            print(&backend(&cli).await?.call("engine.describe".into(), json!({ "id": id })).await.map_err(|e| e.message)?)
        }
        Cmd::Run { command, params } => {
            let p = parse_params(params)?;
            let b = backend(&cli).await?;
            match b.call(command.clone(), p).await {
                Ok(v) => print(&v),
                Err(e) => return Err(format!("{} ({})", e.message, e.code)),
            }
        }
        Cmd::Batch { file, keep_going } => {
            let text = if file.as_os_str() == "-" {
                let mut s = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut s).map_err(|e| e.to_string())?;
                s
            } else {
                std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?
            };
            let steps: Value = serde_json::from_str(&text).map_err(|e| format!("JSON inválido: {e}"))?;
            let b = backend(&cli).await?;
            let out = b.call("engine.batch".into(), json!({ "steps": steps, "stopOnError": !keep_going })).await.map_err(|e| e.message)?;
            print(&out);
            if out.as_array().is_some_and(|a| a.iter().any(|s| s["ok"] == false)) {
                return Err("um passo falhou".into());
            }
        }
        Cmd::State => print(&backend(&cli).await?.call("engine.state".into(), Value::Null).await.map_err(|e| e.message)?),
        Cmd::Journal => print(&backend(&cli).await?.call("engine.journal".into(), Value::Null).await.map_err(|e| e.message)?),
        Cmd::Mcp { bridge } => {
            let b: Arc<dyn Backend> = match bridge {
                Some(addr) => {
                    let tf = cli.token_file.clone().unwrap_or_else(default_token_file);
                    let t = token::read(&tf).map_err(|e| format!("token {}: {e}", tf.display()))?;
                    Arc::new(ControlClient::connect(*addr, &t).await.map_err(|e| e.to_string())?)
                }
                None => EngineBackend::new(engine(&cli)),
            };
            mcp::serve_stdio(b, bridge.is_some()).await.map_err(|e| e.to_string())?;
        }
        Cmd::Serve { control, web, web_root, token: given } => {
            if control.is_none() && web.is_none() {
                return Err("use --control e/ou --web".into());
            }
            let e = engine(&cli);
            let t = given.clone().unwrap_or_else(token::generate);
            let tf = cli.token_file.clone().unwrap_or_else(|| e.data_dir().join("control.token"));
            token::write(&tf, &t).map_err(|e| format!("token {}: {e}", tf.display()))?;
            eprintln!("token em {}", tf.display());
            let b: Arc<dyn Backend> = EngineBackend::new(e.clone());
            let mut tasks = Vec::new();
            if let Some(addr) = control {
                eprintln!("canal de controle em {addr}");
                tasks.push(tokio::spawn(server::serve_tcp(*addr, b.clone(), t.clone())));
            }
            if let Some(addr) = web {
                if !addr.ip().is_loopback() {
                    eprintln!("aviso: {addr} não é loopback; exponha só atrás de um proxy com TLS");
                }
                let shown = if addr.ip().is_unspecified() { format!("127.0.0.1:{}", addr.port()) } else { addr.to_string() };
                eprintln!("versão web em http://{shown}/#token={t}");
                let opts = ws::WebOpts { addr: *addr, token: t.clone(), static_dir: web_root.clone() };
                tasks.push(tokio::spawn(ws::serve(opts, b.clone())));
            }
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                r = futures_first(tasks) => { if let Err(err) = r { return Err(err); } }
            }
            e.shutdown().await;
        }
    }
    Ok(())
}

async fn futures_first(tasks: Vec<tokio::task::JoinHandle<std::io::Result<()>>>) -> Result<(), String> {
    for t in tasks {
        match t.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e.to_string()),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

fn main() {
    if lazyaicode_git::answer_askpass() {
        return;
    }
    let cli = Cli::parse();
    let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("erro: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = rt.block_on(run(cli)) {
        eprintln!("erro: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_accept_json_or_key_value() {
        assert_eq!(parse_params(&["{\"a\":1}".into()]).ok(), Some(json!({"a": 1})));
        assert_eq!(
            parse_params(&["id=abc".into(), "isolated=true".into(), "cols=80".into()]).ok(),
            Some(json!({"id": "abc", "isolated": true, "cols": 80}))
        );
        assert_eq!(parse_params(&[]).ok(), Some(Value::Null));
        assert!(parse_params(&["semigual".into()]).is_err());
    }
}

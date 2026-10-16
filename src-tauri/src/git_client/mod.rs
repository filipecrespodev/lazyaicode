//! Cliente git: grafo de commits, staging, branches, rebase,
//! stash, remotos e integrações GitHub/GitLab, sobre libgit2.
//! Os comandos são registrados no `generate_handler!` de `lib.rs`.
//!
//! O núcleo sem Tauri também é compilado no `lazyaicode-agent` (o backend
//! que roda dentro do WSL): a lista dele fica em `agent_core.rs` e precisa
//! acompanhar os módulos daqui. O resto (comandos, IA, GitHub/GitLab,
//! editores) é só do app.

pub mod activity;
pub mod agent_server;
pub mod ai;
pub mod commands;
pub mod editors;
pub mod error;
pub mod events;
pub mod git;
pub mod github;
pub mod gitlab;
pub mod host;
pub mod https_credentials;
pub mod identity;
pub mod passphrase;
pub mod proto;
pub mod remote_agent;
pub mod settings;
pub mod ssh_keys;
pub mod state;
pub mod wsl;
pub mod wslpath;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;

/// Store do frontend onde ficam as configurações lidas pelo backend
/// (`sshKeys`, `signCommits`).
const STORE_FILE: &str = "layout.json";

/// Verdadeiro quando o processo foi iniciado como helper `SSH_ASKPASS` do
/// ssh-keygen para assinar commits (a resposta já foi impressa; deve sair).
pub fn answer_askpass() -> bool {
    git::signing::answer_askpass()
}

/// Inicializa o que precisa do AppHandle e registra o estado do cliente git.
pub fn setup(app: &AppHandle) {
    let handle = app.clone();
    events::set_sink(move |name, payload| {
        let _ = handle.emit(name, payload);
    });
    let handle = app.clone();
    settings::set_provider(move |key| handle.store(STORE_FILE).ok()?.get(key));
    // Instalador do Windows: o agente Linux vai junto (tauri.windows.conf.json)
    // e é copiado para a distro WSL no primeiro uso.
    #[cfg(windows)]
    if let Ok(dir) = app.path().resource_dir() {
        wsl::set_bundled_agent(dir.join("resources").join("lazyaicode-agent"));
    }
    app.manage(state::AppState::default());
}

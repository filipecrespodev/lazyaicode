//! Cliente git do LazyAICode: grafo de commits, staging, branches, rebase,
//! stash, remotos e integrações GitHub/GitLab, sobre libgit2 (`git`), mais os
//! checkouts isolados das threads, feitos com o `git` da máquina (`thread`).
//!
//! Sem interface: o motor (`lazyaicode-engine`) registra cada operação como um
//! comando, e o `lazyaicode-agent` (o backend que roda dentro de uma distro
//! WSL) usa este mesmo crate atrás do protocolo de `proto`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod activity;
pub mod agent_server;
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
pub mod thread;
pub mod wsl;

pub use lazyaicode_platform::wslpath;

/// Verdadeiro quando o processo foi iniciado como helper `SSH_ASKPASS` do
/// ssh-keygen para assinar commits (a resposta já foi impressa; deve sair).
pub fn answer_askpass() -> bool {
    git::signing::answer_askpass()
}

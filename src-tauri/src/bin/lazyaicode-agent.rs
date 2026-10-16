//! The git backend that runs inside a WSL distro for the Windows app.
//! `lazyaicode-agent serve` speaks `git_client::proto` on stdio.
//!
//! Built without Tauri (`cargo build --no-default-features --bin
//! lazyaicode-agent`): it compiles the Tauri-free core of `src/git_client`
//! straight from its sources (see `git_client/agent_core.rs`).

// Only the agent side of the core is used here.
#![allow(dead_code)]

#[path = "../git_client/agent_core.rs"]
mod git_client;

fn main() {
    // ssh-keygen's askpass helper when signing commits inside the distro.
    if git_client::git::signing::answer_askpass() {
        return;
    }
    match std::env::args().nth(1).as_deref() {
        Some("--version") => println!("{}", git_client::wsl::agent_version()),
        Some("serve") => git_client::agent_server::serve_stdio(),
        _ => {
            eprintln!("usage: lazyaicode-agent serve | --version");
            std::process::exit(2);
        }
    }
}

//! The Tauri-free core of the git client, as compiled into `lazyaicode-agent`
//! (`src/bin/lazyaicode-agent.rs` loads this file as its `git_client`
//! module, so `crate::git_client::...` paths resolve the same as in the app).
//! Keep in sync with the core part of `mod.rs`; the app adds the commands,
//! AI, GitHub/GitLab and editors on top.

pub mod activity;
pub mod agent_server;
pub mod error;
pub mod events;
pub mod git;
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

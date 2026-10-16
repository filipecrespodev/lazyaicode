//! Automação do LazyAICode: tudo o que a UI faz, um script ou um agente de
//! IA também faz, pelo mesmo registro de comandos do motor.
//!
//! - [`server`]: o canal de controle — JSON delimitado por linha sobre TCP
//!   em `127.0.0.1`, autenticado por token. O app desktop abre com
//!   `--control <porta>`; a CLI com `lazyaicode-cli serve --control`.
//! - [`ws`]: o mesmo protocolo sobre WebSocket, mais os arquivos estáticos
//!   da versão web (`lazyaicode-cli serve --web <dist>`).
//! - [`mcp`]: servidor MCP no stdio, com o motor no próprio processo
//!   (headless) ou ligado a um app aberto (ponte, [`client`]).
//! - [`backend`]: quem atende os pedidos (o motor, ou o motor + a UI).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod backend;
pub mod client;
pub mod mcp;
pub mod server;
pub mod token;
pub mod ws;

pub use backend::{Backend, BoxFut, EngineBackend};
pub use client::ControlClient;

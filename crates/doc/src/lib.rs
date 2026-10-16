//! Modelo de documento do LazyAICode: só dados, sem IO nem processos.
//!
//! - [`model`]: o que fica no `state.json` (workspaces, projetos, threads,
//!   preferências) e as tabelas fixas (provedores, modelos, modos).
//! - [`transcript`]: a conversa de uma thread, montada a partir do stream do
//!   agente ou do histórico da sessão.
//! - [`tree`]: a árvore da barra lateral.
//! - [`diff`]: patches unificados com numeração de linhas.
//! - [`usage`]: uso da janela de contexto.
//!
//! Compila para `wasm32-unknown-unknown`: a versão web desenha a partir do
//! mesmo modelo que o app desktop.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing))]

pub mod diff;
pub mod graph;
pub mod model;
pub mod transcript;
pub mod tree;
pub mod usage;

pub use model::*;

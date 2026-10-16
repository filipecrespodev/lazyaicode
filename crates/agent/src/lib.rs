//! Os agentes do LazyAICode. Não há chave própria: o agente é o `claude`
//! instalado na máquina (stream-json em `agent`), ou Codex e Gemini CLI
//! falando ACP (`acp`, traduzido para o mesmo formato em `acp_translate`).
//! Também ficam aqui as sessões do Claude Code (`sessions`, `transcript`),
//! o catálogo de skills, agentes e MCP (`catalog`) e os terminais (`terminal`).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod acp;
pub mod acp_translate;
pub mod agent;
pub mod catalog;
pub mod claude;
pub mod lines;
pub mod sessions;
pub mod terminal;
pub mod transcript;

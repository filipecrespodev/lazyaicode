//! O contrato que liga as camadas do LazyAICode sem que uma conheça a outra:
//!
//! - [`CommandInfo`]: o que o motor sabe fazer (id, rótulo, menu, atalho,
//!   parâmetros). A UI monta menus, paleta e atalhos a partir disso; o MCP
//!   e a CLI listam e validam com a mesma tabela.
//! - [`Request`] / [`Reply`] / [`Event`]: o canal de controle, JSON delimitado
//!   por linha. É o mesmo protocolo no TCP do app desktop (`--control`), no
//!   WebSocket da versão web (`lazyaicode-cli serve`) e na ponte do MCP.
//!
//! Sem dependências de plataforma: compila para `wasm32-unknown-unknown`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing))]

pub mod events;
pub mod frame;
pub mod params;

pub use frame::{decode_line, encode_line, ErrorBody, Event, Frame, FrameError, Reply, Request};
pub use params::{check_params, ParamInfo, ParamType};

use serde::{Deserialize, Serialize};

/// Versão do protocolo; o cliente compara no `hello` e recusa uma diferente.
pub const PROTOCOL_VERSION: u32 = 1;

/// Maior linha aceita num pedido (1 MiB).
pub const MAX_REQUEST_LINE: usize = 1 << 20;
/// Maior resposta enviada (8 MiB); acima disso o servidor responde erro.
pub const MAX_REPLY_BYTES: usize = 8 << 20;
/// Passos de um `engine.batch`.
pub const MAX_BATCH_STEPS: usize = 256;
/// Conexões simultâneas no canal de controle.
pub const MAX_CONNECTIONS: usize = 16;
/// Profundidade máxima de um JSON recebido (pedidos e linhas de agentes).
pub const MAX_JSON_DEPTH: usize = 128;

/// Uma ação que o motor (ou a casca da UI) sabe executar. Tudo o que o
/// usuário faz é um comando: a UI, a CLI, o canal de controle e o MCP
/// chamam pelo `id`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandInfo {
    /// `area.acao` em minúsculas (`thread.send`, `git.branch.create`).
    pub id: String,
    /// Chave de tradução (`lazyaicode-i18n`) ou texto pronto.
    pub label: String,
    /// Agrupamento na paleta e no `commands --group`.
    pub group: String,
    /// Posição nos menus (`["Thread", "Arquivar"]`); vazio = fora dos menus.
    #[serde(default)]
    pub menu: Vec<String>,
    /// Atalho padrão em notação egui (`Ctrl+Shift+N`; `Cmd` vira ⌘ no macOS).
    #[serde(default)]
    pub shortcut: Option<String>,
    #[serde(default)]
    pub params: Vec<ParamInfo>,
    /// O que o comando faz, para agentes e para a ajuda da paleta.
    #[serde(default)]
    pub doc: String,
    /// Descrição curta do resultado, para agentes.
    #[serde(default)]
    pub returns: String,
    /// Só lê estado (pode ser repetido sem efeito; o MCP marca como seguro).
    #[serde(default)]
    pub read_only: bool,
    /// Apaga ou reescreve algo de forma difícil de desfazer (o MCP avisa).
    #[serde(default)]
    pub destructive: bool,
    /// `engine` (motor) ou `ui` (estado de tela da casca: abas, zoom, painéis).
    #[serde(default = "engine_scope")]
    pub scope: String,
}

fn engine_scope() -> String {
    "engine".into()
}

impl CommandInfo {
    /// Linha de busca da paleta e do `--filter`: id, rótulo e grupo.
    pub fn matches(&self, needle: &str, label: &str) -> bool {
        let n = needle.to_lowercase();
        n.is_empty() || self.id.to_lowercase().contains(&n) || label.to_lowercase().contains(&n) || self.group.to_lowercase().contains(&n)
    }
}

/// Um id de comando bem formado: `a.b[.c…]`, minúsculas, dígitos e `_`/`-`.
pub fn valid_command_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    parts.len() >= 2
        && id.len() <= 96
        && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_ids_are_dotted_lowercase() {
        assert!(valid_command_id("thread.send"));
        assert!(valid_command_id("git.branch.create"));
        assert!(valid_command_id("ui.tab-1"));
        assert!(!valid_command_id("thread"));
        assert!(!valid_command_id("Thread.Send"));
        assert!(!valid_command_id("a..b"));
        assert!(!valid_command_id("a.b c"));
    }
}

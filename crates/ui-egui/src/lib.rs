//! A interface do LazyAICode em egui: uma camada fina por cima do motor.
//!
//! A UI não conhece o motor: fala com ele por um [`Client`] (o motor no
//! mesmo processo no app desktop, ou o canal de controle por WebSocket na
//! versão web) usando os mesmos ids de comando da CLI e do MCP. O estado de
//! tela ([`state::UiState`]) é serde, e o canal de controle o lê e muda com
//! `ui.inspect` e `ui.run`.
//!
//! Nada aqui depende de janela: [`LazyApp::ui`] desenha dentro de qualquer
//! `egui::Ui` — a janela do eframe, o navegador ou um harness de teste.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing))]

pub mod app;
pub mod client;
pub mod commands;
pub mod control;
pub mod data;
pub mod host;
pub mod state;
pub mod terminal;
pub mod theme;
pub mod views;
pub mod windows;

pub use app::LazyApp;
pub use client::{Client, Incoming, MockClient, Ticket, WsClient};
pub use host::{Host, Inbox, NoHost, UiMsg};
pub use windows::{NewWindow, WindowFactory};

/// Carregadores de imagem do egui (anexos, avatares). Chamar uma vez com o
/// `Context`.
pub fn install(ctx: &egui::Context) {
    egui_extras::install_image_loaders(ctx);
}

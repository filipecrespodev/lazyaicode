//! LazyAICode no navegador. O motor não roda aqui (o navegador não abre
//! processos nem lê o disco): a UI fala com um `lazyaicode-cli serve --web`
//! pelo mesmo canal de controle do desktop, em WebSocket.
//!
//! O endereço do WebSocket é o da própria página (`/ws`); o token vem do
//! `#token=…` que o `serve` imprime, ou é pedido na primeira tela e guardado
//! no `localStorage`.
//!
//! ```sh
//! cd apps/lazyaicode-web && trunk build --release
//! lazyaicode-cli serve --web 127.0.0.1:8080 --web-root apps/lazyaicode-web/dist
//! ```

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("lazyaicode-web só roda no navegador: `trunk build` em apps/lazyaicode-web (ou `cargo xtask web`).");
}

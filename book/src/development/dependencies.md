# Dependências

Principais, por papel:

| Papel | Crates |
|---|---|
| UI e janela | `egui` 0.36, `eframe` (winit + wgpu), `egui_extras`, `egui_commonmark` (markdown), `egui-phosphor` |
| GPU | `wgpu` (Metal, Vulkan, DX12, GL, WebGPU/WebGL) |
| Compositor em CPU | `tiny-skia` (pixmap + PNG), `rayon` |
| Assíncrono | `tokio` |
| Git | `git2` (libgit2 e OpenSSL vendored) e o `git` da máquina |
| Terminal | `portable-pty`, `vt100` |
| Rede | `reqwest` (rustls), `tokio-tungstenite`, `ewebsock` (cliente da UI, nativo e web) |
| Diálogos e área de transferência | `rfd`, `arboard` |
| Web | `wasm-bindgen`, `web-sys`, `trunk` (build) |
| Testes | `proptest`, `tempfile`, `egui_kittest`, `libfuzzer-sys` |

Por que egui 0.36 e não 0.37: na data da reescrita, o 0.37 tinha um dia e o
ecossistema (markdown, ícones, kittest) ainda estava no 0.36.

Ao adicionar uma dependência: prefira as já usadas; nada com `unsafe` novo
no nosso código; licença compatível com MIT; e confira se compila para wasm
quando for de um crate da camada da UI.

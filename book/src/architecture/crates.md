# Crates e camadas

Um crate só depende de crates de camadas **abaixo** da dele. O
`cargo xtask layers` lê os `Cargo.toml` e falha em qualquer violação, em
dependência de egui/eframe/winit/wgpu/rfd abaixo da camada 4, ou se a UI
depender do motor, da automação, do git ou dos agentes.

| Camada | Crate | Papel |
|---|---|---|
| L0 | `lazyaicode-platform` | diferenças entre Linux, macOS, Windows, FreeBSD e WSL; caminhos UNC do WSL |
| L0 | `lazyaicode-protocol` | o contrato: `CommandInfo`, `ParamInfo`, quadros do canal de controle, limites, eventos (wasm) |
| L0 | `lazyaicode-i18n` | textos em en/pt/es/zh (wasm) |
| L0 | `lazyaicode-term` | emulador VT sobre `vt100`, teclas → sequências xterm (wasm) |
| L0 | `lazyaicode-testkit` | repositórios descartáveis e um `claude` falso para testes |
| L1 | `lazyaicode-doc` | modelo de dados, transcrição, árvore, diff, grafo, uso de contexto (wasm) |
| L1 | `lazyaicode-git` | cliente git sobre libgit2 (um ator por repositório), checkouts isolados, WSL, GitHub/GitLab |
| L1 | `lazyaicode-agent` | Claude Code (stream-json), Codex/Gemini (ACP), sessões, catálogo de skills/agentes/MCP, PTY |
| L2 | `lazyaicode-engine` | a sessão e o registro de comandos; IA de commit |
| L3 | `lazyaicode-automation` | canal de controle (TCP, WebSocket), MCP, cliente da ponte |
| L4 | `lazyaicode-ui-egui` | a interface (wasm) |
| L4 | `lazyaicode-render-cpu` | o compositor de referência em CPU |
| apps | `lazyaicode`, `lazyaicode-cli`, `lazyaicode-web`, `lazyaicode-wsl-agent` | executáveis |

Os crates marcados **wasm** são conferidos com
`cargo check --target wasm32-unknown-unknown` (`cargo xtask wasm`): é o que
a versão web carrega.

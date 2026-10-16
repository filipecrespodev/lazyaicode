# AGENTS.md — guia para agentes e contribuidores

O LazyAICode é um workspace desktop para agentes de código (Claude Code,
Codex, Gemini CLI), escrito **só em Rust**: motor de comandos, UI egui sobre
wgpu, CLI, canal de controle JSON e servidor MCP. **Sem Tauri, Electron,
webview ou JS/TS escrito à mão**: o app desktop é eframe nativo, e a versão
web é o mesmo Rust compilado para WebAssembly (trunk + wasm-bindgen). Nunca
adicione Tauri (ou qualquer UI em webview/JS) como dependência, build ou
alvo de empacotamento. O nome do produto é **LazyAICode**; nomes de máquina
ficam em minúsculas (`lazyaicode-*`, `lazyaicode-cli`, `ai.lazyaicode.LazyAICode`).

Leia este arquivo primeiro; depois o [book](book/src/README.md).

## 1. Orientação (5 minutos)

| Leia | Por quê |
|---|---|
| [`book/src/architecture/overview.md`](book/src/architecture/overview.md) | o mapa: motor, UI fina, automação, o caminho de um comando |
| [`book/src/architecture/crates.md`](book/src/architecture/crates.md) | as camadas e o que cada crate faz |
| [`book/src/architecture/command-engine.md`](book/src/architecture/command-engine.md) | o registro de comandos, contexto, lotes, journal |
| [`book/src/automation/`](book/src/automation/overview.md) | CLI, canal de controle, MCP — como dirigir e capturar o app |
| [`book/src/security/`](book/src/security/overview.md) | **antes** de mexer em parser, processo filho, rede, automação |
| [`book/src/development/`](book/src/development/testing.md) | testes, capturas, traduções, empacotamento |
| [`ROADMAP.md`](ROADMAP.md) | onde estamos e o que vem a seguir |

## 2. Mapa do workspace

```text
crates/
  platform protocol i18n term testkit   L0  base (sem dependências internas); protocol/i18n/term são wasm
  doc git agent                         L1  modelo só de dados (wasm) · cliente git · processos dos agentes
  engine                                L2  a sessão + o registro de comandos (tudo é comando)
  automation                            L3  canal de controle TCP/WebSocket, MCP, ponte
  ui-egui render-cpu                    L4  a UI (wasm; não liga o motor) · o compositor de referência em CPU
apps/
  lazyaicode                            desktop (eframe + wgpu), --control, --connect, --screenshot
  lazyaicode-cli                        commands | describe | run | batch | state | journal | mcp | serve
  lazyaicode-web                        a UI no navegador (trunk), ligada a um `serve` por WebSocket
  lazyaicode-wsl-agent                  o backend git dentro de uma distro WSL (binário `lazyaicode-agent`)
xtask/                                  cargo xtask layers | wasm | i18n | commands | book | web | dist | screenshots | ci
fuzz/                                   alvos do cargo-fuzz + smoke em Rust estável
book/                                   documentação (mdBook)
packaging/                              AppImage, Flatpak, DMG, MSI, FreeBSD
```

**As camadas são conferidas** pelo `cargo xtask layers`: um crate só depende
de camadas abaixo; nada abaixo da L4 usa egui/eframe/winit/wgpu/rfd; a UI
(`ui-egui`) **não** depende de `engine`, `automation`, `git` nem `agent`. Um
crate novo precisa entrar em `xtask/src/layers.rs`.

## 3. Regras de ouro

### Nunca entrar em pânico (vale mais que feature)

O app roda agentes que editam o código das pessoas; um crash perde a
conversa e o contexto. Uma linha estranha de um agente, um parâmetro errado
de um script, um `state.json` corrompido, um disco cheio — tudo vira um erro
que a pessoa ou o agente consegue tratar, nunca um pânico.

- **Código de produção não entra em pânico.** Nada de `unwrap()`,
  `expect()`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`. Devolva o
  erro do crate e propague com `?`; use `ok_or(..)?`, `let .. else`,
  `if let`, `unwrap_or*` onde um padrão é de fato correto. Os crates negam
  esses lints (`#![deny(clippy::unwrap_used, …)]`) e o CI roda o clippy com
  `-D warnings`. Testes podem.
- **Sem `unsafe`.** O workspace tem `unsafe_code = "forbid"`.
- **Número que vem de fora é hostil.** `get()` em vez de `[i]`; fatie
  strings em fronteira de caractere; `checked_*`/`saturating_*`; limite
  alocações pelo tamanho **real**, não pelo declarado (`take`,
  `with_capacity(min(..))`).
- **Limite recursão e linhas.** Linhas de agentes ≤ 64 MiB
  (`lazyaicode_agent::lines::BoundedLines`), JSON com profundidade ≤ 128.
- **Não propague.** Mutex envenenado: `lock().unwrap_or_else(PoisonError::into_inner)`.
- **Rede de segurança:** o motor roda cada comando numa task; um pânico
  escapado vira `EngineError::Panicked`. Não é licença para entrar em pânico.
- **Prove.** Toda correção de crash vem com um teste que entrava em pânico
  antes. Parser novo ganha um alvo em `fuzz/` e sementes em `fuzz/seeds/`.

### Tudo é um comando

Comportamento visível = um comando no motor (`crates/engine/src/*_cmds.rs`),
com id, rótulo, grupo, parâmetros, `read_only`/`destructive` e `run`, mais
testes pelo `Engine::run`. A UI, a CLI, o canal de controle e o MCP chamam
pelo id. Só estado de tela (abas, painéis, zoom, tema) fica na casca
(`crates/ui-egui/src/commands.rs`, ids `ui.*`). O total passa de 500 e um
teste do app não deixa cair nem repetir id entre motor e tela. Depois de
mexer, `cargo xtask commands` regenera a referência.

Os comandos do cliente git (`git.*`, `github.*`, `gitlab.*`, `ai.*`) são
gerados de `crates/engine/src/git_cmds/*.rs` por
`python3 xtask/scripts/gen_git_commands.py`; não edite `git_registry.rs` à mão.

### A UI é fina

- Ela desenha a partir de `data.rs` (cópia do que veio do motor) e age por
  `app.call(id, params, callback)` / `app.fire(id, params)`.
- Estado de tela em `state.rs` (serde): é o que `ui.inspect` devolve.
- Cores só de `theme::Tokens`; ícones Phosphor; textos por `t("área.chave")`.
- Toda chave nova nos quatro idiomas (`cargo xtask i18n`;
  `xtask/scripts/i18n_add.py` acrescenta).

### Confira a tela de verdade

Renderize sem janela e **olhe o PNG**:

```sh
cargo run -p lazyaicode -- --ephemeral --screenshot /tmp/x.png --run 'ui.palette.commands {}'
```

Capturas para o book e o README saem de `cargo xtask screenshots`, que monta
um workspace de demonstração neutro. Nunca publique captura com nome, e-mail
ou caminho de uma pessoa (o perfil vem do `git config`).

### Nunca quebre o wasm

`cargo xtask wasm`. Código de disco/processo fica no motor, não na UI.

### Testes são o portão

Toda mudança vem com teste. Antes de cada commit:

```sh
cargo xtask ci     # fmt --check, layers, i18n, clippy -D warnings, test --workspace, wasm
```

## 4. Rodar e dirigir o app

```sh
cargo run -p lazyaicode                                   # janela
cargo run -p lazyaicode -- --control 127.0.0.1:7878       # + canal de controle
cargo run -p lazyaicode-cli -- --connect 127.0.0.1:7878 run ui.inspect
cargo run -p lazyaicode-cli -- --ephemeral run engine.commands filter=thread
LAZYAICODE_CLAUDE_BIN=/caminho/para/um/claude-falso cargo run -p lazyaicode   # sem gastar tokens
```

`LAZYAICODE_DATA_DIR` (ou `--data-dir`) separa o estado do seu uso normal.

## 5. Commits

- Mensagens em português, `tipo(área): descrição` (`feat(git): …`,
  `fix(chat): …`, `docs(book): …`), um assunto por commit.
- Nunca commite segredos, capturas pessoais, `target/`, `dist/`, `fuzz/corpus/`.

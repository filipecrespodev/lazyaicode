# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# LazyAICode — instruções para agentes

Leia o [`AGENTS.md`](AGENTS.md) primeiro: ele tem o mapa do workspace, as
camadas e as regras. Este arquivo é o resumo do que mais pesa numa sessão.

## Comece toda sessão aqui

1. [`ROADMAP.md`](ROADMAP.md): o estágio e o que vem a seguir. Sem tarefa
   dada, pegue da lista "Próximos".
2. O capítulo do [book](book/src/README.md) da área que vai mexer
   (arquitetura, automação, segurança).
3. Oriente → planeje → implemente com teste → verifique (`cargo xtask ci` e,
   se mexeu na tela, uma captura) → commite.

## Inegociáveis

- **Rust só.** Sem Tauri, Electron, webview ou JS/TS escrito à mão.
- **Nunca entrar em pânico** em código de produção: nada de `unwrap`,
  `expect`, `panic!`, indexação sem checagem; entrada de fora é hostil e tem
  teto. Correção de crash vem com teste; parser novo, com alvo em `fuzz/`.
- **Tudo é um comando** no registro do motor; a UI, a CLI, o canal de
  controle e o MCP chamam pelo id. Mais de 500 — não deixe cair.
- **A UI não liga o motor** (`cargo xtask layers`). Ela fala por `Client`.
- **Nunca quebre o wasm** (`cargo xtask wasm`).
- **Textos nos quatro idiomas** (`cargo xtask i18n`).
- **Sem dados pessoais** em capturas, testes ou documentação: use
  `cargo xtask screenshots` (demo neutro).

## Comandos úteis

```sh
cargo xtask ci                      # o portão: fmt --check, layers, i18n, clippy -D warnings, test, wasm
cargo test -p lazyaicode-engine nome_do_teste   # um teste só (pacotes: lazyaicode-<crate>)
cargo xtask layers | wasm | i18n    # os portões separados, mais rápidos que o ci inteiro
cargo xtask commands                # regenera book/src/reference/{commands,keyboard}.md
python3 xtask/scripts/gen_git_commands.py   # depois de mexer em crates/engine/src/git_cmds/
cargo run -p lazyaicode -- --ephemeral --screenshot /tmp/x.png --run 'ui.settings.appearance {}'
cargo test --manifest-path fuzz/Cargo.toml  # smoke de fuzzing
mdbook build book
```

`LAZYAICODE_CLAUDE_BIN=<claude falso>` roda o app sem gastar tokens;
`LAZYAICODE_DATA_DIR` (ou `--data-dir`) separa o estado do seu uso normal.

## O caminho de um comando (o que exige ler vários arquivos)

- Comando do motor: `crates/engine/src/*_cmds.rs` (id, parâmetros,
  `read_only`/`destructive`, `run`); teste por `Engine::run` em
  `crates/engine/src/tests.rs`. O motor valida os parâmetros (`check_params`:
  nenhuma chave desconhecida), roda numa task própria e registra no journal.
- Comandos `git.*`/`github.*`/`gitlab.*`/`ai.*`: edite `git_cmds/*.rs` e
  regenere; `git_registry.rs` é gerado.
- Comando de tela (`ui.*`): `crates/ui-egui/src/commands.rs`; estado de tela
  em `state.rs` (o que `ui.inspect` devolve), cópia dos dados do motor em
  `data.rs`. A UI chama via `app.call`/`app.fire` → `Client` (`LocalClient`
  no desktop, `WsClient` na web contra `lazyaicode-cli serve`).
- O que vem depois (saída do agente, status, terminal) chega como eventos
  (`agent-event`, `thread-status`, `term-data`…) a todos os clientes.
- O teste `engine_and_screen_commands_together_pass_500_without_collisions`
  (`apps/lazyaicode/src/main.rs`) quebra se ids colidirem ou o total cair.

## Dirigindo o app

`lazyaicode --control 127.0.0.1:7878` abre o canal de controle (token em
`<dados>/control.token`); `lazyaicode-cli --connect 127.0.0.1:7878 run ui.inspect`
mostra a tela; `ui.screenshot {"cpu": true}` captura sem GPU. Para agentes,
`lazyaicode-cli mcp` (ou `--bridge` para o app aberto) expõe `command_list`,
`command_run`, `thread_send_and_wait` e companhia.

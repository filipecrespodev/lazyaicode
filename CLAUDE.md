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
cargo xtask ci                      # o portão
cargo xtask commands                # regenera book/src/reference/{commands,keyboard}.md
python3 xtask/scripts/gen_git_commands.py   # depois de mexer em crates/engine/src/git_cmds/
cargo run -p lazyaicode -- --ephemeral --screenshot /tmp/x.png --run 'ui.settings.appearance {}'
cargo test --manifest-path fuzz/Cargo.toml  # smoke de fuzzing
mdbook build book
```

## Dirigindo o app

`lazyaicode --control 127.0.0.1:7878` abre o canal de controle (token em
`<dados>/control.token`); `lazyaicode-cli --connect 127.0.0.1:7878 run ui.inspect`
mostra a tela; `ui.screenshot {"cpu": true}` captura sem GPU. Para agentes,
`lazyaicode-cli mcp` (ou `--bridge` para o app aberto) expõe `command_list`,
`command_run`, `thread_send_and_wait` e companhia.

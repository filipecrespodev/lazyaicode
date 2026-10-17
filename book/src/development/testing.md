# Testes

```sh
cargo test --workspace                        # tudo (~380 testes)
cargo test -p lazyaicode-engine               # o motor, pelo caminho dos clientes
cargo test --manifest-path fuzz/Cargo.toml    # smoke de fuzzing (Rust estável)
cargo xtask ci                                # o portão completo
```

| Onde | O que testa |
|---|---|
| `crates/engine/tests/every_command.rs` | **todas as funcionalidades do motor**: o cenário de `lazyaicode_testkit::scenario` roda cada comando do registro com o efeito conferido; falha se um comando ficar sem teste |
| `apps/lazyaicode-cli/tests/e2e.rs` | **ponta a ponta com o binário**: o mesmo cenário contra um `lazyaicode-cli serve` pelo canal TCP e pelo WebSocket da versão web, e contra o `lazyaicode-cli mcp`; mais as ferramentas do MCP e a CLI de uma chamada (`run`, `batch`, `--connect`…) |
| `crates/engine/src/tests.rs` | o motor por `Engine::run`: workspace → projeto → thread isolada → revisão → integrar → arquivar; comandos git pelo ator; um `claude` falso falando stream-json (`thread.send` até os eventos); o panic hunt |
| `crates/automation` | canal TCP (auth, eventos, linhas gigantes e lixo), WebSocket + arquivos estáticos + `Origin`, MCP (JSON-RPC, ferramentas, recursos) |
| `crates/git` | ~245 testes herdados do cliente git (staging por linha, rebase interativo, hooks, assinatura, stash, undo…) |
| `crates/render-cpu/tests/oracle.rs` | o compositor em CPU contra o wgpu |
| `crates/ui-egui` | comandos de tela (ids, atalhos sem conflito), tema (contraste WCAG ≥ 4,5 em todas as paletas), menu `/`, imagens |
| `apps/lazyaicode` | mais de 500 comandos, sem id repetido entre motor e tela |
| `apps/lazyaicode-wsl-agent/tests` | o protocolo do agente WSL por um processo real |
| `fuzz/` | os alvos de [fuzzing](../security/fuzzing.md) |

## O cenário de todas as funcionalidades

`lazyaicode_testkit::scenario::run(runner, fixture)` é uma sequência que
passa por **todos** os comandos do motor num ambiente real e hermético
(`Fixture`): um repositório git com remoto bare, um workspace com um projeto
git e uma pasta comum, `HOME`, configuração do git e pasta de dados no
tempdir, e um `PATH` com programas falsos (`lazyaicode_testkit::fakes`):

- `claude`: fala stream-json (conversa, pedidos de permissão, `mcp_status`…),
  grava a sessão em `<conta>/projects/…/<sessão>.jsonl` como o Claude Code, e
  responde às ações de IA do modo `-p` (mensagem de commit, PR, conflito,
  composição de commits). Só a conta de `$FAKE_CLAUDE_LOGGED_IN` tem login, e
  uma permissão sem o `toolUseID` do pedido é perguntada de novo — como o CLI
  real;
- `codex-acp` e `gemini`: um agente ACP;
- `xdg-open`, `open`, `wslview` e um editor: só registram o que mandaram abrir.

O cenário recebe um `Runner` (um `call(id, params)`), então o mesmo roteiro
roda em processo, pelo TCP, pelo WebSocket e pelo MCP. Comando novo sem passo
no cenário quebra o teste: acrescente o passo (com a conferência do efeito)
em `crates/testkit/src/scenario.rs`.

## Um `claude` falso

Para testar o caminho inteiro sem rede, aponte `LAZYAICODE_CLAUDE_BIN` para
um script que fala stream-json (`lazyaicode_testkit::fake_claude`). É o que
o teste `send_streams_agent_events_and_updates_the_thread` faz, e o que dá
para usar à mão:

```sh
LAZYAICODE_CLAUDE_BIN=./fake-claude lazyaicode-cli --ephemeral batch turno.json
```

## Testes manuais com o Claude real

`cargo test -p lazyaicode-agent -- --ignored real_claude` roda uma conversa
de verdade (gasta alguns tokens do Haiku): o agente pede permissão, edita o
checkout isolado e a mudança aparece no diff.

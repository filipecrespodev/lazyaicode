# Testes

```sh
cargo test --workspace                        # tudo (~380 testes)
cargo test -p lazyaicode-engine               # o motor, pelo caminho dos clientes
cargo test --manifest-path fuzz/Cargo.toml    # smoke de fuzzing (Rust estável)
cargo xtask ci                                # o portão completo
```

| Onde | O que testa |
|---|---|
| `crates/engine/src/tests.rs` | o motor por `Engine::run`: workspace → projeto → thread isolada → revisão → integrar → arquivar; comandos git pelo ator; um `claude` falso falando stream-json (`thread.send` até os eventos); o panic hunt |
| `crates/automation` | canal TCP (auth, eventos, linhas gigantes e lixo), WebSocket + arquivos estáticos + `Origin`, MCP (JSON-RPC, ferramentas, recursos) |
| `crates/git` | ~245 testes herdados do cliente git (staging por linha, rebase interativo, hooks, assinatura, stash, undo…) |
| `crates/render-cpu/tests/oracle.rs` | o compositor em CPU contra o wgpu |
| `crates/ui-egui` | comandos de tela (ids, atalhos sem conflito), tema (contraste WCAG ≥ 4,5 em todas as paletas), menu `/`, imagens |
| `apps/lazyaicode` | mais de 500 comandos, sem id repetido entre motor e tela |
| `apps/lazyaicode-wsl-agent/tests` | o protocolo do agente WSL por um processo real |
| `fuzz/` | os alvos de [fuzzing](../security/fuzzing.md) |

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

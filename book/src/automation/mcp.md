# MCP

`lazyaicode-cli mcp` é um servidor **MCP** (Model Context Protocol, revisão
`2025-06-18`) no stdio: JSON-RPC 2.0, uma mensagem por linha.

```sh
# motor no processo do MCP (mesmo state.json do app)
claude mcp add lazyaicode -- lazyaicode-cli mcp
# ponte para um app aberto com --control (vê a tela também)
claude mcp add lazyaicode -- lazyaicode-cli mcp --bridge 127.0.0.1:7878 --token-file ~/.local/share/lazyaicode/control.token
```

## Ferramentas

| Ferramenta | O quê |
|---|---|
| `command_list {filter?, group?}` | os comandos (id, rótulo, grupo, parâmetros) — comece por aqui |
| `command_describe {id}` | a descrição completa de um comando |
| `command_run {command, params}` | executa um comando |
| `command_batch {steps, stopOnError?}` | até 256 comandos em sequência |
| `state_get` | workspaces, projetos, threads com status, preferências |
| `journal {last?}` | os últimos comandos que mudaram algo |
| `thread_send_and_wait {id, text, timeoutMs?}` | manda a mensagem e espera o turno: devolve o texto final, as ferramentas usadas, o custo e a permissão pendente |
| `thread_wait {id, timeoutMs?}` | espera a thread sair de `running` (ex.: depois de responder uma permissão) |
| `ui_inspect`, `ui_screenshot`, `ui_command` | só na ponte: a tela do app aberto |

Recursos: `lazyaicode://commands`, `lazyaicode://state`, `lazyaicode://journal`.

Cada pedido roda numa task própria: um `thread_send_and_wait` de minutos não
segura os outros. O `thread_send_and_wait` assina os eventos **antes** de
mandar a mensagem, então nenhum evento do turno se perde.

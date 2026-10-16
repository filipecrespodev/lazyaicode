# Automação

Tudo o que a interface faz é um comando do mesmo registro, então um script
ou um agente de IA faz o mesmo, por três caminhos:

| Caminho | Quando usar | Onde o motor roda |
|---|---|---|
| [`lazyaicode-cli`](cli.md) | scripts, CI, lotes em JSON | no processo da CLI (ou num app aberto, com `--connect`) |
| [Canal de controle](control-protocol.md) | dirigir o **app aberto** (inclusive a tela: `ui.inspect`, `ui.screenshot`, `ui.run`) | no app, ou num `lazyaicode-cli serve` |
| [MCP](mcp.md) | um agente (Claude Code, outro cliente MCP) dirigindo o LazyAICode | no processo do MCP, ou ponte para o app aberto |

Os três usam os mesmos ids e parâmetros da [referência de comandos](../reference/commands.md).

## Um exemplo de ponta a ponta

```sh
# workspace e projeto
W=$(lazyaicode-cli run workspace.create path=$HOME/code | jq -r .id)
P=$(lazyaicode-cli run project.list workspaceId=$W | jq -r '.[] | select(.name=="api") | .id')
# thread isolada
T=$(lazyaicode-cli run thread.create workspaceId=$W projectId=$P isolated=true | jq -r .id)
# conversa e espera o turno no MESMO processo (o agente vive com o motor)
cat > turno.json <<JSON
[{"command": "thread.send", "params": {"id": "$T", "text": "adicione testes para o parser de datas"}},
 {"command": "thread.wait", "params": {"id": "$T", "timeoutMs": 600000}},
 {"command": "thread.last-message", "params": {"id": "$T"}}]
JSON
lazyaicode-cli batch turno.json
# o que mudou, e trazer para o projeto
lazyaicode-cli run thread.changes id=$T base=upstream | jq '.files[].path'
lazyaicode-cli run thread.land id=$T mode=apply
```

Cada `lazyaicode-cli run` sobe e encerra o motor; os agentes vivem com ele.
Para conversas entre invocações, deixe um motor de pé e conecte nele:

```sh
lazyaicode-cli serve --control 127.0.0.1:7878 &      # ou o app: lazyaicode --control 127.0.0.1:7878
lazyaicode-cli --connect 127.0.0.1:7878 run thread.send id=$T text="…"
lazyaicode-cli --connect 127.0.0.1:7878 run thread.wait id=$T
```

Com o MCP, o mesmo fluxo é `command_run` + `thread_send_and_wait`.

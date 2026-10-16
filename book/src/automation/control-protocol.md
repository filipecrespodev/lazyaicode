# Canal de controle

JSON delimitado por linha (um objeto por linha, `\n`), sobre:

- **TCP** em loopback: `lazyaicode --control 127.0.0.1:7878` (o app) ou
  `lazyaicode-cli serve --control 127.0.0.1:7878` (sem janela);
- **WebSocket** em `/ws`: `lazyaicode-cli serve --web …` (uma mensagem de
  texto por quadro).

## Quadros

```json
{"id":1,"method":"auth","params":{"token":"<64 hex>"}}
{"id":1,"result":{"ok":true}}
{"id":2,"method":"engine.run","params":{"command":"thread.list","params":{"workspaceId":"…"}}}
{"id":3,"method":"thread.get","params":{"id":"…"}}
{"id":3,"error":{"code":"not_found","message":"Thread não encontrada"}}
{"id":4,"method":"events.subscribe","params":{"events":["thread-status","agent-event"]}}
{"event":"thread-status","payload":{"threadId":"…","status":"running"}}
```

- O **primeiro** quadro é `auth`; token errado fecha a conexão.
- Cada pedido tem `id` (número ou texto, volta igual), `method` e `params`.
- `method` é `engine.run {command, params}` ou, como atalho, o próprio id do
  comando.
- Pedidos são atendidos **em paralelo**: um `thread.send` longo não segura
  um `engine.state`.
- Erros: `{code, message, data?}` com `code` em `unauthorized`,
  `bad_request`, `unknown_method`, `not_found`, `disabled`, `failed`, `git`
  (com `data = {kind, details}` do erro do git), `timeout`, `too_large`,
  `internal`.

## Métodos do protocolo

| Método | O quê |
|---|---|
| `auth {token}` | autentica a conexão |
| `hello` | app, versão e versão do protocolo |
| `methods` | os métodos do protocolo e, no app aberto, os `ui.*` |
| `engine.run {command, params}` | executa um comando do motor |
| `events.subscribe {events}` / `events.unsubscribe` | passa (ou para) a receber eventos; lista vazia = todos |
| `<id de comando>` | atalho para `engine.run` |

No **app aberto**, os `ui.*` vão para a janela:

| Método | O quê |
|---|---|
| `ui.inspect` | o estado da tela (aba, thread aberta, painéis, diálogos, contagens, texto do terminal) |
| `ui.screenshot {cpu?}` | PNG em base64 da janela (`renderer: wgpu`) ou do [compositor em CPU](../architecture/rendering.md) |
| `ui.run {command, params}` | qualquer comando de tela (`ui.tab.terminal`, `ui.palette.open`…) |
| `ui.state.set {patch}` | mescla campos no estado de tela |

## Limites

| Limite | Valor |
|---|---|
| linha de pedido | 1 MiB (acima: `too_large`, o resto da linha é descartado) |
| resposta | 8 MiB (acima: `too_large`) |
| profundidade do JSON | 128 |
| conexões simultâneas | 16 |
| passos de `engine.batch` | 256 |
| tempo de um pedido | 600 s (`timeout`) |

`engine.limits` devolve a tabela ao vivo.

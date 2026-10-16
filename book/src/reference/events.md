# Eventos

O motor publica eventos a quem assinou (`events.subscribe` no canal de
controle; o cliente local da UI recebe todos). Formato:
`{"event": "<nome>", "payload": {…}}`.

| Evento | Payload | Quando |
|---|---|---|
| `agent-event` | `{threadId, msg}` | cada linha do agente (stream-json do Claude Code, ou o ACP traduzido): `system/init`, `stream_event`, `assistant`, `user`, `control_request`, `result`, `lazy_notice`, `lazy_usage`, `lazy_commands`, `lazy_config` |
| `agent-exit` | `{threadId, code, stderr}` | o processo do agente terminou (`stderr` só quando falhou no meio de um turno) |
| `thread-status` | `{threadId, status}` | `idle` / `running` / `awaiting` / `error` — sempre **depois** do `agent-event` que causou a mudança |
| `thread-updated` | a thread com status | título, não lida, sessão… mudaram |
| `term-data` | `{termId, data}` | bytes do PTY em base64; `data: null` = o terminal fechou |
| `state-changed` | `{what}` | `workspaces`, `threads`, `settings` ou `prefs` mudaram |
| `ssh-passphrase-request` | a chave | o git precisa da senha de uma chave SSH |
| `https-credentials-request` | `{url, username?}` | o git precisa de usuário e senha/token |
| `activity://log` | `{level, message, repoId?}` | registro do cliente git |
| `git://fetch-progress`, `git://push-progress` | progresso | fetch/push em andamento |
| `git://autostash-conflict` | `{repoId, paths}` | o stash automático conflitou ao voltar |
| `notice` | `{text, kind}` | aviso para mostrar |
| `events-lagged` | `{missed}` | o cliente ficou para trás e perdeu eventos |

`engine.events` lista os nomes ao vivo.

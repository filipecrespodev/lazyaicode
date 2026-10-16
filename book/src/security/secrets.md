# Segredos e credenciais

| Segredo | Onde fica | Como o app trata |
|---|---|---|
| login do Claude Code | na conta (`~/.claude…`) | o app não lê: só passa `CLAUDE_CONFIG_DIR` ao processo |
| `OPENAI_API_KEY`, `GEMINI_API_KEY` | ambiente | herdados pelos agentes ACP; não vão para o estado |
| senha de chave SSH | memória, durante a operação | pedida pela UI (`ssh-passphrase-request`); operação em segundo plano nunca pede |
| credenciais HTTPS do git | cache em memória por host | pedidas pela UI (`https-credentials-request`) |
| tokens GitHub/GitLab | `prefs` no `state.json` | **limitação conhecida:** texto puro no arquivo do usuário |
| token do canal de controle | `<dados>/control.token`, 0600 | gerado a cada `--control`/`serve` |

## Redação

- **Journal:** valores de chaves cujo nome contém `password`, `passphrase`,
  `token`, `secret`, `data` ou `images` viram `"***"`, em qualquer
  profundidade, e textos acima de 2 000 caracteres são resumidos
  (`journal::redact`).
- **MCP dos agentes:** só nome, status, escopo, transporte, URL sem query e
  nome do executável chegam à UI (`catalog::redact_mcp`).
- **Logs:** o app não registra parâmetros de comando nem conteúdo de
  mensagens.

## Proposto

Guardar os tokens do GitHub/GitLab no chaveiro do sistema (Keychain,
Secret Service, Credential Manager) em vez do `state.json`.

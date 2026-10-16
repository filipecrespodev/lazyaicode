# Agentes

O app não tem chave nem conta própria: roda os agentes que estão na
máquina, com as credenciais deles.

## Claude Code

Um processo `claude -p` por thread (`crates/agent/src/agent.rs`), falando
**stream-json** nos dois sentidos:

```text
claude -p --input-format stream-json --output-format stream-json --verbose \
       --include-partial-messages --permission-prompt-tool stdio \
       [--model …] [--permission-mode …] [--effort …] [--agent …] \
       [--add-dir …] [--dangerously-skip-permissions] [--resume <sessão>]
```

- O processo fica vivo entre turnos (cada mensagem é uma linha no stdin) e,
  quando morre, é recriado com `--resume`.
- A conta é o `CLAUDE_CONFIG_DIR` do workspace (`~/.claude`,
  `~/.claude-trabalho`…). Uma sessão só continua na conta onde nasceu.
- Pedidos de permissão chegam como `control_request {subtype: can_use_tool}`
  (o mesmo protocolo do Agent SDK); o app responde com `thread.respond-permission`.
- Modelo e modo mudam com o processo aberto (`set_model`,
  `set_permission_mode`); esforço, agente e pastas extras só valem ao abrir:
  com o agente parado, o processo é encerrado e o próximo turno retoma a
  sessão já com as opções novas.
- O histórico vem do `.jsonl` que o Claude Code grava
  (`<conta>/projects/<cwd codificado>/<sessão>.jsonl`); o app não duplica.
- `thread.sync-sessions` importa como threads as sessões da conta que
  rodaram dentro das pastas do workspace.
- O binário é achado no PATH, em locais de instalação conhecidos, ou em
  `LAZYAICODE_CLAUDE_BIN`.

## Codex e Gemini CLI (ACP)

`crates/agent/src/acp.rs` fala o **Agent Client Protocol** (JSON-RPC no
stdio) com o `codex-acp` e o `gemini --experimental-acp`. Sem o binário no
PATH, roda via `npx -y <pacote@versão fixa>`; `LAZYAICODE_CODEX_ACP` e
`LAZYAICODE_GEMINI` apontam um binário à mão.

`acp_translate.rs` converte as atualizações ACP (`agent_message_chunk`,
`tool_call`, `plan`, `usage_update`…) para o **mesmo** stream-json do Claude
Code, então a UI e o reducer da transcrição não sabem qual provedor está do
outro lado. As threads ACP têm a transcrição gravada pelo app
(`<dados>/transcripts/<thread>.jsonl`).

## Skills, agentes e MCP

`capabilities.get` sonda o Claude Code numa pasta com a conta do workspace
(o `init` do stream lista comandos, agentes, MCP e modelos) e cruza com o
disco (`catalog.rs` lê o frontmatter de `skills/`, `agents/` e `commands/`
da conta e do `.claude/` do projeto). O resultado fica 2 minutos em cache.
Da configuração dos MCP só saem nome, status, escopo, transporte, a URL
**sem a query string** e o nome do executável (`redact_mcp`): variáveis de
ambiente, cabeçalhos e argumentos — onde ficam tokens — não chegam à UI.

## Terminais

`terminal.rs` abre um shell num PTY (`portable-pty`) no checkout da thread.
Os bytes saem como eventos `term-data` (base64) e a UI os entrega ao
emulador (`lazyaicode-term`, sobre `vt100`). Numa pasta de distro WSL, o
shell roda dentro da distro (`wsl.exe -d <distro> --cd <caminho>`).

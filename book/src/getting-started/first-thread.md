# Primeira thread

1. **Workspace.** Na primeira abertura, escolha uma pasta que agrupa seus
   repositórios (ex.: `~/code`). Cada subpasta vira um projeto na barra
   lateral; a conta Claude do workspace (`~/.claude`, `~/.claude-trabalho`…)
   é a que os agentes dele usam.
2. **Thread.** `Ctrl+N` abre uma thread no projeto selecionado, trabalhando
   direto na pasta; `Ctrl+Shift+N` abre uma **isolada**: um `git worktree`
   numa branch própria (`lazy/<id>`), criada a partir do upstream. Nada
   chega ao projeto até você revisar e integrar.
3. **Conversa.** Escreva a tarefa e `Enter`. `/` abre os skills e comandos
   que o Claude Code carrega naquela pasta; `Ctrl+Shift+V` ou arrastar anexa
   imagens. Pedidos de permissão aparecem como cartões (`Ctrl+Y` permite,
   `Ctrl+Shift+Y` permite sempre, `Ctrl+Shift+X` nega).
4. **Revisão.** Na aba **Histórico** (`Ctrl+2`), **Revisão do agente**
   (`Ctrl+Shift+R`) lista os arquivos mudados desde o início da thread, o
   último turno ou o último commit, com o diff de cada um e a marca de
   "revisado" (que se desfaz sozinha se o diff mudar).
5. **Integrar.** Em uma thread isolada, **Integrar** aplica as mudanças como
   não commitadas no checkout principal, faz merge da branch, ou pede ao
   agente um PR.

Tudo isso também é comando: `Ctrl+K` abre a paleta com os mais de 500
comandos do app, e os mesmos rodam pela [CLI](../automation/cli.md), pelo
[canal de controle](../automation/control-protocol.md) e pelo [MCP](../automation/mcp.md).

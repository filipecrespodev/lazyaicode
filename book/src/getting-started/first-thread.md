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
6. **Vários workspaces.** No desktop, cada workspace abre na sua própria
   janela (o **+** da barra de título lista os workspaces; um que já está
   aberto ganha o foco). O título da janela resume o que pede atenção nela
   (`[⏸ 1 · ▶ 2 · ● 1] Loja — LazyAICode`: esperando você, rodando,
   terminadas sem ler), e o contador ao lado da busca (`Ctrl+Shift+D`) lista
   as sessões de **todos** os workspaces nesses três estados; clicar numa
   leva à janela dela. `Ctrl+Shift+T` vai direto para a próxima que espera
   você ou terminou sem ser lida. Fechar a janela principal mantém as outras;
   `Ctrl+Q` fecha todas. Na versão web, os workspaces seguem em abas.
7. **Versões novas.** O app consulta o último release ao abrir (e a cada
   6 h) e mostra um botão na barra de título quando há versão nova; em
   **Configurações → Sobre** dá para verificar na hora ou desligar o aviso.

Tudo isso também é comando: `Ctrl+K` abre a paleta com os mais de 500
comandos do app, e os mesmos rodam pela [CLI](../automation/cli.md), pelo
[canal de controle](../automation/control-protocol.md) e pelo [MCP](../automation/mcp.md).

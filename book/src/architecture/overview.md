# Visão geral

```text
                   ┌──────────────── apps ────────────────┐
                   │ lazyaicode (eframe+wgpu)  lazyaicode-web (wasm)
                   │ lazyaicode-cli            lazyaicode-wsl-agent
                   └──────┬──────────────┬──────────────┬─┘
                          │ Client       │ WebSocket    │ stdio (MCP)
         ┌────────────────┴───┐   ┌──────┴──────────────┴───────┐
   L4    │ ui-egui  render-cpu│   │ automation (L3): controle   │
         │ (só comandos)      │   │ TCP/WS, MCP, ponte          │
         └────────┬───────────┘   └──────────────┬──────────────┘
                  │ ids de comando + JSON         │
   L2      ┌──────┴───────────────────────────────┴──────┐
           │ engine: Session + registro de comandos      │
           └─────┬──────────────┬──────────────┬─────────┘
   L1        doc (dados)     agent           git
   L0        protocol  i18n  term  platform  testkit
```

O LazyAICode é um **motor** com uma **interface fina** por cima:

- O **motor** (`lazyaicode-engine`) é a sessão do app: o estado persistido
  (`state.json`), os processos dos agentes, os terminais, os repositórios
  abertos e o registro de comandos. Toda ação — da UI, da CLI, do canal de
  controle ou do MCP — é um `Engine::run(id, params)`.
- O **modelo de dados** (`lazyaicode-doc`) é só dados: workspaces, projetos,
  threads, preferências, a transcrição da conversa, a árvore da barra
  lateral, o grafo de commits. Sem IO, compila para wasm.
- A **interface** (`lazyaicode-ui-egui`) desenha a partir de uma cópia local
  dos dados e age **apenas** por comandos, através de um `Client`. Ela não
  depende do motor (o `cargo xtask layers` garante): o mesmo código roda no
  desktop (cliente local, motor no processo) e no navegador (cliente
  WebSocket, motor num `lazyaicode-cli serve`).
- A **automação** (`lazyaicode-automation`) expõe o mesmo registro ao mundo:
  JSON por linha em TCP loopback (`--control`), o mesmo em WebSocket para a
  versão web, e um servidor MCP no stdio.

## O caminho de um comando

1. O usuário aperta `Ctrl+N` → a UI acha o comando de tela `ui.thread.new`,
   que monta `thread.create {workspaceId, projectId, isolated}` com o
   contexto da tela.
2. `client.call("thread.create", params)` devolve um ticket. No desktop o
   `LocalClient` roda `engine.run` numa task do tokio; na web o `WsClient`
   manda `{"id":…,"method":"thread.create","params":…}`.
3. O motor confere os parâmetros contra a descrição do comando
   (`check_params`: tipos, obrigatórios, nenhuma chave desconhecida), roda
   numa task própria (um pânico vira erro, não derruba ninguém), registra no
   journal e responde.
4. A resposta volta no próximo quadro; o callback atualiza a cópia local.
5. O que acontece depois (o agente escrevendo, o status da thread, a saída
   do terminal) chega como **eventos** (`agent-event`, `thread-status`,
   `term-data`…) a todos os clientes assinantes.

## Pastas

| Pasta | O quê |
|---|---|
| `crates/` | as bibliotecas, por camada |
| `apps/` | os executáveis |
| `book/` | esta documentação (mdBook) |
| `fuzz/` | alvos de fuzzing (cargo-fuzz) e o smoke em Rust estável |
| `packaging/` | receitas de AppImage, Flatpak, DMG, MSI, FreeBSD |
| `xtask/` | checagens e empacotamento (`cargo xtask …`) |
| `assets/` | ícones e marca |

<p align="center"><img src="docs/brand/logo.png" width="128" alt="LazyAICode" /></p>

# LazyAICode

Workspace desktop para agentes Claude Code: threads em paralelo, cada uma num
checkout isolado, com revisão de diff e integração ao projeto — a experiência
do [Delta](https://delta.dev/docs/getting-started) com o motor do trevocode.

- **Backend em Rust** (Tauri 2): processos do agente, git, PTY e estado.
- **UI em Svelte 5** numa webview nativa (binário pequeno, sem Node em runtime).
- **Workspaces com conta própria**: um workspace é uma pasta
  que agrupa repositórios e usa um diretório de configuração do Claude Code
  (`~/.claude`, `~/.claude-trevo`, …). Cada um é uma aba na barra de título.
- **Sem chave própria**: o agente é o `claude` instalado na máquina, rodando
  com a conta do workspace. Login, assinatura, `ANTHROPIC_API_KEY`, plugins,
  skills, MCP e hooks são os mesmos do terminal.
- **Três provedores**: Claude Code (Anthropic), Codex (OpenAI) e Gemini CLI
  (Google), escolhidos no seletor de modelo do rodapé. Codex e Gemini falam
  ACP (Agent Client Protocol) e usam o login deles (`codex login` / conta
  Google) ou `OPENAI_API_KEY` / `GEMINI_API_KEY`. Sem o binário instalado, o
  app roda o agente via `npx` com versão fixa (precisa de Node.js).
- **Janela sem a barra do sistema**: abas, busca, configurações e
  minimizar/maximizar/fechar ficam na barra de título do app.
- **Git dentro da thread**: cada thread tem as abas **Conversa**, **Mudanças**
  (staging por arquivo ou hunk, commit, revisão do que o agente mudou),
  **Histórico** (grafo de commits, branches, tags, stash, reflog, merge/rebase
  com resolução de conflitos, cherry-pick, PRs do GitHub e MRs do GitLab) e
  **Terminal**, todas no checkout da thread (`Ctrl+1…4`). Clicar no nome de um
  repositório na sidebar abre as mesmas abas sobre o checkout principal dele.
- **IA de commit sem chave**: a mensagem de commit e a divisão do staging em
  commits usam o Claude Code com a conta do workspace.

### Organização do código

| | Agentes | Git |
|---|---|---|
| UI | Svelte 5 (`src/lib`, `src/App.svelte`) | React 19 + Tailwind 4 (`src/git`) |
| Backend | `src-tauri/src/*.rs` (git via CLI) | `src-tauri/src/git_client` (libgit2) |

O resto é um só para o app inteiro: workspaces, configurações (`Ctrl+,`, com
as abas Git e Integrações), terminal, tema e paleta (`Ctrl+K`).

Os painéis Git são ilhas React dentro da casca Svelte: `GitPanel.svelte` chama
`mountRepoPanel` (`src/git/mount.tsx`) no elemento da aba, carregando o bundle
do git só na primeira vez. Um repositório é aberto no backend uma vez por
caminho, mesmo que a thread e a tela do repositório o mostrem. O preflight do
Tailwind vale só dentro dos painéis (`src/git/preflight.css`), e o tema de
Configurações → Aparência vale para tudo. A interface Git está sendo migrada
para Svelte painel a painel; o React sai quando a última ilha for migrada.

## Instalação

Baixe o instalador do [último release](https://github.com/filipecrespodev/lazyaicode/releases/latest):

| Sistema | Arquivo |
|---|---|
| Windows | `LazyAICode_x.y.z_x64-setup.exe` (ou `.msi`) |
| macOS (Apple Silicon / Intel) | `LazyAICode_x.y.z_aarch64.dmg` / `LazyAICode_x.y.z_x64.dmg` |
| Linux | `.AppImage` ou `.deb` |
| Linux e WSL | `curl -fsSL https://raw.githubusercontent.com/filipecrespodev/lazyaicode/main/scripts/install.sh \| sh` |

O app verifica atualizações ao abrir e se atualiza sozinho (releases assinados).
Pré-requisito em todos: `git` e o [Claude Code](https://docs.claude.com/en/docs/claude-code) instalado e com login.

### WSL

Use o build **Linux** dentro do WSL2, exibido no Windows pelo WSLg (Windows 11,
ou Windows 10 21H2+):

- `install.sh` baixa o AppImage e cria o comando `lazyaicode`, que roda com
  `--appimage-extract-and-run` (o WSL normalmente não tem FUSE).
- O Claude Code e o `git` precisam estar instalados **dentro** do WSL, e os
  repositórios de preferência no disco do Linux (`~/...`): no `/mnt/c` o git
  fica muito mais lento.
- No WSL o app desliga o renderizador DMABUF/compositing do WebKitGTK (causa
  conhecida de janela em branco no WSLg) e abre arquivos e pastas no Windows
  (`wslview` ou Explorer via `wslpath`).
- No build Windows nativo, o cliente git abre repositórios que estão dentro do
  WSL sem lê-los pelo `\\wsl.localhost` (9P, lento): o app sobe o
  `lazyaicode-agent` dentro da distro e conversa com ele por um pipe, então
  git, hooks, chaves SSH, credenciais e git-lfs rodam nativamente no Linux.
  Use **Do WSL** em *Novo workspace* (ou qualquer pasta
  `\\wsl.localhost\<distro>\...`): a lista de repositórios do workspace, a
  visão Git e o terminal (que abre um shell na distro) passam pelo WSL. O
  instalador leva o agente e o copia para `~/.lazyaicode/` na distro no
  primeiro uso. As threads do Claude Code, porém, ainda rodam o `claude` do
  Windows sobre o 9P; para elas, o build Linux no WSL continua sendo o melhor.
- Dev: `cargo build --no-default-features --bin lazyaicode-agent` (em
  `src-tauri`) compila o agente; `LAZYAICODE_AGENT_BIN=<binário>` faz o app
  rodá-lo direto em vez de passar pelo `wsl.exe`.

## Desenvolvimento

Pré-requisitos: Rust, Node 20+, Claude Code (`claude`) com login feito, e as
dependências de sistema do Tauri (no Linux: `webkit2gtk-4.1`, `libsoup-3.0`).

```bash
npm install
npm run tauri dev               # desenvolvimento
npm run tauri build             # gera .deb e .AppImage em src-tauri/target/release/bundle
```

## Como funciona

| Conceito | Implementação |
|---|---|
| Workspace | Pasta + conta Claude + repositórios ignorados. Os repositórios são os filhos imediatos da pasta (e ela própria, se for repo). |
| Projeto | Um repositório do workspace. |
| Sessões existentes | Sessões do Claude Code da conta do workspace que rodaram dentro da pasta viram threads (sincroniza ao abrir, ao focar a janela e ao trocar de aba). |
| Thread | Um processo `claude -p --input-format stream-json --output-format stream-json --permission-prompt-tool stdio`, vivo entre turnos; recriado com `--resume <session>` quando morre. |
| Workspace isolado | `git worktree` numa branch `lazy/<id>` criada a partir de `origin/HEAD`, guardado em `~/.local/share/lazyaicode/worktrees/`. O checkout principal não muda. Arquivar remove o checkout e mantém a branch; desarquivar o recria. |
| Preparação | `.agents/prepare` (ou `.delta/prepare`) roda na criação; caminhos em `.agents/linked` viram symlinks para o checkout principal (ex.: `.env`). |
| Permissões | `control_request can_use_tool` vira um card Permitir/Negar na conversa. |
| Mudanças | Diff contra uma árvore temporária da pasta (inclui arquivos novos, respeita `.gitignore`, não toca no índice). Bases: desde a origem, desde o último commit, último turno. "Visto" por arquivo se desfaz quando o diff do arquivo muda. |
| Integrar | Aplicar no projeto (`git apply --3way`), merge da branch (exige projeto limpo) ou pedir ao agente commit + push + PR. |
| Histórico | Lido do armazenamento nativo do Claude Code (`<claudeDir>/projects/<cwd>/<session>.jsonl`). |
| Terminal | PTY (`portable-pty`) no checkout da thread, xterm.js na UI. |
| Codex / Gemini | Cliente ACP em `acp.rs` (`codex-acp`, `gemini --experimental-acp`); os eventos são traduzidos para o formato do stream-json do Claude (`acp_translate.rs`), então conversa, permissões, menu `/`, contexto e notificações funcionam iguais. A transcrição dessas threads fica em `~/.local/share/lazyaicode/transcripts/`. Sobrescreva o comando com `LAZYAICODE_CODEX_ACP` / `LAZYAICODE_GEMINI`. |

Estado do app: `~/.local/share/lazyaicode/state.json` (sobrescreva com
`LAZYAICODE_DATA_DIR`). Binário do Claude: PATH, locais de instalação comuns ou
`LAZYAICODE_CLAUDE_BIN`.

## Atalhos

| Ação | Atalho |
|---|---|
| Paleta de comandos | Ctrl K |
| Nova thread | Ctrl N |
| Mudanças | Alt Shift D |
| Terminal | Ctrl ` |
| Favoritar | Ctrl D |
| Thread anterior / próxima | Alt ↑ / ↓ |
| Interromper o agente | Esc |
| Configurações | Ctrl , |
| Workspace anterior / próximo | Ctrl Tab / Ctrl Shift Tab |
| Workspace 1–9 | Alt 1…9 |

## Publicando um release

1. Suba a versão em `package.json`, `src-tauri/Cargo.toml` e `src-tauri/tauri.conf.json`.
2. `git tag vX.Y.Z && git push origin vX.Y.Z`.

O workflow `release` compila macOS (arm64 e x64), Linux (AppImage e deb) e
Windows (NSIS e MSI), assina os pacotes de atualização com
`TAURI_SIGNING_PRIVATE_KEY` (secret do repositório) e publica o `latest.json`
que o updater consulta. A chave privada fica fora do repositório
(`~/.tauri/lazyaicode.key`); sem ela não é possível publicar atualizações.

O workflow `ci` roda os testes em Linux, macOS e Windows e, num runner Windows,
compila e testa dentro do WSL, inclusive abrindo o app (xvfb).

## Testes

```bash
cd src-tauri && cargo test                          # backend
cd src-tauri && cargo test -- --ignored real_claude # ponta a ponta com o claude real (usa Haiku)
cd src-tauri && cargo test -- --ignored real_acp    # cliente ACP contra um agente real (adaptador ACP do Claude)
npm test                                            # redutor da transcrição e parser de diff
npm run check                                       # svelte-check
```

## Licenças de terceiros

Partes da visão Git (`src/git`, `src-tauri/src/git_client`) derivam de código
sob licença MIT, © 2026 João Vitor Melo — ver [`src/git/LICENSE`](src/git/LICENSE).

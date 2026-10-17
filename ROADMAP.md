# Roadmap

**Estágio:** reescrita em Rust puro concluída (v0.3). O app desktop, a CLI,
o canal de controle, o MCP e a versão web rodam sobre o mesmo motor de
comandos (516 comandos: 246 do motor + 270 de tela).

## Pronto

| Área | Estado |
|---|---|
| Motor | sessão, registro de comandos, journal, lotes, eventos, panic hunt |
| Agentes | Claude Code (stream-json), Codex e Gemini (ACP), permissões, MCP da sessão, sessões importadas |
| Threads | diretas e isoladas (worktree), revisão com "visto", integrar (aplicar, merge, PR pelo agente), arquivar |
| UI | barra de título com workspaces, barra lateral, conversa com markdown/ferramentas/permissões/imagens, menu `/`, rodapé (modelo, modo, esforço, contexto, MCP), revisão, git (grafo, staging por arquivo/hunk/linha, commit com IA, branches/tags/stash, push/pull/fetch, conflitos com IA, rebase interativo, blame, histórico de arquivo, comparação, busca, PR/MR, worktrees, remotos, submódulos, LFS, reflog, estatísticas, visão geral com zoom), terminal, paleta, configurações, 13 temas, 4 idiomas |
| Automação | CLI, canal de controle TCP e WebSocket, MCP (headless e ponte) |
| Renderização | wgpu (Metal/Vulkan/DX12/GL/WebGPU) e compositor de referência em CPU testado contra ele |
| Web | wasm + `lazyaicode-cli serve --web`, Dockerfile |
| Empacotamento | MSI, DMG assinado/notarizado, AppImage, Flatpak, deb, rpm, tarballs (Linux, macOS, FreeBSD) |
| Qualidade | clippy `-D warnings` com lints de não-pânico, `unsafe` proibido, 12 alvos de fuzzing, book |

## Próximos (por prioridade)

1. **Telas git que faltam** (os comandos existem): dividir o staging em
   commits com a IA (`ai.compose-commits`), chaves SSH, abrir arquivo no
   editor, log de atividade do git.
2. **Arrastar para reordenar** no rebase interativo (hoje: setas).
3. **Escopos no token** do canal de controle e confirmação de comandos
   destrutivos vindos da automação.
4. **Tokens do GitHub/GitLab no chaveiro** do sistema.
5. **Shaping de texto complexo** (scripts RTL e índicos).
6. **`cargo fuzz` contínuo** no CI.
7. Atualizações automáticas do app.

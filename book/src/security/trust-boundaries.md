# Fronteiras de confiança

| Fronteira | Entrada | Quem lê | Controle |
|---|---|---|---|
| stdout do Claude Code / ACP | JSON por linha | `agent.rs`, `acp.rs`, `lines.rs`, `acp_translate.rs`, `doc::transcript` | linha ≤ 64 MiB (`BoundedLines`), JSON com profundidade ≤ 128 (`serde_json`), reducer sem pânico, resultado de ferramenta ≤ 256 KiB, transcrição ≤ 20 000 itens |
| `.jsonl` das sessões no disco | JSON por linha | `sessions.rs`, `transcript.rs` | leitura linha a linha, linha ≤ 64 MiB, ≤ 50 000 mensagens |
| saída do `git` (CLI) | texto/patch | `git::thread` | patch dividido por arquivo, diffs grandes truncados |
| libgit2 | objetos do repositório | `git::git::*` | a própria libgit2; erros viram `GitCommandError` |
| agente WSL | quadros com prefixo de tamanho | `git::proto` | quadro ≤ 512 MiB **sem** pré-alocar o tamanho declarado |
| PTY | bytes | `lazyaicode-term` (vt100) | tela mínima 2×4 (bug do vt100 achado no fuzzing), fuzzing |
| canal de controle TCP/WS | JSON por linha/mensagem | `automation::server`, `ws` | auth primeiro, linha ≤ 1 MiB, profundidade ≤ 128, resposta ≤ 8 MiB, 16 conexões, 600 s |
| MCP (stdio) | JSON-RPC por linha | `automation::mcp` | mesmos tetos de linha e profundidade, resultado ≤ 8 MiB |
| parâmetros de comando | JSON | `protocol::check_params`, cada `run` | tipos, obrigatórios, chaves desconhecidas recusadas; caminhos validados onde viram efeito |
| HTTP da versão web | caminho do pedido | `ws::resolve_static` | sem `..`, absolutos, `\` ou NUL |
| imagens (anexos, histórico) | PNG/JPEG/GIF/WebP | `system.read-image`, `ui-egui` | ≤ 5 MiB do disco, tipo pelo conteúdo (não pela extensão), cabeçalho ≤ 8192² e 40 Mpx antes de decodificar |
| frontmatter de skills/agentes | texto | `agent::catalog` | parser próprio, fuzzing |
| APIs GitHub/GitLab | JSON | `git::github`, `git::gitlab` | serde tipado; **limitação conhecida:** sem teto de corpo |
| `state.json` | JSON | `engine::store` | corrompido é guardado ao lado e o app abre vazio; gravação atômica |

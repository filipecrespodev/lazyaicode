# Modelo de dados

`lazyaicode-doc` é só dados (serde), sem IO nem processos:

| Tipo | O quê |
|---|---|
| `State` | o `state.json`: `settings`, `workspaces`, `active_workspace`, `projects`, `threads` |
| `Workspace` | pasta que agrupa repositórios + a conta Claude (`claude_dir`) + pastas ignoradas |
| `Project` | uma pasta do workspace (`is_git` diz se tem abas git e checkout isolado) |
| `Thread` | conversa: `cwd` (checkout isolado ou a pasta), `branch`/`base_ref`, `session_id`, provedor, modelo, modo, `seen` (impressões digitais dos arquivos revisados) |
| `ThreadView` | a thread + o `Status` ao vivo (`idle`/`running`/`awaiting`/`error`) |
| `Settings` | padrões das threads e `prefs` (chave → JSON) para a UI e o git |
| `transcript::Transcript` | a conversa montada do stream: `User`, `Text`, `Thinking`, `Tool`, `Permission`, `Result`, `Error`, `Notice` |
| `tree` | os grupos e linhas da barra lateral (por projeto ou por data) |
| `graph` | as lanes do grafo de commits |
| `diff` | linhas de um patch com a numeração dos dois lados |

O estado é gravado de forma **atômica** (arquivo irmão + rename). Um
`state.json` corrompido é guardado ao lado (`state.json.corrupt-<ts>`) e o
app abre vazio, em vez de falhar.

As preferências da interface (`ui.appearance`, `ui.openWorkspaces`, atalhos…)
ficam em `settings.prefs`: a versão web, o desktop e a CLI veem as mesmas.

## A transcrição

O mesmo reducer monta a conversa ao vivo (stream-json do Claude Code, ou o
ACP traduzido) e a do histórico (o `.jsonl` da sessão, que é o mesmo stream
sem os eventos parciais). Mensagens de subagentes (`parent_tool_use_id`) não
entram; blocos que o CLI injeta (`<system-reminder>`) saem do texto do
usuário; pedidos de permissão pendentes viram "cancelados" no fim do turno.
Resultados de ferramenta acima de 256 KiB são cortados e a transcrição guarda
no máximo 20 000 itens.

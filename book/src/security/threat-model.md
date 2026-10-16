# Modelo de ameaças

## O que protegemos

- O **código** dos seus repositórios (o agente só escreve onde a thread
  trabalha; threads isoladas não tocam o checkout principal até você integrar).
- As **credenciais**: a conta Claude (o app não lê os tokens, só aponta o
  `CLAUDE_CONFIG_DIR`), chaves SSH, credenciais HTTPS do git, tokens do
  GitHub/GitLab e o token do canal de controle.
- A **disponibilidade** do app: uma linha ruim de um agente ou um arquivo de
  sessão estranho não pode travar ou derrubar a janela com trabalho aberto.

## Quem ataca (e como)

| Ator | Vetor | Resposta |
|---|---|---|
| Conteúdo malicioso lido pelo agente (README, issue, página web) que tenta injetar instruções | o agente decide rodar algo perigoso | o pedido de permissão chega à UI com a ferramenta e a entrada; o modo `bypassPermissions` e o "pular permissões" ficam visíveis no rodapé |
| Um agente (ou binário trocado) que manda saída malformada | linhas gigantes, JSON profundo, UTF-8 quebrado, tipos errados | [parsers com teto](parser-hardening.md) e reducer sem pânico (fuzzing) |
| Outro processo local | conectar no canal de controle | loopback + token de 244 bits aleatórios em arquivo 0600, comparação em tempo constante |
| Uma página web aberta no navegador | falar com o servidor da versão web | `Origin` precisa ser igual ao `Host`; token no fragmento da URL |
| Um repositório hostil | hooks do git, nomes de arquivo, diffs enormes | hooks são os do repositório (como no `git`); diffs com teto; caminhos tratados como dados |

## Fora do escopo

- Um **agente** com permissão concedida faz o que a permissão deixa — o app
  não é sandbox. Use threads isoladas, o modo `plan` e revise antes de integrar.
- Alguém com acesso à sua conta de usuário (lê o `state.json`, o token, os
  arquivos de sessão).
- Os próprios agentes e o `git` (atualize-os; o app usa versões fixas só
  quando roda Codex/Gemini via `npx`).

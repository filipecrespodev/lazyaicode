# Git e threads isoladas

## Threads isoladas

`lazyaicode_git::thread` usa o `git` da máquina para o fluxo das threads:

- `create_isolated`: `git worktree add -b lazy/<id> <dados>/worktrees/<projeto>/<id> <upstream>`
  — o checkout principal não muda de branch nem de arquivos.
- `snapshot_tree`: uma árvore do estado atual da pasta (`git add -A` e
  `git write-tree` numa cópia temporária do índice, sem tocar no staging do
  usuário), usada como base do diff "último turno".
- `changes`: o diff da base escolhida até a foto atual, separado por
  arquivo, cada um com uma impressão digital (para a marca de "revisado").
- `apply_into` / `merge_into`: integrar no checkout principal.
- Arquivar remove o checkout e mantém a branch; desarquivar o recria.

## O cliente git

A aba **Histórico** e as vizinhas usam `lazyaicode_git::git`, sobre
**libgit2** (vendored), com um **ator por repositório** (`actor.rs`): uma
thread dona do `git2::Repository` que atende uma fila de comandos. Lá estão
log com grafo, staging por arquivo e por linha (`partial.rs`), commit com
hooks e assinatura (SSH/GPG), branches, tags, stash, merge, rebase e rebase
interativo com resolução de conflitos, cherry-pick, revert, reset, reflog,
blame, histórico de arquivo, comparação de branches, worktrees, submódulos,
LFS, desfazer/refazer da última ação, estatísticas e as integrações com
GitHub e GitLab (PRs e MRs). São ~110 comandos `git.*`, gerados das funções
de `crates/engine/src/git_cmds/`.

Fetch, pull e push autenticam com o agente SSH, chaves configuradas
(`sshKeys`), credenciais HTTPS guardadas ou pedidas à UI (eventos
`ssh-passphrase-request` / `https-credentials-request`); operações em
segundo plano nunca pedem senha.

## WSL

No Windows, repositórios dentro de uma distro WSL (`\\wsl.localhost\<distro>\…`)
não são abertos pelo 9P (lento): o app instala e conversa com o
`lazyaicode-agent` dentro da distro (`apps/lazyaicode-wsl-agent`), que roda o
mesmo crate git e fala `lazyaicode_git::proto` — quadros JSON com prefixo de
tamanho no stdio.

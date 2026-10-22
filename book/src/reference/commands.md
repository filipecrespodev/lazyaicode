# Referência de comandos

> Gerado por `cargo xtask commands` — não edite à mão.

São **528** comandos: 250 do motor (rodam na CLI, no canal de controle, no MCP e na UI) e 278 de tela (`ui.*`, só no app aberto, pelo `ui.run` do canal de controle). Parâmetros com `?` são opcionais.

## Motor

### `account`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `account.list` 🔎 | Claude accounts — Diretórios de configuração do Claude Code encontrados. |  |  |
| `account.status` 🔎 | Account login — Login de uma conta (diretório Claude); sem `claudeDir`, a ativa (`$CLAUDE_CONFIG_DIR` ou `~/.claude`). | `claudeDir`? |  |

### `ai`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `ai.commit-message` | Ai generate commit message | `model`, `diff`, `customInstructions` |  |
| `ai.compose-commits` | Ai compose commits | `repoId`, `model`, `customInstructions`, `answers` |  |
| `ai.resolve-conflict` | Ai resolve conflict | `model`, `path`, `ancestor`?, `ours`, `theirs`, `customInstructions` |  |
| `ai.pr-description` | Ai generate pr description | `model`, `commits`, `diff` |  |

### `capabilities`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `capabilities.get` 🔎 | Skills, agents and MCP — O que o Claude Code carrega numa pasta com a conta do workspace: skills e comandos, agentes, MCPs e modelos. | `workspaceId`, `path`?, `force`? |  |
| `capabilities.trash` ⚠ | Move skill/agent to trash — Move um skill (a pasta inteira) ou um agente/comando para a lixeira. Só itens da conta ou do `.claude/` de um projeto, nunca de plugins. | `workspaceId`, `path` |  |
| `capabilities.skills` 🔎 | Skills and commands | `workspaceId`, `path`? |  |
| `capabilities.agents` 🔎 | Agents | `workspaceId`, `path`? |  |
| `capabilities.mcp` 🔎 | MCP servers | `workspaceId`, `path`? |  |
| `capabilities.models` 🔎 | Models | `workspaceId`, `path`? |  |
| `capabilities.refresh` | Forget cached capabilities |  |  |

### `engine`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `engine.ping` 🔎 | Ping |  |  |
| `engine.version` 🔎 | Version |  |  |
| `engine.commands` 🔎 | List commands — Todos os comandos que o motor executa, com parâmetros. É o ponto de partida de um agente. | `filter`?, `group`? |  |
| `engine.describe` 🔎 | Describe a command | `id` |  |
| `engine.groups` 🔎 | Command groups |  |  |
| `engine.events` 🔎 | Event names |  |  |
| `engine.journal` 🔎 | Session journal — Os últimos comandos que mudaram algo nesta sessão, com resultado e duração (segredos ocultos). | `last`? |  |
| `engine.state` 🔎 | Session state — O estado inteiro (workspaces, projetos, threads com status, preferências). |  |  |
| `engine.batch` | Run several commands — Executa comandos em sequência e devolve um resultado por passo. | `steps`, `stopOnError`? |  |
| `engine.models` 🔎 | Claude Code models |  |  |
| `engine.permission-modes` 🔎 | Permission modes |  |  |
| `engine.efforts` 🔎 | Effort levels |  |  |
| `engine.limits` 🔎 | Protocol limits |  |  |
| `engine.journal-clear` | Clear the journal |  |  |

### `git`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `git.path.status` 🔎 | Status (git status) — Saída de `git status --porcelain -b` na pasta. | `path` |  |
| `git.path.log` 🔎 | Recent commits — Os 30 últimos commits (`git log --oneline`). | `path` |  |
| `git.path.diff` 🔎 | Unstaged diff — Diff do que não está no staging. | `path` |  |
| `git.path.diff-staged` 🔎 | Staged diff — Diff do que está no staging. | `path` |  |
| `git.path.branches` 🔎 | Branches — Todas as branches (locais e remotas). | `path` |  |
| `git.path.current-branch` 🔎 | Current branch — Nome da branch atual. | `path` |  |
| `git.path.head` 🔎 | HEAD commit — SHA do HEAD. | `path` |  |
| `git.path.remotes` 🔎 | Remotes — Remotos configurados. | `path` |  |
| `git.path.stash-list` 🔎 | Stashes — Entradas do stash. | `path` |  |
| `git.path.tags` 🔎 | Tags — Tags, das mais novas. | `path` |  |
| `git.path.root` 🔎 | Repository root — Raiz do repositório que contém a pasta. | `path` |  |
| `git.path.shortstat` 🔎 | Change summary — Resumo das mudanças desde o HEAD. | `path` |  |
| `git.path.is-repo` 🔎 | Is a git repository? | `path` |  |
| `git.path.commit-all` | Commit everything — `git add -A` e commit na pasta. | `path`, `message` |  |
| `git.working.apply-composed` | Apply composed commits | `repoId`, `treeId`, `commits`, `authorName`?, `authorEmail`?, `noVerify`? |  |
| `git.branch.list` 🔎 | List branches | `repoId` |  |
| `git.tag.list` 🔎 | List tags | `repoId` |  |
| `git.branch.create` | Create branch | `repoId`, `name`, `startPoint`?, `checkout` |  |
| `git.branch.checkout` | Checkout branch | `repoId`, `name` |  |
| `git.branch.checkout-at` | Checkout at | `repoId`, `name`, `targetSha`, `remoteName`? |  |
| `git.branch.checkout-detached` | Checkout commit detached | `repoId`, `sha` |  |
| `git.commit.cherry-pick` | Cherry pick | `repoId`, `sha`, `createCommit` |  |
| `git.reset.to-commit` ⚠ | Reset to commit | `repoId`, `sha`, `hard` |  |
| `git.reset.to-ref` ⚠ | Reset to ref | `repoId`, `refName`, `hard` |  |
| `git.branch.sync-to-remote` ⚠ | Sync branch to remote | `repoId`, `targetSha` |  |
| `git.branch.delete-local` ⚠ | Delete local branch | `repoId`, `name` |  |
| `git.branch.delete-remote` ⚠ | Delete remote branch | `repoId`, `remoteName`, `name` |  |
| `git.branch.delete-both` ⚠ | Delete branch both | `repoId`, `remoteNames`, `name` |  |
| `git.branch.merge` | Merge branch | `repoId`, `name` |  |
| `git.tag.create` | Create tag | `repoId`, `name`, `sha` |  |
| `git.tag.delete-local` ⚠ | Delete local tag | `repoId`, `name` |  |
| `git.tag.push` | Push tag | `repoId`, `remoteName`, `name` |  |
| `git.tag.delete-remote` ⚠ | Delete remote tag | `repoId`, `remoteName`, `name` |  |
| `git.tag.list-remote` 🔎 | List remote tags | `repoId`, `remoteName` |  |
| `git.config.identity` 🔎 | Get global git identity — Reads the machine's global git identity (`user.name`/`user.email`) — used to seed the "Default" commit profile the first time the app runs. |  |  |
| `git.config.editors` 🔎 | List installed editors — Editors found on this machine, for the default-editor setting. |  |  |
| `git.config.repo-file-path` 🔎 | Repo file path — Absolute path of `rel_path` inside the repo's working directory. | `repoId`, `relPath` |  |
| `git.config.open-in-editor` | Open in editor — Opens `path` with the editor launcher `command` (from `list_installed_editors`). | `command`, `path` |  |
| `git.diff.commit` 🔎 | Get commit diff | `repoId`, `sha` |  |
| `git.diff.range` 🔎 | Get range diff | `repoId`, `fromSha`, `toSha` |  |
| `git.diff.file-content` 🔎 | Get file content | `repoId`, `sha`?, `path` |  |
| `git.commit.revert` | Revert commit | `repoId`, `sha`, `mainline`? |  |
| `git.history.file` 🔎 | Get file history | `repoId`, `query` |  |
| `git.history.commit-file-diff` 🔎 | Get commit file diff | `repoId`, `sha`, `path`, `oldPath`? |  |
| `git.history.blame` 🔎 | Get blame | `repoId`, `path`, `sha`?, `startLine`, `maxLines` |  |
| `git.history.compare` 🔎 | Get branch compare | `repoId`, `a`, `b`, `limit` |  |
| `git.history.tree-diff` 🔎 | Get tree diff | `repoId`, `fromSha`, `toSha` |  |
| `git.irebase.plan` 🔎 | Get rebase plan — Commits an interactive rebase starting at `from_sha` would rewrite. | `repoId`, `fromSha` |  |
| `git.irebase.start` | Start interactive rebase | `repoId`, `upstream`?, `items` |  |
| `git.log.list` 🔎 | Get commit log | `repoId`, `query` |  |
| `git.log.head` 🔎 | Get head commit | `repoId` |  |
| `git.log.signature` 🔎 | Get commit signature — Whether a commit is signed (and how), for the commit details panel. Asked for the selected commit only, never per graph row. | `repoId`, `sha` |  |
| `git.log.stats` 🔎 | Get repo stats | `repoId` |  |
| `git.reflog.list` 🔎 | Get reflog | `repoId`, `refName` |  |
| `git.log.search` 🔎 | Search commits | `repoId`, `query`, `mode` |  |
| `git.repo.fingerprint` 🔎 | Get repo fingerprint — Token that changes when refs/HEAD/stash move (see `git::fingerprint`). Read straight from disk, not through the repo actor: it must answer even while the actor is busy. | `repoId` |  |
| `git.pull.pull` | Pull branch — `strategy`: overrides the repo's pull config for this one pull. | `repoId`, `strategy`? |  |
| `git.pull.upstream-status` 🔎 | Get upstream status | `repoId` |  |
| `git.rebase.start` | Start rebase | `repoId`, `ontoBranch` |  |
| `git.rebase.status` 🔎 | Get rebase status | `repoId` |  |
| `git.rebase.resolve-conflict` | Resolve rebase conflict | `repoId`, `path`, `resolution` |  |
| `git.rebase.continue` | Continue rebase | `repoId` |  |
| `git.rebase.abort` ⚠ | Abort rebase | `repoId` |  |
| `git.remote.fetch` | Fetch remote — `remote_name`: `None` fetches every remote. `background`: a fetch the user didn't ask for (on open, periodic), which must not prompt for an SSH key passphrase or HTTPS credentials. | `repoId`, `remoteName`?, `background`? |  |
| `git.remote.url` 🔎 | Get remote url | `repoId`, `remoteName` |  |
| `git.remote.push` | Push branch — `remote_name`: `None` pushes where `git push` would (upstream, else `origin`, else the only remote), failing with `RemoteRequired` when that's ambiguous so the UI can ask. | `repoId`, `remoteName`?, `branch`, `force` |  |
| `git.remote.list` 🔎 | List remotes | `repoId` |  |
| `git.remote.add` | Add remote | `repoId`, `name`, `url` |  |
| `git.remote.remove` ⚠ | Remove remote | `repoId`, `name` |  |
| `git.remote.rename` | Rename remote | `repoId`, `name`, `newName` |  |
| `git.remote.set-url` | Set remote url | `repoId`, `name`, `url` |  |
| `git.repo.open` | Open repository — A `\\wsl.localhost\<distro>\...` path opens through that distro's agent (see `git_client::wsl`); anything else in this process. | `path` |  |
| `git.repo.status` 🔎 | Get repo status | `repoId` |  |
| `git.repo.close` | Close repository | `repoId` |  |
| `git.worktree.list` 🔎 | List worktrees | `repoId` |  |
| `git.worktree.add` | Add worktree | `repoId`, `args` |  |
| `git.worktree.remove` ⚠ | Remove worktree | `repoId`, `name`, `force` |  |
| `git.worktree.prune` | Prune worktrees | `repoId` |  |
| `git.submodule.list` 🔎 | List submodules | `repoId` |  |
| `git.submodule.count` 🔎 | Get submodule count — Decides whether the repo menu offers Submodules at all; cheap enough to ask on every repo open, unlike `list_submodules`. | `repoId` |  |
| `git.submodule.update` | Update submodules — `git submodule update --init` for `paths` (every submodule when empty). | `repoId`, `paths` |  |
| `git.lfs.status` 🔎 | Get lfs status — Off the actor: the first call may spawn `git lfs version`. | `repoId` |  |
| `git.lfs.pull` | Run lfs pull — Runs off the actor so a long download doesn't hold up every other command for this repo; git-lfs does its own locking. | `repoId` |  |
| `git.stash.save` | Stash save | `repoId`, `message`?, `includeUntracked` |  |
| `git.stash.list` | Stash list | `repoId` |  |
| `git.stash.pop` | Stash pop | `repoId`, `index` |  |
| `git.stash.apply` | Stash apply | `repoId`, `index` |  |
| `git.stash.drop` ⚠ | Stash drop | `repoId`, `index` |  |
| `git.undo.undo` | Undo last action — Reverts the last local action; returns its label. | `repoId` |  |
| `git.undo.redo` | Redo last action — Re-applies the last undone action; returns its label. | `repoId` |  |
| `git.undo.status` 🔎 | Get undo status | `repoId` |  |
| `git.working.status` 🔎 | Get working status | `repoId` |  |
| `git.working.stage-path` | Stage path | `repoId`, `path` |  |
| `git.working.stage-all` | Stage all | `repoId` |  |
| `git.working.unstage-path` | Unstage path | `repoId`, `path` |  |
| `git.working.unstage-all` | Unstage all | `repoId` |  |
| `git.working.discard-path` ⚠ | Discard path | `repoId`, `path` |  |
| `git.working.discard-all` ⚠ | Discard all | `repoId` |  |
| `git.working.apply-lines` | Apply lines — Stages, unstages or discards only the selected changed lines of `path` (a hunk is just all of its lines). | `repoId`, `path`, `action`, `selection` |  |
| `git.working.ignore` | Add ignore pattern — Appends `pattern` to `.gitignore`, or to `.git/info/exclude` when `local`. Resolves to `false` when the pattern was already there. | `repoId`, `pattern`, `local` |  |
| `git.working.diff` 🔎 | Get workdir diff | `repoId`, `staged` |  |
| `git.working.commit` | Git commit changes | `repoId`, `summary`, `body`, `authorName`?, `authorEmail`?, `amend`, `noVerify`? |  |
| `git.working.amend-message` | Amend commit message | `repoId`, `sha`, `summary`, `body`, `noVerify`? |  |
| `git.ssh.list-keys` 🔎 | List ssh keys — Keys in `~/.ssh` of this machine, or of `distro` when given. | `distro`? |  |
| `git.ssh.submit-passphrase` | Submit ssh passphrase — Answers the passphrase prompt, wherever it came from (this process or an agent: the one that asked is not tracked). | `passphrase`? |  |
| `git.ssh.pending-passphrase` 🔎 | Pending ssh passphrase |  |  |
| `git.https.submit-credentials` | Submit https credentials | `username`?, `password`? |  |
| `git.https.pending-credentials` 🔎 | Pending https credentials |  |  |

### `github`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `github.pr.list` | Github list open pull requests | `baseUrl`, `token`, `projectPath` |  |
| `github.pr.create` | Github create pull request | `baseUrl`, `token`, `projectPath`, `sourceBranch`, `targetBranch`, `title`, `description`, `assignee`?, `reviewers` |  |
| `github.assignees` | Github list assignees | `baseUrl`, `token`, `projectPath` |  |
| `github.events` | Github list repo events | `baseUrl`, `token`, `projectPath` |  |

### `gitlab`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `gitlab.project` | Gitlab get project | `baseUrl`, `token`, `projectPath` |  |
| `gitlab.mr.list` | Gitlab list open merge requests | `baseUrl`, `token`, `projectPath` |  |
| `gitlab.mr.create` | Gitlab create merge request | `baseUrl`, `token`, `projectPath`, `sourceBranch`, `targetBranch`, `title`, `description`, `assigneeId`?, `reviewerIds`, `removeSourceBranch`, `squash` |  |
| `gitlab.members` | Gitlab search members | `baseUrl`, `token`, `projectPath` |  |
| `gitlab.events` | Gitlab list project events | `baseUrl`, `token`, `projectPath` |  |

### `project`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `project.list` 🔎 | Projects — Repositórios do workspace (menos os ignorados) mais qualquer projeto onde uma thread dele esteja. | `workspaceId` |  |
| `project.get` 🔎 | Project | `id` |  |
| `project.threads` 🔎 | Threads of a project | `id` |  |
| `project.open-folder` | Open project folder | `id` |  |

### `provider`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `provider.status` 🔎 | Provider status — Situação do Codex (OpenAI) ou do Gemini CLI (Google) na máquina: instalado, login, versão, modos e modelos. | `provider`, `force`? |  |
| `provider.auth-methods` 🔎 | Provider login methods | `provider` |  |
| `provider.login` | Provider login — Login pelo próprio agente (abre o navegador). | `provider`, `method` |  |
| `provider.list` 🔎 | Providers |  |  |
| `provider.forget` | Forget provider status |  |  |

### `review`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `thread.diff-stats` 🔎 | Diff summary — Linhas adicionadas/removidas desde a base da thread. | `id` |  |
| `thread.changes` 🔎 | Changes for review — Arquivos mudados desde a base (`upstream` = de onde a thread partiu, `turn` = início do último turno, `head` = último commit). | `id`, `base` |  |
| `thread.set-seen` | Mark files as reviewed | `id`, `entries` |  |
| `thread.branch-info` 🔎 | Branch status | `id` |  |
| `thread.land` | Bring changes into the project — `apply` copia as mudanças como não commitadas no checkout principal; `merge` faz merge da branch da thread (precisa de commit). | `id`, `mode` |  |
| `thread.commit` | Commit in the thread checkout — `git add -A` e commit no checkout da thread (antes de um merge). | `id`, `message` |  |
| `thread.push` | Push the thread branch — Push da branch da thread (com upstream). | `id`, `remote`? |  |
| `thread.discard-file` ⚠ | Revert a file to the thread base — Volta um arquivo do checkout da thread ao que era na base dela (`git checkout <base> -- <arquivo>`; arquivos novos são removidos). | `id`, `path` |  |
| `thread.file-diff` 🔎 | Diff of one file | `id`, `path`, `base`? |  |
| `thread.update-from-base` | Update the thread with its base — Traz para a branch da thread o que entrou na base desde que ela nasceu (`git merge <base>` no checkout). | `id` |  |

### `settings`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `settings.get` 🔎 | Settings — Preferências, binário do Claude encontrado e pastas do app. |  |  |
| `settings.save` | Save settings | `settings` |  |
| `settings.default-model` | Set default model — Muda uma preferência. | `value` |  |
| `settings.default-permission-mode` | Set default permission mode — Muda uma preferência. | `value` |  |
| `settings.send-with-modifier` | Set send with modifier — Muda uma preferência. | `value` |  |
| `settings.notify-on-finish` | Set notify on finish — Muda uma preferência. | `value` |  |
| `settings.sound-on-finish` | Set sound on finish — Muda uma preferência. | `value` |  |
| `settings.default-provider` | Set default provider — Muda uma preferência. | `value` |  |
| `settings.default-skip-permissions` | Set default skip permissions — Muda uma preferência. | `value` |  |
| `settings.language` | Set language — Muda uma preferência. | `value` |  |
| `settings.check-updates` | Set check updates — Muda uma preferência. | `value` |  |
| `prefs.get` 🔎 | Read a preference — Preferência da interface ou do cliente git (`theme`, `sshKeys`, `signCommits`…); sem `key`, todas. | `key`? |  |
| `prefs.set` | Change a preference — Grava uma preferência; `value: null` apaga. | `key`, `value`? |  |
| `prefs.list` 🔎 | Preference keys |  |  |
| `prefs.reset` ⚠ | Reset preferences — Apaga as preferências (todas, ou as que começam com `prefix`). | `prefix`? |  |
| `settings.reset` ⚠ | Reset settings — Volta as preferências das threads ao padrão (mantém as da interface). |  |  |
| `settings.export` 🔎 | Export settings |  |  |

### `system`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `system.user-profile` 🔎 | Git profile — Nome e e-mail do git (global) e o avatar do Gravatar. |  |  |
| `system.open-path` | Open in file manager — Abre um arquivo ou pasta no programa padrão do sistema. | `path` |  |
| `system.read-image` 🔎 | Read image file — Bytes de uma imagem do disco para anexar no chat (até 5 MB; PNG, JPEG, GIF ou WebP). | `path` |  |
| `system.health` 🔎 | Health check — O que o app precisa na máquina: Claude Code, git, Node (para Codex/Gemini via npx) e a pasta de dados. |  |  |
| `system.which` 🔎 | Find a program | `name` |  |
| `system.data-dir` 🔎 | Data folder |  |  |
| `system.home` 🔎 | Home folder |  |  |
| `system.check-update` 🔎 | Check for a new version — Consulta o último release publicado e diz se é mais novo que esta versão (`LAZYAICODE_UPDATE_URL` troca o endereço). |  |  |
| `system.avatar` 🔎 | Author photo — A foto do Gravatar de um e-mail (`{"image": base64}`), ou `null` se não houver. Guarda a resposta enquanto o app roda; `LAZYAICODE_GRAVATAR_URL` troca o endereço (vazio desliga). | `email` |  |
| `system.open-url` | Open a web page — Abre um endereço http(s) no navegador padrão. | `url` |  |
| `system.session-file` 🔎 | Claude session file — Onde está o `.jsonl` da sessão do Claude Code de uma thread. | `id` |  |
| `system.session-raw` 🔎 | Raw session messages — As últimas mensagens brutas da sessão (para depuração). | `id`, `last`? |  |
| `system.fonts` 🔎 | List system fonts — Font families installed on this machine, for the Appearance font pickers: `fc-list` on Linux/macOS, WPF's font list through PowerShell on Windows. |  |  |

### `terminal`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `terminal.spawn` | Open terminal — Abre um shell num PTY. A saída chega pelos eventos `term-data` (base64). | `termId`, `cwd`, `cols`, `rows` |  |
| `terminal.write` | Type into terminal | `termId`, `data` |  |
| `terminal.resize` | Resize terminal | `termId`, `cols`, `rows` |  |
| `terminal.kill` | Close terminal | `termId` |  |
| `terminal.list` 🔎 | Open terminals |  |  |
| `terminal.interrupt` | Send Ctrl+C | `termId` |  |
| `terminal.eof` | Send Ctrl+D | `termId` |  |
| `terminal.clear` | Clear terminal | `termId` |  |
| `terminal.tab` | Send Tab | `termId` |  |
| `terminal.enter` | Send Enter | `termId` |  |
| `terminal.run` | Run a command line — Digita a linha e aperta Enter. | `termId`, `command` |  |

### `thread`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `thread.list` 🔎 | Threads | `workspaceId` |  |
| `thread.get` 🔎 | Thread | `id` |  |
| `thread.sync-sessions` | Import Claude Code sessions — Traz as sessões do Claude Code da conta do workspace, que rodaram dentro da pasta dele, como threads. Devolve quantas surgiram. | `workspaceId` |  |
| `thread.create` | New thread | `workspaceId`, `projectId`, `isolated`, `model`?, `permissionMode`?, `provider`?, `skipPermissions`? | `Ctrl+N` |
| `thread.update` | Rename / favorite thread | `id`, `title`?, `bookmarked`?, `unread`? |  |
| `thread.archive` | Archive thread — Arquiva (o checkout isolado sai do disco; a branch fica) ou desarquiva (recria o checkout a partir da branch). | `id`, `archived` |  |
| `thread.delete` ⚠ | Delete thread — Apaga a thread; numa isolada, também o checkout e a branch dela. | `id` |  |
| `thread.transcript` 🔎 | Thread history | `id` |  |
| `thread.send` | Send message — Manda uma mensagem ao agente da thread, abrindo o processo se preciso. A resposta chega pelos eventos `agent-event`. | `id`, `text`, `images`? |  |
| `thread.interrupt` | Stop the agent — Interrompe o turno em curso. | `id` | `Escape` |
| `thread.respond-permission` | Answer a permission request | `id`, `requestId`, `allow`, `input`?, `message`?, `toolUseId`?, `updatedPermissions`? |  |
| `thread.pending-permission` 🔎 | Pending permission request — O pedido de permissão que o agente da thread espera responder (status `awaiting`), para quem não acompanha os eventos: responda com `thread.respond-permission`. | `id` |  |
| `thread.set-model` | Change model | `id`, `model` |  |
| `thread.set-permission-mode` | Change permission mode | `id`, `mode` | `Shift+Tab` |
| `thread.set-options` | Session options — Opções que só valem ao abrir o processo (esforço, agente, pastas extras). Com o agente aberto e parado, ele é encerrado: o próximo turno retoma a mesma conversa já com elas. `""` limpa. | `id`, `effort`?, `agent`?, `addDirs`?, `skipPermissions`? |  |
| `thread.set-provider` | Change provider — Troca o provedor de uma thread que ainda não conversou. | `id`, `provider`, `model`? |  |
| `thread.mcp` | Session MCP servers — MCP da sessão aberta: status, ligar/desligar (só nesta sessão, como o `/mcp`) e reconectar. | `id`, `action`, `server`?, `enabled`? |  |
| `thread.rename` | Rename thread | `id`, `title` |  |
| `thread.bookmark` | Add to favorites | `id` |  |
| `thread.unbookmark` | Remove from favorites | `id` |  |
| `thread.mark-read` | Mark as read | `id` |  |
| `thread.mark-unread` | Mark as unread | `id` |  |
| `thread.unarchive` | Unarchive thread | `id` |  |
| `thread.status` 🔎 | Thread status | `id` |  |
| `thread.wait` 🔎 | Wait for the agent — Espera a thread sair de `running` (fim do turno ou pedido de permissão). | `id`, `timeoutMs`? |  |
| `thread.export` 🔎 | Export conversation — A conversa inteira em Markdown (padrão) ou como a lista de itens em JSON. | `id`, `format`? |  |
| `thread.last-message` 🔎 | Last agent message | `id` |  |
| `thread.search` 🔎 | Search threads — Threads cujo título ou pasta contém o texto. | `query`, `workspaceId`?, `archived`? |  |
| `thread.duplicate` | Duplicate thread — Uma thread nova no mesmo projeto, com o mesmo provedor, modelo e modo (conversa do zero). | `id`, `isolated`? |  |
| `thread.activity` 🔎 | Threads needing attention |  |  |
| `thread.count` 🔎 | Thread counts | `workspaceId`? |  |

### `workspace`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `workspace.list` 🔎 | Workspaces |  |  |
| `workspace.create` | New workspace — Cria um workspace a partir de uma pasta e o torna o ativo. | `path`, `name`?, `claudeDir`? |  |
| `workspace.update` | Edit workspace — Renomeia, troca a conta (`claudeDir: ""` = ~/.claude) ou a lista de pastas ignoradas. | `id`, `name`?, `claudeDir`?, `ignoredPaths`? |  |
| `workspace.remove` ⚠ | Remove workspace — Tira o workspace do app (as pastas e as branches das threads ficam). | `id` |  |
| `workspace.activate` | Switch workspace | `id` |  |
| `workspace.repos` 🔎 | Workspace folders — Todas as pastas do workspace, inclusive as ignoradas, marcando as que são repositórios. | `id` |  |
| `workspace.get` 🔎 | Workspace | `id` |  |
| `workspace.active` 🔎 | Active workspace |  |  |
| `workspace.rename` | Rename workspace | `id`, `name` |  |
| `workspace.set-account` | Change workspace account | `id`, `claudeDir`? |  |
| `workspace.ignore` | Hide a folder | `id`, `path` |  |
| `workspace.unignore` | Show a hidden folder | `id`, `path` |  |

### `wsl`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `wsl.distros` 🔎 | List wsl distros — Installed WSL distros (empty off Windows). |  |  |
| `wsl.home` 🔎 | Wsl home — The distro's home folder, as a UNC path: where the WSL folder picker starts. | `distro` |  |
| `wsl.dirs` 🔎 | List wsl dirs — Subfolders of a WSL UNC path, for the in-app folder picker (the native dialog crawls over 9P). | `path` |  |

## Tela

### `appearance`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `ui.theme.system` | System |  |  |
| `ui.theme.dark` | LazyAICode Dark |  |  |
| `ui.theme.tokyo` | Tokyo Night |  |  |
| `ui.theme.dracula` | Dracula |  |  |
| `ui.theme.one-dark` | One Dark |  |  |
| `ui.theme.nord` | Nord |  |  |
| `ui.theme.gruvbox` | Gruvbox Dark |  |  |
| `ui.theme.solarized` | Solarized Dark |  |  |
| `ui.theme.catppuccin` | Catppuccin Mocha |  |  |
| `ui.theme.rose-pine` | Rosé Pine |  |  |
| `ui.theme.light` | LazyAICode Light |  |  |
| `ui.theme.catppuccin-latte` | Catppuccin Latte |  |  |
| `ui.theme.rose-pine-dawn` | Rosé Pine Dawn |  |  |
| `ui.theme.solarized-light` | Solarized Light |  |  |
| `ui.theme.next` | Next theme |  |  |
| `ui.density.compact` | Compact |  |  |
| `ui.density.normal` | Normal |  |  |
| `ui.density.comfortable` | Comfortable |  |  |
| `ui.width.narrow` | Narrow |  |  |
| `ui.width.normal` | Normal |  |  |
| `ui.width.wide` | Wide |  |  |
| `ui.width.full` | Full screen |  |  |
| `ui.zoom.in` | Zoom in |  | `Ctrl+Plus` |
| `ui.zoom.in-alt` | Zoom in |  | `Ctrl+Equals` |
| `ui.zoom.out` | Zoom out |  | `Ctrl+Minus` |
| `ui.zoom.reset` | Reset zoom |  | `Ctrl+0` |
| `ui.font.text.bigger` | Text font bigger |  |  |
| `ui.font.text.smaller` | Text font smaller |  |  |
| `ui.font.code.bigger` | Code font bigger |  |  |
| `ui.font.code.smaller` | Code font smaller |  |  |
| `ui.font.terminal.bigger` | Terminal font bigger |  |  |
| `ui.font.terminal.smaller` | Terminal font smaller |  |  |
| `ui.appearance.reset` | Reset appearance |  |  |
| `ui.language.en` | English |  |  |
| `ui.language.pt` | Português |  |  |
| `ui.language.es` | Español |  |  |
| `ui.language.zh` | 中文 |  |  |
| `ui.density.toggle` | Toggle compact density |  |  |
| `ui.language.next` | Next language |  |  |
| `ui.zoom.set` | Set zoom | `percent` |  |

### `git`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `ui.git.blame` | Blame | `path` |  |
| `ui.git.file-history` | File history | `path` |  |
| `ui.git.compare` | Compare | `a`?, `b` |  |
| `ui.git.open-pr` | New pull request |  |  |
| `ui.git.interactive-rebase` | Interactive rebase | `sha`? |  |
| `ui.git.search` | Search commits | `query`? | `Ctrl+Shift+H` |
| `ui.git.worktrees` | Worktrees |  |  |
| `ui.git.remotes` | Remotes |  |  |
| `ui.git.submodules` | Submodules |  |  |
| `ui.git.conflicts.refresh` | Reload conflicts |  |  |
| `ui.git.lfs-pull` | Pull LFS files |  |  |
| `ui.git.refresh` | Refresh |  |  |
| `ui.git.fetch` | Fetch |  | `Ctrl+Shift+F` |
| `ui.git.pull` | Pull |  | `Ctrl+Shift+L` |
| `ui.git.push` | Push |  | `Ctrl+Shift+U` |
| `ui.git.force-push` | Force push |  |  |
| `ui.git.stage-all` | stage all |  |  |
| `ui.git.unstage-all` | unstage all |  |  |
| `ui.git.undo` | Undo |  |  |
| `ui.git.redo` | Redo |  |  |
| `ui.git.rebase-continue` | Continue |  |  |
| `ui.git.rebase-abort` | Abort |  |  |
| `ui.git.worktree-prune` | Clean up missing |  |  |
| `ui.git.discard-all` | discard all |  |  |
| `ui.git.commit` | Commit | `summary`?, `body`? |  |
| `ui.git.amend.toggle` | Amend commit |  |  |
| `ui.git.ai-message` | Write the message with AI |  | `Ctrl+Shift+M` |
| `ui.git.stash` | Stash | `message`? |  |
| `ui.git.stash-pop` | Pop stash | `index`? |  |
| `ui.git.stash-apply` | Apply | `index`? |  |
| `ui.git.stash-drop` | Drop | `index`? |  |
| `ui.git.new-branch` | New branch | `name`? | `Ctrl+Shift+B` |
| `ui.git.checkout` | Check out | `name` |  |
| `ui.git.merge` | Merge into current | `name` |  |
| `ui.git.rebase-onto` | Rebase {other} onto {target} | `name` |  |
| `ui.git.delete-branch` | Delete branch | `name` |  |
| `ui.git.select-working` | Local changes |  |  |
| `ui.git.select-commit` | Select commit | `sha` |  |
| `ui.git.load-more` | Load more commits |  |  |
| `ui.git.toggle-refs` | Branches panel |  |  |
| `ui.git.commit.copy-sha` | Copy commit SHA | `sha`? |  |
| `ui.git.commit.checkout` | Check out this commit | `sha`? |  |
| `ui.git.commit.cherry-pick` | Cherry-pick | `sha`? |  |
| `ui.git.commit.revert` | Revert | `sha`? |  |
| `ui.git.commit.reset-mixed` | Reset here (keep changes) | `sha`? |  |
| `ui.git.commit.reset-hard` | Hard (discards all changes) | `sha`? |  |
| `ui.git.commit.branch-here` | Create branch here | `sha`? |  |
| `ui.git.commit.tag-here` | Create tag here | `sha`? |  |

### `help`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `ui.help.shortcuts` | Keyboard shortcuts |  | `F1` |
| `ui.help.about` | About LazyAICode |  |  |
| `ui.notifications.test` | Test a notification |  |  |
| `ui.debug.reload-commands` | Reload the command list |  |  |
| `ui.debug.reload-settings` | Reload settings |  |  |
| `ui.debug.clear-caches` | Clear screen caches |  |  |
| `ui.debug.health` | Check the machine |  |  |
| `ui.debug.journal` | Show the session journal |  |  |
| `ui.debug.state` | Show the screen state |  |  |

### `model`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `ui.thread.model.default` | Default (settings) |  |  |
| `ui.thread.model.opus` | Opus |  |  |
| `ui.thread.model.opus-1m` | Opus 1M |  |  |
| `ui.thread.model.sonnet` | Sonnet |  |  |
| `ui.thread.model.sonnet-1m` | Sonnet 1M |  |  |
| `ui.thread.model.haiku` | Haiku |  |  |
| `ui.thread.model.fable` | Fable |  |  |
| `ui.thread.mode.default` | Default (settings) |  |  |
| `ui.thread.mode.manual` | Always ask |  |  |
| `ui.thread.mode.acceptedits` | Accept edits |  |  |
| `ui.thread.mode.auto` | Automatic |  |  |
| `ui.thread.mode.plan` | Plan |  |  |
| `ui.thread.mode.bypasspermissions` | Bypass (permission-mode) |  |  |
| `ui.thread.mode.cycle` | Cycle permission mode |  | `Shift+Tab` |
| `ui.thread.effort.default` | Default |  |  |
| `ui.thread.effort.low` | Low |  |  |
| `ui.thread.effort.medium` | Medium |  |  |
| `ui.thread.effort.high` | High |  |  |
| `ui.thread.effort.xhigh` | XHigh |  |  |
| `ui.thread.effort.max` | Max |  |  |
| `ui.thread.provider.claude` | Claude Code |  |  |
| `ui.thread.provider.codex` | Codex |  |  |
| `ui.thread.provider.gemini` | Gemini CLI |  |  |
| `ui.thread.skip-permissions.toggle` | Toggle skip permissions |  |  |

### `review`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `ui.thread.land-apply` | Apply to project |  |  |
| `ui.thread.land-merge` | Merge the branch |  |  |
| `ui.thread.land-pr` | Ask the agent for a PR |  |  |
| `ui.review.base.upstream` | Thread start |  |  |
| `ui.review.base.turn` | Last turn |  |  |
| `ui.review.base.head` | Last commit |  |  |
| `ui.review.next-file` | Next file |  | `Alt+ArrowRight` |
| `ui.review.prev-file` | Previous file |  | `Alt+ArrowLeft` |
| `ui.review.toggle-seen` | Mark file as reviewed |  | `Ctrl+Shift+S` |
| `ui.review.mark-all-seen` | Mark all as reviewed |  |  |
| `ui.review.mark-none-seen` | Mark none as reviewed |  |  |
| `ui.review.refresh` | Refresh changes |  |  |
| `ui.thread.commit` | Commit the thread checkout | `message`? |  |
| `ui.thread.push` | Push the thread branch |  |  |
| `ui.thread.update-from-base` | Update thread from its base |  |  |
| `ui.review.first-file` | First changed file |  |  |
| `ui.review.last-file` | Last changed file |  |  |
| `ui.review.open-file` | Open a file in the review | `path` |  |
| `ui.review.discard-file` | Revert file to the thread base | `path`? |  |
| `ui.review.copy-patch` | Copy the file diff |  |  |

### `settings`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `ui.settings.workspaces` | Workspaces |  |  |
| `ui.settings.appearance` | Appearance |  |  |
| `ui.settings.defaults` | Defaults |  |  |
| `ui.settings.accounts` | Accounts |  |  |
| `ui.settings.providers` | Providers |  |  |
| `ui.settings.skills` | Skills & commands |  |  |
| `ui.settings.agents` | Agents |  |  |
| `ui.settings.mcp` | MCP |  |  |
| `ui.settings.git` | Git |  |  |
| `ui.settings.integrations` | Integrations |  |  |
| `ui.settings.shortcuts` | Shortcuts |  |  |
| `ui.settings.about` | About |  |  |

### `tabs`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `ui.tab.chat` | Chat |  | `Ctrl+1` |
| `ui.tab.history` | History |  | `Ctrl+2` |
| `ui.tab.tree` | Tree |  | `Ctrl+3` |
| `ui.tab.reflog` | Reflog |  | `Ctrl+4` |
| `ui.tab.stats` | Statistics |  | `Ctrl+5` |
| `ui.tab.terminal` | Terminal |  | `Ctrl+6` |
| `ui.tab.next` | Next tab |  | `Ctrl+Tab` |
| `ui.tab.prev` | Previous tab |  | `Ctrl+Shift+Tab` |
| `ui.history.graph` | History |  |  |
| `ui.history.review` | Agent review |  | `Ctrl+Shift+R` |

### `terminal`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `ui.terminal.restart` | Restart terminal |  |  |
| `ui.terminal.clear` | Clear terminal |  | `Ctrl+Shift+K` |
| `ui.terminal.interrupt` | Send Ctrl+C |  |  |
| `ui.terminal.copy-screen` | Copy terminal screen |  |  |
| `ui.terminal.write` | Type into terminal | `text` |  |
| `ui.terminal.run` | Run in terminal | `command` |  |

### `thread`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `ui.thread.new` | New thread | `projectId`? | `Ctrl+N` |
| `ui.thread.new-isolated` | New isolated thread | `projectId`? | `Ctrl+Shift+N` |
| `ui.thread.select` | Open thread | `id` |  |
| `ui.thread.next` | Next thread |  | `Alt+ArrowDown` |
| `ui.thread.prev` | Previous thread |  | `Alt+ArrowUp` |
| `ui.thread.open-1` | Open thread 1 |  | `Alt+1` |
| `ui.thread.open-2` | Open thread 2 |  | `Alt+2` |
| `ui.thread.open-3` | Open thread 3 |  | `Alt+3` |
| `ui.thread.open-4` | Open thread 4 |  | `Alt+4` |
| `ui.thread.open-5` | Open thread 5 |  | `Alt+5` |
| `ui.thread.open-6` | Open thread 6 |  | `Alt+6` |
| `ui.thread.open-7` | Open thread 7 |  | `Alt+7` |
| `ui.thread.open-8` | Open thread 8 |  | `Alt+8` |
| `ui.thread.open-9` | Open thread 9 |  | `Alt+9` |
| `ui.thread.close` | Close tab — Fecha a aba da sessão (a aberta, sem `id`) e abre a vizinha do mesmo projeto. | `id`? | `Ctrl+Shift+W` |
| `ui.thread.close-others` | Close other tabs — Fecha as outras abas do projeto da sessão e deixa só ela. | `id`? |  |
| `ui.thread.next-tab` | Next session tab |  | `Ctrl+Shift+PageDown` |
| `ui.thread.prev-tab` | Previous session tab |  | `Ctrl+Shift+PageUp` |
| `ui.thread.rename` | Double-click to rename | `title`? | `F2` |
| `ui.thread.toggle-bookmark` | Bookmark thread |  | `Ctrl+D` |
| `ui.thread.mark-unread` | Mark as unread |  |  |
| `ui.thread.archive` | Archive thread |  | `Ctrl+Shift+E` |
| `ui.thread.delete` ⚠ | Delete thread |  | `Ctrl+Shift+Delete` |
| `ui.thread.interrupt` | Stop the agent |  | `Escape` |
| `ui.thread.send` | Send message | `text`? |  |
| `ui.composer.focus` | Focus the message box |  | `Ctrl+L` |
| `ui.composer.clear` | Clear the message box |  |  |
| `ui.composer.attach` | Attach image |  | `Ctrl+Shift+I` |
| `ui.composer.attach-paths` | Attach image files | `id`?, `paths` |  |
| `ui.composer.attach-data` | Attach image data | `id`?, `mime`, `data`, `name`? |  |
| `ui.composer.paste-image` | Paste image |  | `Ctrl+Shift+V` |
| `ui.thread.scroll-bottom` | Scroll to the end |  | `Ctrl+End` |
| `ui.thread.expand-tools` | Expand tool calls |  |  |
| `ui.thread.collapse-tools` | Collapse tool calls |  |  |
| `ui.thread.expand-thinking` | Show reasoning |  |  |
| `ui.permission.allow` | Allow |  | `Ctrl+Y` |
| `ui.permission.always` |  |  | `Ctrl+Shift+Y` |
| `ui.permission.deny` | Deny |  | `Ctrl+Shift+X` |
| `ui.thread.copy-path` | Copy checkout path |  |  |
| `ui.thread.copy-branch` | Copy branch name |  |  |
| `ui.thread.copy-session` | Copy session id |  |  |
| `ui.thread.copy-transcript` | Copy conversation as Markdown |  |  |
| `ui.thread.open-folder` | Open checkout folder |  |  |
| `ui.thread.reload` | Reload conversation |  |  |
| `ui.thread.first` | First thread |  | `Alt+Home` |
| `ui.thread.last` | Last thread |  | `Alt+End` |
| `ui.thread.copy-last-message` | Copy last agent message |  | `Ctrl+Shift+C` |
| `ui.thread.copy-id` | Copy thread id |  |  |
| `ui.thread.duplicate` | Duplicate thread |  |  |
| `ui.thread.collapse-thinking` | Hide reasoning |  |  |
| `ui.thread.scroll-top` | Scroll to the start |  | `Ctrl+Home` |
| `ui.thread.terminal` | Open the thread terminal |  | `Ctrl+Backquote` |
| `ui.composer.insert` | Insert text in the message box | `text` |  |
| `ui.composer.slash` | Start a slash command | `name`? |  |
| `ui.composer.recall` | Recall the last message |  |  |

### `ui`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `ui.inspect` 🔎 | Inspect the screen — Estado da tela: aba, thread aberta, painéis e diálogos, workspace ativo, contagens. |  |  |
| `ui.screenshot` 🔎 | Screenshot — PNG da janela em base64. | `cpu`? |  |
| `ui.run` | Run a screen command | `command`, `params`? |  |
| `ui.state.set` | Change screen state — Mescla campos no estado da tela (os mesmos do ui.inspect). | `patch` |  |

### `view`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `ui.palette.toggle` | Command palette |  | `Ctrl+K` |
| `ui.palette.commands` | All commands |  | `Ctrl+Shift+P` |
| `ui.settings.open` | Settings |  | `Ctrl+Comma` |
| `ui.settings.close` | Close settings |  |  |
| `ui.sidebar.toggle` | Show / hide sidebar |  | `Ctrl+B` |
| `ui.sidebar.mode.project` | Group by project |  |  |
| `ui.sidebar.mode.recent` | Group by date |  |  |
| `ui.sidebar.mode.toggle` | Toggle grouping |  | `Ctrl+Shift+G` |
| `ui.sidebar.archived.toggle` | Show archived |  | `Ctrl+Shift+A` |
| `ui.sidebar.filter` | Filter threads | `text`? |  |
| `ui.sidebar.collapse-all` | Collapse all groups |  |  |
| `ui.sidebar.expand-all` | Expand all groups |  |  |
| `ui.dialog.confirm` | Confirm — Confirma o diálogo aberto (apagar thread, descartar mudanças…), como o botão Confirmar. |  |  |
| `ui.dialog.cancel` | Cancel — Fecha o diálogo aberto sem fazer nada, ou a janela de resultado da paleta. |  |  |
| `ui.toasts.clear` | Dismiss notifications |  |  |
| `ui.sidebar.clear-filter` | Clear |  |  |
| `ui.sidebar.show-all` | Show every thread |  |  |
| `ui.sidebar.width-reset` | Reset sidebar width |  |  |
| `ui.palette.open` | Open the palette with text | `query`? |  |
| `ui.palette.threads` | Go to thread |  | `Ctrl+P` |
| `ui.settings.toggle` | Toggle settings |  |  |

### `window`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `ui.window.fullscreen` | Toggle full screen |  | `F11` |
| `ui.window.minimize` | Minimize |  |  |
| `ui.window.maximize` | Maximize |  |  |
| `ui.app.check-update` | Check for updates — Consulta o último release e avisa se há versão nova (ou que esta já é a mais nova). |  |  |
| `ui.app.quit` | Quit |  | `Ctrl+Q` |
| `ui.window.always-on-top` | Keep window on top |  |  |

### `workspace`

| Comando | Descrição | Parâmetros | Atalho |
|---|---|---|---|
| `ui.workspace.new` | New workspace |  | `Ctrl+Shift+O` |
| `ui.workspace.pick` | Which workspaces to open? |  |  |
| `ui.workspace.open` | Open workspace — Abre o workspace: na janela onde ele já está, ou numa janela nova (no desktop); na web, numa aba. | `id` |  |
| `ui.activity.toggle` | Sessions across all workspaces — Mostra / esconde as sessões de todos os workspaces que rodam, esperam você ou terminaram sem ser lidas. |  | `Ctrl+Shift+D` |
| `ui.activity.next` | Go to the next session that needs attention — Abre a próxima sessão que espera você ou terminou sem ser lida, em qualquer workspace (na janela dele). |  | `Ctrl+Shift+T` |
| `ui.workspace.close` | Close workspace tab |  | `Ctrl+W` |
| `ui.workspace.next` | Next workspace |  | `Ctrl+PageDown` |
| `ui.workspace.prev` | Previous workspace |  | `Ctrl+PageUp` |
| `ui.workspace.switch-1` | Workspace 1 |  | `Ctrl+Alt+1` |
| `ui.workspace.switch-2` | Workspace 2 |  | `Ctrl+Alt+2` |
| `ui.workspace.switch-3` | Workspace 3 |  | `Ctrl+Alt+3` |
| `ui.workspace.switch-4` | Workspace 4 |  | `Ctrl+Alt+4` |
| `ui.workspace.switch-5` | Workspace 5 |  | `Ctrl+Alt+5` |
| `ui.workspace.switch-6` | Workspace 6 |  | `Ctrl+Alt+6` |
| `ui.workspace.switch-7` | Workspace 7 |  | `Ctrl+Alt+7` |
| `ui.workspace.switch-8` | Workspace 8 |  | `Ctrl+Alt+8` |
| `ui.workspace.switch-9` | Workspace 9 |  | `Ctrl+Alt+9` |
| `ui.workspace.refresh` | Refresh |  | `F5` |
| `ui.workspace.edit` | Edit workspace |  |  |
| `ui.workspace.set-path` | Set new workspace folder | `path` |  |
| `ui.repo.open` | Open repository screen | `projectId` |  |
| `ui.repo.open-current` | Open the repository screen |  |  |
| `ui.repo.close` | Close repository |  |  |

🔎 só lê · ⚠ apaga ou reescreve algo difícil de desfazer

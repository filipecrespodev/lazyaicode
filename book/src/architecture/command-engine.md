# Motor de comandos

O registro (`crates/engine/src/registry.rs`) é uma tabela de `Spec`:

```rust
r.add(
    Spec::new("thread.land", "Bring changes into the project", "review")
        .params(vec![p::s("id"), p::one_of("mode", &["merge", "apply"], true)])
        .menu(&["Thread", "Integrate"])
        .doc("`apply` copia as mudanças …; `merge` faz merge da branch …")
        .run(|e, v| async move { /* … */ }),
);
```

| Campo | Para quê |
|---|---|
| `id` | `area.acao` em minúsculas; é o nome do comando em todo cliente |
| `label` | chave de tradução ou texto (a paleta e a referência mostram) |
| `group` | agrupamento (`thread`, `git`, `review`, `settings`…) |
| `params` | nome camelCase, tipo (`String`, `Bool`, `Int`, `Number`, `Path`, `Enum`, `Strings`, `Json`), obrigatório |
| `read_only` | só lê (o MCP marca como seguro; o journal não registra) |
| `destructive` | apaga ou reescreve algo difícil de desfazer |
| `run` | `async fn(Engine, Value) -> Result<Value, EngineError>` |

`Engine::run`:

1. acha o comando (`UnknownCommand` se não existe);
2. valida os parâmetros (`check_params`): objeto, obrigatórios, tipos e
   **nenhuma chave desconhecida** — um erro de digitação de um agente vira
   mensagem, não parâmetro ignorado;
3. roda numa task própria do tokio: um pânico vira `EngineError::Panicked`;
4. registra no journal (só comandos que mudam algo, com segredos ocultos).

## Quantos e quais

São **mais de 500**: ~250 do motor e ~260 de tela. A
[referência](../reference/commands.md) é gerada do registro
(`cargo xtask commands`); um teste do app falha se o total cair para 500 ou
menos, ou se um id se repetir entre motor e tela.

- **Motor:** `engine.*` (descoberta, journal, lote), `settings.*`, `prefs.*`,
  `account.*`, `workspace.*`, `project.*`, `thread.*`, `review`, `terminal.*`,
  `capabilities.*`, `provider.*`, `system.*`, `git.*`/`github.*`/`gitlab.*`/
  `ai.*` (os do cliente git são gerados de `git_cmds/` por
  `xtask/scripts/gen_git_commands.py`) e `git.path.*` (git direto por caminho,
  para scripts).
- **Tela (`ui.*`):** abas, paleta, configurações, barra lateral, navegação,
  composer, revisão, git rápido, aparência, idioma, janela, depuração e os do
  canal de controle (`ui.inspect`, `ui.screenshot`, `ui.run`, `ui.state.set`).

## Comandos de tela e contexto

Os comandos `ui.*` vivem na casca (`crates/ui-egui/src/commands.rs`) porque
mexem em estado de tela. Quando a paleta roda um comando do **motor**, a UI
preenche os parâmetros que ela conhece (`fill_context`): `id`/`threadId` da
thread aberta, `workspaceId`, `projectId`, `repoId` do repositório da tela.
O que faltar abre um formulário gerado da descrição dos parâmetros.

## Lotes e journal

- `engine.batch {steps:[{command, params}], stopOnError}` roda até 256
  passos e devolve um resultado por passo (`lazyaicode-cli batch arquivo.json`).
- `engine.journal {last}` devolve os últimos comandos que mudaram algo,
  com resultado e duração — o que um agente consulta para saber o que já fez.

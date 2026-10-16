# Contribuir

As regras completas estão no [`AGENTS.md`](https://github.com/filipecrespodev/lazyaicode/blob/main/AGENTS.md).
O essencial:

- **Tudo é um comando.** Comportamento visível ao usuário é um comando no
  motor (`crates/engine/src/*_cmds.rs`) ou, se for só estado de tela, em
  `crates/ui-egui/src/commands.rs`. A UI, a CLI, o canal de controle e o MCP
  chamam pelo id.
- **A UI é fina.** Ela não liga o motor: fala por comandos através de um
  `Client`. O `cargo xtask layers` falha se a UI depender do motor.
- **Nunca entrar em pânico.** Nada de `unwrap`/`expect`/`panic!`/indexação
  sem checagem em código de produção (clippy `-D warnings` barra). Entrada de
  fora é hostil.
- **Testes são o portão.** Toda mudança vem com teste; parser novo ganha um
  alvo de [fuzzing](../security/fuzzing.md).
- **Nunca quebrar o wasm.** `cargo xtask wasm`.
- **Textos traduzidos.** Toda chave nova nos quatro idiomas (`cargo xtask i18n`).

## Adicionar um comando

1. Escolha o id (`area.acao`, minúsculas) e o grupo.
2. Registre em `crates/engine/src/<área>_cmds.rs` com `Spec::new(id, rótulo, grupo)`,
   os parâmetros (`p::s`, `p::ob`…), `.read_only()`/`.destructive()` quando
   couber, e o `run`.
3. Teste pelo mesmo caminho dos clientes: `engine.run(id, json!({…}))`.
4. `cargo xtask commands` atualiza a referência.
5. Se a UI precisar, chame o id — não a função.

## Commits

Mensagens no estilo `tipo(área): descrição` (`feat(git): …`, `fix(chat): …`),
em português. Um assunto por commit.

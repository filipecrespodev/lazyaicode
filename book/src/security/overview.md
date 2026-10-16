# Segurança — visão geral

O LazyAICode roda agentes de IA que editam código e executam comandos na sua
máquina, com as suas credenciais. A segurança aqui tem três frentes:

1. **Não ampliar o que o agente pode fazer.** O app não concede permissão
   nenhuma por conta própria: os pedidos de permissão do agente chegam à UI
   (ou à automação) e só seguem com uma resposta explícita; o modo de
   permissão é o do Claude Code (`default`, `acceptEdits`, `plan`,
   `bypassPermissions`…), escolhido por thread.
2. **Não quebrar com entrada de fora.** Tudo o que vem de processos, disco e
   rede é tratado como hostil: [parsers com teto](parser-hardening.md),
   [fuzzing](fuzzing.md) e a regra de nunca entrar em pânico.
3. **Não abrir porta sem dono.** A [automação](automation-security.md) só
   escuta em loopback, com token, e o MCP no stdio não abre porta.

| Página | Para quê |
|---|---|
| [Modelo de ameaças](threat-model.md) | quem ataca, o quê, e o que fica fora do escopo |
| [Fronteiras de confiança](trust-boundaries.md) | por onde a entrada hostil chega |
| [Hardening dos parsers](parser-hardening.md) | os limites de cada parser e o checklist de revisão |
| [Fuzzing e testes de regressão](fuzzing.md) | os alvos, como rodar, o que já achou |
| [Segurança da automação](automation-security.md) | canal de controle, WebSocket, MCP |
| [Segredos e credenciais](secrets.md) | tokens, senhas do git, redação no journal |
| [Relatar vulnerabilidades](vulnerability-reporting.md) | como e para quem |

## Regras que o build aplica

- **Implementado:** `unsafe_code = "forbid"` no workspace inteiro (os crates
  do app não têm `unsafe`). Os crates de produção negam `unwrap`, `expect`,
  `panic!`, `todo!`, `unimplemented!` (e, nos de parsing puro, indexação sem
  checagem) — o `cargo xtask ci` roda o clippy com `-D warnings`.
- **Implementado:** o motor roda cada comando numa task própria; um pânico
  escapado vira `EngineError::Panicked` e não derruba o app. O
  `panic_hunt_every_command_with_hostile_params` chama **todo** comando com
  parâmetros vazios, absurdos e com caminhos de travessia.
- **Limitação conhecida:** a UI (egui) roda no quadro da janela; um pânico
  ali fecha o app. Ela segue as mesmas regras de lint; não há guarda de
  último recurso em volta do quadro.

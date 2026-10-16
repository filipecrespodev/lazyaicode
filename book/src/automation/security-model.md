# Modelo de segurança da automação

Resumo (o detalhe está em [Segurança da automação](../security/automation-security.md)):

- **Implementado:** o canal TCP só escuta em loopback e recusa outro
  endereço; o token tem 244 bits aleatórios, fica num arquivo 0600 e é comparado em
  tempo constante; o primeiro quadro precisa ser `auth`; limites de linha,
  resposta, profundidade, conexões, lote e tempo; o WebSocket recusa
  `Origin` de outro site; o MCP no stdio não abre porta.
- **Limitação conhecida:** o token é um bearer: quem o tem controla tudo o
  que o motor faz — inclusive mandar um agente rodar comandos na máquina. Não
  há escopos, identidade de cliente nem criptografia (use TLS num proxy se
  sair do loopback).
- **Proposto:** escopos por grupo de comando (só leitura, sem `destructive`,
  sem `terminal.*`), tokens com validade, e confirmação na UI para comandos
  destrutivos vindos da automação.

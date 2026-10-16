# Documentação do LazyAICode

O LazyAICode é um workspace desktop para agentes de código: threads em
paralelo com o Claude Code, o Codex ou o Gemini CLI, cada uma no seu
checkout git isolado, com revisão do que o agente mudou, um cliente git
completo e um terminal — tudo escrito em Rust, com uma interface egui
desenhada na GPU (wgpu) e um motor de comandos que scripts e agentes de IA
dirigem pelo mesmo caminho que a interface.

Este book é a porta de entrada mantida para instalação, arquitetura,
automação, segurança e desenvolvimento.

## Por onde começar

- Quem vai usar: [Instalação](getting-started/installation.md) e
  [Primeira thread](getting-started/first-thread.md).
- Quem vai contribuir: [Compilar](getting-started/building.md) e
  [Contribuir](getting-started/contributing.md).
- Mudança de arquitetura: [Visão geral](architecture/overview.md) e
  [Crates e camadas](architecture/crates.md).
- Scripts e agentes: [Automação](automation/overview.md) e a
  [referência de comandos](reference/commands.md).
- Mudança sensível (parser, processo filho, rede, automação): primeiro
  [Segurança](security/overview.md) e o [modelo de ameaças](security/threat-model.md).

## Para agentes de código

Leia nesta ordem:

1. [`AGENTS.md`](https://github.com/filipecrespodev/lazyaicode/blob/main/AGENTS.md):
   regras do repositório e os portões de verificação.
2. [`README.md`](https://github.com/filipecrespodev/lazyaicode/blob/main/README.md):
   o produto e os pontos de entrada.
3. [Arquitetura](architecture/overview.md): as fronteiras entre os crates
   e o caminho de um comando.
4. [Automação](automation/overview.md) antes de dirigir a CLI, o canal de
   controle ou o MCP.
5. [Segurança](security/overview.md) antes de mexer em um parser, em
   processos filhos, no canal de controle, no MCP ou em serialização.

Trate como entrada hostil tudo o que vem de fora: linhas dos agentes,
arquivos de sessão, saída do git, parâmetros de comando, mensagens do canal
de controle e do MCP, bytes do terminal. Uma página que descreve um controle
**proposto** não quer dizer que ele existe no código.

## Como as páginas de segurança falam

- **Implementado:** confirmado no código ou nos testes atuais.
- **Limitação conhecida:** um risco exposto ou um controle que falta hoje.
- **Proposto:** uma direção de projeto que o build atual não aplica.
- **Endurecimento futuro:** trabalho útil ainda sem implementação aceita.

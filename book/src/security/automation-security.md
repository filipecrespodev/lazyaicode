# Segurança da automação

## Canal de controle (TCP)

- **Implementado:** escuta só em loopback (`serve_tcp` recusa outro
  endereço; a ponte do MCP só conecta em loopback); o primeiro quadro tem de
  ser `auth` e um token errado fecha a conexão; token de 64
  caracteres hex (244 bits aleatórios do gerador do sistema), num arquivo
  criado com 0600 e
  comparado em tempo constante; linha ≤ 1 MiB, profundidade ≤ 128,
  resposta ≤ 8 MiB, 16 conexões, 600 s por pedido.
- **Limitação conhecida:** o token é um bearer e dá o controle inteiro do
  motor — inclusive `thread.send` (um agente com as permissões da thread),
  `terminal.*` (um shell) e `git.*`. Não há criptografia: não exponha fora do
  loopback sem TLS e autenticação na frente.

## Versão web (WebSocket)

- **Implementado:** o WebSocket só abre em `/ws` e só com `Origin` igual ao
  `Host` (barra outra aba do navegador); o mesmo `auth` do TCP; quadros
  ≤ 1 MiB; os arquivos estáticos não saem da pasta do build.
- **Implementado:** o token vai no fragmento (`#token=`), que o navegador não
  manda ao servidor nem grava em log de acesso; a página o tira da URL.
- **Limitação conhecida:** com `--web 0.0.0.0:…` (o padrão do contêiner) o
  servidor fica acessível na rede, sem TLS. Ponha atrás de um proxy com TLS
  e restrinja quem chega; o `serve` avisa ao escutar fora do loopback.

## MCP

- **Implementado:** stdio, sem porta; os mesmos tetos de linha e
  profundidade; resultados ≤ 8 MiB (cortados com aviso); as ferramentas
  marcam `readOnlyHint`. Na ponte, o MCP autentica no canal do app com o
  token do arquivo.
- **Limitação conhecida:** as anotações das ferramentas são dicas, não
  autorização: um cliente MCP pode chamar qualquer comando com
  `command_run`.

## Proposto

- escopos no token (só leitura; sem `destructive`; sem `terminal.*`/`thread.send`);
- tokens com validade e revogação;
- confirmação na UI para comandos `destructive` vindos da automação;
- log de auditoria persistido (hoje o journal é só da sessão).

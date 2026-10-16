# Linha de comando

```text
lazyaicode-cli [--data-dir DIR] [--ephemeral] [--connect ADDR --token-file F] <subcomando>

  commands [--filter X] [--group G] [--json | --markdown]   lista os comandos
  describe <id>                                            descreve um comando
  run <id> [JSON | chave=valor …]                          executa um comando
  batch <arquivo.json | ->  [--keep-going]                 lote [{command, params}]
  state                                                    estado da sessão
  journal                                                  últimos comandos que mudaram algo
  mcp [--bridge ADDR]                                      servidor MCP no stdio
  serve [--control ADDR] [--web ADDR --web-root DIR] [--token T]
```

- Sem `--connect`, o motor roda **no processo da CLI**, com o mesmo
  `state.json` do app (use `--data-dir` ou `LAZYAICODE_DATA_DIR` para outro,
  `--ephemeral` para não gravar nada).
- `run` aceita um objeto JSON ou `chave=valor` (o valor é lido como JSON
  quando for válido: `isolated=true`, `cols=80`, `ids=["a","b"]`).
- O resultado sai em JSON; erros saem no stderr com código 1, no formato
  `mensagem (código)`.
- `serve` sobe o motor sem janela: `--control 127.0.0.1:7878` para o canal
  TCP, `--web 127.0.0.1:8080 --web-root …` para a versão web. O token é
  gerado (ou vem de `--token`/`LAZYAICODE_TOKEN`) e gravado em
  `<dados>/control.token` com permissão 0600.

```sh
lazyaicode-cli commands --group thread
lazyaicode-cli describe thread.set-options
lazyaicode-cli run engine.version
lazyaicode-cli batch passos.json
lazyaicode-cli --connect 127.0.0.1:7878 run ui.inspect     # num app aberto com --control
```

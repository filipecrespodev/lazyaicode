# Versão web

O navegador não abre processos nem lê o disco, então o **motor não roda
lá**: a UI (o mesmo `lazyaicode-ui-egui`, compilado para WebAssembly em
`apps/lazyaicode-web`) fala com um `lazyaicode-cli serve` pelo canal de
controle em **WebSocket**, com o mesmo protocolo do TCP.

```sh
cargo xtask web    # trunk build --release → apps/lazyaicode-web/dist
lazyaicode-cli serve --web 127.0.0.1:8080 --web-root apps/lazyaicode-web/dist
# versão web em http://127.0.0.1:8080/#token=…
```

- `GET /` serve os arquivos estáticos (sem `..`, sem caminhos absolutos);
  `GET /ws` abre o canal. A mesma porta, um processo.
- O token vem no `#token=…` (o fragmento não vai para o servidor nem para
  logs); a página o tira da URL e guarda no `localStorage`. Sem token, a
  primeira tela pede.
- O WebSocket só aceita `Origin` igual ao `Host`: outra página aberta no
  navegador não fala com o servidor.
- Anexar imagem lê o arquivo no navegador e manda os bytes
  (`ui.composer.attach-data`); "escolher pasta" vira um campo de texto.
- O app desktop também conecta num servidor: `lazyaicode --connect ws://host:8080/ws --token …`.

## Contêiner

O `Dockerfile` da raiz compila o wasm e a CLI e monta uma imagem com o
servidor, o `git` e o Claude Code:

```sh
docker build -t lazyaicode-web .
docker run --rm -p 8080:8080 -e LAZYAICODE_TOKEN=$(openssl rand -hex 32) \
  -v lazyaicode-data:/data -v $HOME/.claude:/root/.claude -v $HOME/code:/work lazyaicode-web
```

O servidor escuta em `0.0.0.0` dentro do contêiner: publique só atrás de um
proxy com TLS (o canal não é criptografado). Veja
[Segurança da automação](../security/automation-security.md).

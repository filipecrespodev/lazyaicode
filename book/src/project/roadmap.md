# Roadmap

O [`ROADMAP.md`](https://github.com/filipecrespodev/lazyaicode/blob/main/ROADMAP.md)
na raiz é a fonte; resumo:

## Feito na reescrita em Rust

- motor de comandos (500+), modelo só de dados, canal de controle, MCP, CLI;
- UI egui sobre wgpu, compositor de referência em CPU testado contra o wgpu;
- versão web (wasm + WebSocket) e contêiner;
- empacotamento: MSI, DMG assinado/notarizado, AppImage, Flatpak, deb, rpm, tarballs;
- book, fuzzing e hardening dos parsers.

## Próximos

- telas git que ainda só existem como comando: dividir o staging em commits
  com a IA, chaves SSH, abrir no editor, log de atividade;
- shaping de texto complexo (árabe, hebraico, devanágari) — hoje o texto é o
  do egui;
- escopos no token do canal de controle e confirmação de comandos destrutivos;
- tokens do GitHub/GitLab no chaveiro do sistema;
- `cargo fuzz` contínuo no CI.

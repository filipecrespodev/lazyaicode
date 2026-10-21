# Instalação

O LazyAICode não tem conta nem chave própria: o agente é o `claude` (Claude
Code) instalado na máquina, rodando com a sua conta, ou o Codex/Gemini CLI.
Instale ao menos um deles e o `git`.

| Precisa | Para quê |
|---|---|
| [Claude Code](https://docs.claude.com/claude-code) (`claude` no PATH, ou `LAZYAICODE_CLAUDE_BIN`) | threads do Claude e a IA de commit |
| `git` 2.30+ | checkouts isolados, revisão, integração |
| Node.js (opcional) | Codex e Gemini CLI via `npx` quando não estão instalados |

## Pacotes

Cada release publica:

| Sistema | Pacote | Observação |
|---|---|---|
| Windows | `LazyAICode-<versão>-x86_64.msi` | instala o app, a `lazyaicode-cli.exe` e o agente WSL |
| macOS | `LazyAICode-<versão>-macos-<arch>.dmg` | assinado e notarizado; a CLI vem num `.zip` à parte |
| Linux | `LazyAICode-<versão>-x86_64.AppImage` | roda sem instalar; `LazyAICode.AppImage cli …` chama a CLI |
| Linux | `.deb`, `.rpm` | `/usr/bin/lazyaicode` e `/usr/bin/lazyaicode-cli` |
| Linux | `.flatpak` | sandbox com acesso à pasta pessoal |
| Linux, macOS, FreeBSD | `lazyaicode-<versão>-<os>-<arch>.tar.gz` | os dois binários, `.desktop` e ícone |
| Web | `docker build -t lazyaicode-web .` | veja [Versão web](../architecture/web.md) |

No Linux e no WSL2 há também o instalador de uma linha, que baixa o
AppImage e cria o comando `lazyaicode`:

```sh
curl -fsSL https://raw.githubusercontent.com/filipecrespodev/lazyaicode/main/scripts/install.sh | sh
```

### Linux: Wayland e X11

Numa sessão Wayland com XWayland (GNOME, KDE), a janela abre pelo X11. No
Wayland, uma janela coberta por outra pode ficar congelada por dezenas de
segundos depois de voltar à frente, sem responder a clique nem a
redimensionar. Para usar o Wayland assim mesmo, rode com
`LAZYAICODE_WAYLAND=1`.

## Do código-fonte

```sh
git clone https://github.com/filipecrespodev/lazyaicode
cd lazyaicode
cargo run --release -p lazyaicode          # o app
cargo run --release -p lazyaicode-cli -- --help
```

Veja [Compilar](building.md) para as dependências de cada sistema.

## Onde ficam os dados

| O quê | Onde |
|---|---|
| estado (workspaces, threads, preferências) | `<dados>/lazyaicode/state.json` (`~/.local/share`, `~/Library/Application Support`, `%APPDATA%`) — ou `LAZYAICODE_DATA_DIR` |
| checkouts isolados das threads | `<dados>/lazyaicode/worktrees/<projeto>/<thread>` |
| transcrições das threads Codex/Gemini | `<dados>/lazyaicode/transcripts/` |
| sessões do Claude Code | onde o Claude Code guarda (`<conta>/projects/…`) — o app só lê |
| token do canal de controle | `<dados>/lazyaicode/control.token` (0600) |

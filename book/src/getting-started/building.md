# Compilar

Rust estável (a versão mínima está em `rust-version` no `Cargo.toml` da
raiz). O libgit2 e o OpenSSL são compilados junto (`vendored`), então não há
biblioteca de sistema para o git.

| Sistema | Pacotes |
|---|---|
| Debian/Ubuntu | `build-essential pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev libvulkan-dev` |
| Fedora | `gcc pkgconf libxkbcommon-devel wayland-devel libX11-devel mesa-libGL-devel vulkan-loader-devel` |
| FreeBSD | `pkg install rust pkgconf libxkbcommon wayland` |
| macOS | Xcode Command Line Tools |
| Windows | MSVC Build Tools |

```sh
cargo build                     # crates + app + CLI + agente WSL
cargo run -p lazyaicode         # o app
cargo xtask ci                  # o portão antes de cada commit
```

## Tarefas do `xtask`

| Comando | O que faz |
|---|---|
| `cargo xtask layers` | confere as [camadas](../architecture/crates.md) |
| `cargo xtask wasm` | os crates da versão web compilam para `wasm32-unknown-unknown` |
| `cargo xtask i18n` | toda chave de tradução usada existe em en/pt/es/zh |
| `cargo xtask commands` | gera a [referência de comandos](../reference/commands.md) |
| `cargo xtask book` | gera os comandos e roda `mdbook build book` |
| `cargo xtask web` | `trunk build --release` da versão web |
| `cargo xtask dist <formato>` | pacotes (`tarball`, `deb`, `rpm`, `appimage`, `flatpak`, `dmg`, `msi`, `docker`) |
| `cargo xtask screenshots` | capturas da documentação, renderizadas em CPU |
| `cargo xtask ci` | fmt, layers, i18n, clippy `-D warnings`, testes, wasm |

## Ver o app sem abrir janela

```sh
cargo run -p lazyaicode -- --ephemeral --screenshot tela.png --size 1280x800 \
  --run 'ui.palette.commands {}'
```

`--screenshot` renderiza com o [compositor em CPU](../architecture/rendering.md)
e sai — funciona em CI, servidor e contêiner, sem GPU. `--run` executa
comandos de tela antes da captura.

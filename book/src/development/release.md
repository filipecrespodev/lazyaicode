# Empacotamento e release

Todos os formatos saem de `cargo xtask dist <formato>` para `packaging/out/`:

| Formato | Ferramenta | Receita |
|---|---|---|
| `tarball` | `tar` (zip no Windows) | `xtask/src/dist.rs` |
| `deb` | `cargo-deb` | `[package.metadata.deb]` em `apps/lazyaicode/Cargo.toml` |
| `rpm` | `cargo-generate-rpm` | `[package.metadata.generate-rpm]` |
| `appimage` | `appimagetool` | `packaging/linux/appimage.sh` |
| `flatpak` | `flatpak-builder` | `packaging/linux/flatpak/ai.lazyaicode.LazyAICode.yml` |
| `dmg` | `hdiutil`, `codesign`, `notarytool` | `packaging/macos/dmg.sh` |
| `msi` | `cargo-wix` (WiX 3) | `packaging/windows/wix/main.wxs` |
| `docker` | `docker build` | `Dockerfile` |

## macOS: assinar e notarizar

`dmg.sh` monta o `LazyAICode.app`, assina os binários e o bundle com
*hardened runtime* e `packaging/macos/entitlements.plist`, cria o DMG, assina
o DMG e, com as credenciais, envia ao serviço de notarização e grampeia o
ticket:

| Variável | Para quê |
|---|---|
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Nome (TEAMID)` |
| `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD` | `xcrun notarytool submit --wait` e `stapler staple` |

Sem as variáveis, o DMG sai sem assinatura (para teste local).

## Windows

O MSI instala `lazyaicode.exe`, `lazyaicode-cli.exe` e o
`lazyaicode-agent` Linux (em `resources/`), que o app copia para a distro WSL
no primeiro uso. O agente é compilado no job Linux do release
(`cargo build --release -p lazyaicode-wsl-agent`) e baixado para
`packaging/windows/resources/` antes do `cargo wix`.

## O workflow

`.github/workflows/release.yml` roda em tags `v*`: compila e empacota em
Linux (tarball, deb, rpm, AppImage, Flatpak), macOS arm64 e x86_64 (DMG +
zip da CLI), Windows (MSI + zip) e FreeBSD (tarball, numa VM), gera a versão
web, e publica tudo num release com as somas SHA-256.

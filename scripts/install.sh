#!/bin/sh
# Instala o LazyAICode (Linux / WSL2 via WSLg) e um comando `lazyaicode` que
# abre o app desacoplado do terminal. Rode de novo para atualizar.
set -eu
REPO=filipecrespodev/lazyaicode
DIR="$HOME/.local/share/lazyaicode/app"
BIN="$HOME/.local/bin"
mkdir -p "$DIR" "$BIN"

case "$(uname -m)" in
  x86_64|amd64) ;;
  *) echo "só há AppImage x86_64 por enquanto ($(uname -m) não suportado)" >&2; exit 1 ;;
esac

url=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
  | grep -o 'https://[^"]*x86_64\.AppImage"' | tr -d '"' | head -n1)
[ -n "$url" ] || { echo "nenhum AppImage no último release" >&2; exit 1; }
curl -fL "$url" -o "$DIR/LazyAICode.AppImage"
chmod +x "$DIR/LazyAICode.AppImage"

cat > "$BIN/lazyaicode" <<'WRAP'
#!/bin/sh
# --appimage-extract-and-run: dispensa FUSE (ausente na maioria das instalações WSL)
setsid nohup "$HOME/.local/share/lazyaicode/app/LazyAICode.AppImage" --appimage-extract-and-run "$@" >/dev/null 2>&1 &
WRAP
chmod +x "$BIN/lazyaicode"

if grep -qi microsoft /proc/sys/kernel/osrelease 2>/dev/null; then
  [ -n "${WAYLAND_DISPLAY:-}${DISPLAY:-}" ] || echo "aviso: sem display — o app precisa do WSLg (WSL2 no Windows 11 ou Windows 10 21H2+)"
  command -v git >/dev/null || echo "aviso: instale o git no WSL (sudo apt install git)"
  command -v claude >/dev/null || echo "aviso: instale o Claude Code dentro do WSL (o do Windows não serve: curl -fsSL https://claude.ai/install.sh | bash)"
fi
case ":$PATH:" in *":$BIN:"*) ;; *) echo "adicione $BIN ao PATH";; esac
echo "instalado: rode 'lazyaicode'"

# FreeBSD

O app compila nativo no FreeBSD (winit X11/Wayland, wgpu Vulkan/GL):

```sh
pkg install rust git pkgconf
cargo build --release -p lazyaicode -p lazyaicode-cli
cargo xtask dist tarball   # packaging/out/lazyaicode-<versão>-freebsd-<arch>.tar.gz
```

O tarball traz os dois binários, o `.desktop` e o ícone. O Claude Code roda
no FreeBSD pelo `npm install -g @anthropic-ai/claude-code` (Node do pkg).

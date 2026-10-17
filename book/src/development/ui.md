# Interface e capturas

## Regras

- **Fina:** a UI lê da cópia local (`data.rs`) e age com
  `app.call(id, params, callback)` ou `app.fire(id, params)`. Nada de lógica
  de domínio na tela.
- **Estado de tela em serde:** `state.rs` (`UiState`) é o que o
  `ui.inspect` mostra e o `ui.state.set` muda.
- **Cores dos tokens:** `theme::Tokens` (paleta + derivadas). Nenhuma cor
  fixa nas telas; o teste `every_theme_parses_and_has_readable_text` confere
  contraste em todas as paletas.
- **Ícones Phosphor** (`egui_phosphor::regular::*`); a fonte padrão não tem
  `●`/`•`/CJK.
- **Textos traduzidos** (`t("area.chave")`, `tf` com `{placeholders}`).

## Janelas

Cada workspace tem a sua janela (`windows.rs`): a principal é dona das
outras e as desenha como viewports imediatos do egui; cada uma é um
`LazyApp` inteiro, com o próprio `Client` no mesmo motor. A casca entrega a
fábrica (`set_window_factory`); sem ela (web, `--screenshot`) os workspaces
ficam em abas. Pedidos de abrir um workspace (`open_workspace`) sobem para a
principal, que foca a janela onde ele já está ou abre uma nova. Sem janelas
de verdade (contexto de teste), as filhas rodam num contexto egui próprio,
fora da tela — o teste `each_workspace_opens_in_its_own_window` dirige
assim. `ui.inspect` mostra `windows` (as outras janelas e o que cada uma
tem aberto) e `activity` (`thread.activity`: o que roda, espera ou terminou
sem ser lido em todos os workspaces).

## Ver a tela sem abrir janela

```sh
lazyaicode --ephemeral --screenshot tela.png --size 1280x800
lazyaicode --data-dir /tmp/demo --screenshot thread.png --frames 60 \
  --run 'ui.thread.select {"id":"…"}' --run 'ui.tab.history {}'
cargo xtask screenshots    # as capturas do book
```

O `--screenshot` roda os quadros num contexto egui próprio e rasteriza com o
[compositor em CPU](../architecture/rendering.md): não rouba foco, não precisa
de GPU nem de display.

Com o app aberto em `--control`, `ui.screenshot` devolve a captura da janela
(wgpu) ou, com `{"cpu": true}`, a do compositor em CPU.

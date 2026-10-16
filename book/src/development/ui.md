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

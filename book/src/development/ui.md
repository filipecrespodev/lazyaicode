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

Cada workspace tem a sua janela (`windows.rs`). Cada uma é um `LazyApp`
inteiro, com o próprio `Client` no mesmo motor; as filhas são viewports
*deferred* do egui penduradas na principal, e cada uma desenha com os
próprios quadros. A casca entrega a fábrica (`set_window_factory`); sem ela
(web, `--screenshot`) os workspaces ficam em abas.

As filhas moram num registro da thread da interface (o callback do egui
leva só o id do viewport). Pedidos de abrir um workspace (`open_workspace`)
vão para o registro e qualquer janela os atende no próprio quadro: foca a
janela onde ele já está ou abre uma nova.

**Janela escondida não desenha.** No Wayland, uma janela minimizada, coberta
ou em outra área de trabalho não recebe quadros — e a lógica da tela roda
dentro deles. Por isso:

- nada pode depender da principal desenhar (as filhas não são viewports
  imediatos, que só desenham dentro do quadro dela);
- o `LocalClient` repassa os eventos do motor para uma fila sem limite (o
  canal do motor transbordaria e perderia o meio da conversa);
- os avisos do sistema saem de um observador em segundo plano
  (`apps/lazyaicode/src/notifier.rs`); a janela informa a cada quadro a
  thread que mostra com foco (`Host::report_looking`), e o aviso só não sai
  se uma janela desenhou depois do evento mostrando a thread.

Sem janelas de verdade (contexto de teste), as filhas rodam num contexto
egui próprio, fora da tela (`windows::offscreen_passes`); o teste
`each_workspace_opens_in_its_own_window` dirige assim, inclusive com a
principal sem quadros. `ui.inspect` mostra `windows` (as outras janelas e o
que cada uma tem aberto), `activity` (`thread.activity`) e `update`
(`system.check-update`).

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

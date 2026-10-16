# Renderização

## egui sobre wgpu

A interface é **egui** (modo imediato), uma camada fina sobre o motor. O
**winit** cuida das janelas e da entrada; o **eframe** amarra os dois. A
pintura vai para a GPU pelo **wgpu**, com os backends que ele escolhe em
cada sistema:

| Sistema | Backend |
|---|---|
| macOS | Metal |
| Windows | DX12 (ou Vulkan) |
| Linux, FreeBSD | Vulkan (ou GL) — no WSLg o app força GL (`WGPU_BACKEND=gl`), onde o Vulkan costuma falhar |
| Navegador | WebGPU, com WebGL 2 de reserva |

Fontes: as do egui, os ícones **Phosphor** e, no desktop, uma fonte CJK do
sistema (Noto CJK, PingFang, Microsoft YaHei…) como fallback para chinês,
japonês e coreano. Todas as cores saem de `theme::Tokens` (13 paletas,
claro/escuro do sistema, densidade, escala, larguras).

## O compositor em CPU

`lazyaicode-render-cpu` desenha a mesma saída do egui (os triângulos
texturizados de `ctx.tessellate`) **sem GPU**:

- a imagem é dividida em **tiles de 256×256**, rasterizados em paralelo com
  **rayon** (um tile por tarefa); cada tile percorre só os triângulos cuja
  caixa o toca, na ordem de pintura;
- regra *top-left* nas arestas compartilhadas (sem costura nem pintura
  dupla), recorte por `clip_rect`, amostragem bilinear ou nearest conforme a
  textura, mistura com alfa pré-multiplicado em espaço gama — como o
  `egui-wgpu` num framebuffer não-sRGB;
- o resultado é um `tiny_skia::Pixmap`, que grava PNG.

Ele é o **oráculo** do caminho de GPU: `crates/render-cpu/tests/oracle.rs`
desenha a mesma tela pelos dois (o wgpu via `egui_kittest`) e compara pixel
a pixel. Na medição atual a diferença média é **0,006** por canal, e o teste
falha acima de 0,5 (ou com mais de 0,1% de pixels muito diferentes). Sem
adaptador gráfico, o lado wgpu é pulado e só a CPU é conferida.

Usos fora dos testes: `lazyaicode --screenshot` (sem janela, para CI e
documentação) e `ui.screenshot {"cpu": true}` no canal de controle.

## O que não se aplica

Esta arquitetura segue a dos apps da família *Craft* (editores de imagem e
vetor). Peças de lá que **não** têm papel num workspace de agentes ficaram de
fora de propósito: gestão de cor ICC e prova de cor na GPU, documento em
tiles com copy-on-write, filtros, geometria `kurbo`, booleanas `linesweeper`
e o par `skrifa`/HarfRust de texto. O texto é o do egui (sem shaping
complexo — veja o [roadmap](../project/roadmap.md)); os tiles de 256² e o
paralelismo existem no compositor de referência, que é onde eles servem.

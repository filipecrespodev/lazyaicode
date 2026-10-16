# Hardening dos parsers

Não entrar em pânico é o mínimo. Um parser de entrada hostil também precisa
limitar **memória**, **tempo**, **recursão** e **quanto cresce a saída**.

## Controles implementados

### Linhas dos agentes — `lazyaicode-agent::lines`

`BoundedLines` lê com `take(max + 1)`: uma linha sem `\n` não cresce além de
64 MiB; o resto é pulado em pedaços de 64 KiB e a thread recebe um aviso
(`lazy_notice`). O `lines()` do tokio usado antes crescia sem limite. O
stderr dos provedores ACP tem teto de 64 KiB por linha e 8 000 caracteres no
total guardado.

### Transcrição — `lazyaicode-doc::transcript`

O reducer aceita qualquer JSON: campo ausente ou de tipo errado vira valor
vazio; índices de bloco desconhecidos são ignorados; `parent_tool_use_id`
filtra subagentes. Corta resultados de ferramenta em 256 KiB (respeitando
fronteira de caractere), o JSON parcial de entrada em 256 KiB e a lista em
20 000 itens. Teste de propriedade (`never_panics_on_hostile_messages`) e
alvo de fuzzing `transcript`.

### Histórico de sessão — `lazyaicode-agent::transcript::load`

Lê o `.jsonl` linha a linha (antes: o arquivo inteiro numa `String`), com
linha ≤ 64 MiB e as 50 000 mensagens mais recentes. Alvo `session`.

### Protocolo do agente WSL — `lazyaicode-git::proto`

Quadros `u32` big-endian + JSON. O leitor **não aloca o tamanho
declarado**: cresce com os bytes que chegam (`take(len).read_to_end`), então
um cabeçalho de 512 MiB sem corpo falha com `UnexpectedEof` sem alocar nada
(`a_huge_header_without_data_fails_without_allocating_it`). Alvo `git_frame`.

### Canal de controle e MCP — `lazyaicode-protocol::frame`, `automation`

`decode_line` mede a linha (≤ 1 MiB) e a **profundidade** do JSON (≤ 128,
contando colchetes fora de strings, sem montar a árvore) antes do parse. O
leitor de linhas do servidor usa o mesmo `take` e descarta o resto de uma
linha grande demais, respondendo `too_large` e seguindo com a próxima.
Respostas acima de 8 MiB viram erro `too_large`. Alvos `frame` e `params`.

### Terminal — `lazyaicode-term`

O fuzzing (`never_panics_on_arbitrary_bytes`) achou um estouro de contador
no `vt100` 0.16 ao quebrar linha com caractere largo numa tela de 1 linha.
O emulador agora impõe tela mínima de 2 linhas × 4 colunas, e o teste segue
no CI. Alvo `term`.

### Diffs e grafo — `lazyaicode-doc::diff`, `graph`

`parse_patch` usa aritmética saturante na numeração e ignora o que não é
hunk; `graph::layout` limita as colunas a 32 e aceita pais repetidos,
ciclos e commits ausentes. Alvos `patch` e `graph`.

### Imagens

Do disco: até 5 MiB e tipo detectado pelos bytes mágicos (`sniff_image`). Na
UI: antes de decodificar, o cabeçalho precisa declarar no máximo 8192 px de
lado e 40 Mpx no total — um PNG de 100 bytes que diz 60000×60000 não chega ao
decodificador (`refuses_images_whose_header_declares_a_huge_canvas`).

### Caminhos

- arquivos estáticos da versão web: só componentes normais (`resolve_static`);
- `capabilities.trash`: só dentro de `skills/`, `agents/`, `commands/` da
  conta ou de `.claude/`/`.agents/` de um projeto do workspace, nunca de
  `plugins/`, e só arquivos;
- `thread.discard-file`: caminho relativo, sem `..`;
- `system.open-url`: só `http(s)`, sem espaços ou quebras de linha.

## Checklist de revisão

Para toda contagem, tamanho, offset ou caminho que vem de fora:

1. Leia num tipo que represente o campo sem truncar.
2. Valide o limite **antes** de alocar (`take`, `with_capacity(min(..))`).
3. Use `checked_*`/`saturating_*`; nada de `a - b` com dados.
4. `get(i)` em vez de `[i]`; fatie strings só em fronteira de caractere.
5. Limite recursão e profundidade.
6. Devolva um erro do domínio; nunca `panic!` nem laço sem fim.
7. Teste zero, máximo, máximo + 1, truncado e declarações inconsistentes.
8. Ponha um alvo em `fuzz/` (e sementes em `fuzz/seeds/<alvo>/`).

## Endurecimento futuro

- teto de corpo nas respostas do GitHub/GitLab (`reqwest` lê tudo);
- orçamento de tempo por comando (hoje só o teto de 600 s do canal);
- limite agregado de memória por thread (transcrição + changes em cache);
- `cargo fuzz` contínuo no CI (hoje roda o smoke em Rust estável).

# Fuzzing e testes de regressão

## Alvos

`fuzz/` é um crate do `cargo fuzz` (libFuzzer). Cada alvo é uma função em
`fuzz/src/lib.rs` que recebe bytes arbitrários:

| Alvo | O que exercita |
|---|---|
| `transcript` | linhas de stream-json no reducer da conversa |
| `frame` | `decode_line` do canal de controle (pedido e quadro) e a medição de profundidade |
| `patch` | `parse_patch` e o `split_patch` do diff por arquivo |
| `graph` | o layout do grafo com pais arbitrários (ciclos, repetidos, ausentes) |
| `term` | bytes do PTY no emulador, com redimensionamentos no meio |
| `acp_translate` | atualizações ACP (Codex, Gemini) no tradutor |
| `session` | um `.jsonl` de sessão no leitor de títulos e no `transcript::load` |
| `git_frame` | quadros com prefixo de tamanho do agente WSL |
| `frontmatter` | o frontmatter de skills/agentes/comandos |
| `params` | `check_params` contra uma descrição fixa |
| `wslpath` | caminhos UNC do WSL |
| `slash_rank` | a busca do menu `/` |

## Rodar

```sh
# de verdade (nightly, com AddressSanitizer)
cargo install cargo-fuzz
cargo +nightly fuzz run transcript fuzz/seeds/transcript -- -max_total_time=600

# smoke em Rust estável (o que o CI roda): sementes + 2000 mutações por alvo
cargo test --manifest-path fuzz/Cargo.toml
FUZZ_SMOKE_ITERS=200000 cargo test --manifest-path fuzz/Cargo.toml --release
```

As sementes ficam em `fuzz/seeds/<alvo>/` (versionadas); o corpus que o
libFuzzer gera fica em `fuzz/corpus/` (ignorado pelo git). Um crash
minimizado vira teste de regressão no crate do parser, com o byte-a-byte no
corpo do teste e uma nota de onde veio.

## Testes de propriedade e de varredura

Além do `fuzz/`, os crates têm `proptest` para as entradas de fora
(`protocol::frame`, `doc::transcript`, `doc::diff`, `doc::graph`, `term`), e
o motor tem o **panic hunt**: todo comando registrado é chamado com três
variantes de parâmetros hostis (vazios; `../../../etc/passwd`, `u64::MAX`;
strings com NUL e emoji repetidos, `-1`) e não pode entrar em pânico. Ficam
de fora só os comandos que abrem janela, navegador ou processos externos.

## O que já achou

- **`vt100` com tela de 1 linha** (`term`, teste de propriedade): estouro
  de contador ao quebrar linha com caractere largo. Correção: tela mínima de
  2×4 no `lazyaicode-term`.
- **Revisão para os alvos** (`git_frame`, `transcript`): o quadro do agente
  WSL pré-alocava o tamanho declarado (até 512 MiB) e o stdout dos agentes
  era lido sem teto de linha; os dois agora crescem só com os dados reais.

## Última rodada

16/10/2026, local: os 12 alvos com libFuzzer + AddressSanitizer, 45 s cada
(`-rss_limit_mb=2048 -max_len=65536`), partindo das sementes — cerca de
**32 milhões de execuções, nenhum crash, vazamento ou estouro de memória**.
O alvo mais lento é o `term` (telas de até 255×255 por entrada).

O CI roda o smoke a cada push e o libFuzzer de verdade (um minuto por alvo)
fora dos pull requests.

A página de [hardening](parser-hardening.md) lista os limites de cada parser.

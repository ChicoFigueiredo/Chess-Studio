# Cobertura de testes (NFR-10)

Medição local de 12/11/2026 no `main` (bf2ebe0), sem CI. Fecha o item de medição do [#269](https://github.com/ChicoFigueiredo/Chess-Studio/issues/269); o que ficou abaixo da meta está no [#313](https://github.com/ChicoFigueiredo/Chess-Studio/issues/313).

## Como medir

```sh
cargo install cargo-llvm-cov --locked      # MIT OR Apache-2.0
rustup component add llvm-tools-preview
/home/chico/.cache/chess-studio-build/cargo-fila llvm-cov --workspace --no-fail-fast --json --output-path cobertura.json
```

A suíte instrumentada rodou 2316 testes, 0 falhas. Os números são de linhas e incluem os módulos de teste que moram no mesmo arquivo do código, o que puxa o percentual um pouco para cima.

## Resultado por crate

| Crate | Linhas cobertas | Total | % |
|---|---|---|---|
| chess-core | 762 | 804 | 94,8 |
| chess-game | 1949 | 2062 | 94,5 |
| chess-input | 1329 | 1380 | 96,3 |
| chess-net | 3393 | 3805 | 89,2 |
| chess-openings | 149 | 156 | 95,5 |
| chess-pgn | 817 | 888 | 92,0 |
| chess-providers | 4774 | 5424 | 88,0 |
| chess-store | 3224 | 3414 | 94,4 |
| chess-training | 2158 | 2353 | 91,7 |
| chess-uci | 3354 | 3772 | 88,9 |
| chess-vault | 702 | 792 | 88,6 |
| chess-studio | 39240 | 63764 | 61,5 |
| **Workspace** | 61851 | 88614 | 69,8 |

## Alvos do NFR-10 (≥ 85% de linhas)

| Alvo | % | Resultado |
|---|---|---|
| `chess-net` | 89,2 | PASS |
| `chess-vault` | 88,6 | PASS |
| `chess-providers` | 88,0 | PASS |
| `chess-studio::fair_play` | 94,3 | PASS |
| `chess-studio::connections` | 83,3 | FAIL |
| `chess-studio::online` (com `online_import_ui.rs`) | 51,9 | FAIL |
| `chess-studio::remote_engine` | 36,4 | FAIL |
| `chess-studio::engine_online` | 34,4 | FAIL |

Os crates sem Bevy passam todos. O que falta está no `chess-studio`, quase todo em desenho egui e no modo de demonstração.

## Arquivos mais descobertos nos alvos

| Arquivo | % |
|---|---|
| `chess-studio/src/engine_online/view.rs` | 0,9 |
| `chess-studio/src/online_import_ui.rs` | 7,3 |
| `chess-studio/src/engine_online/exit.rs` | 17,0 |
| `chess-vault/src/keyring.rs` (cofre real do sistema, que a suíte não toca) | 17,5 |
| `chess-studio/src/engine_online/mod.rs` | 25,3 |
| `chess-studio/src/online/view.rs` | 28,9 |
| `chess-studio/src/engine_online/demo.rs` | 29,0 |
| `chess-studio/src/engine_online/spectator.rs` | 31,5 |
| `chess-studio/src/online/mod.rs` | 35,1 |
| `chess-studio/src/remote_engine.rs` | 36,4 |
| `chess-studio/src/connections/view_fide.rs` | 37,5 |
| `chess-studio/src/connections/stored_data.rs` | 52,6 |

A parte "100% dos ramos de segurança e fair play mapeados por teste" do NFR-10 está em [`docs/seguranca/matriz-ramos-v4.md`](../seguranca/matriz-ramos-v4.md).

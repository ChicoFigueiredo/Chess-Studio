# Engines reais testadas pelo app

Resultado do teste [`real_engines.rs`](../../crates/chess-uci/tests/real_engines.rs) em **22/11/2026**, Linux x86-64 (AMD Ryzen 7 3700X, RTX 4080 SUPER). Os binários **nunca entram no repositório** (ADR-0003); este documento só registra de onde vieram e como repetir. Todas são GPL-3.0 e rodam como processo externo (ADR-0004).

## O que o teste faz

Usa os mesmos caminhos do app, sem simulação:

- **Detecção:** `detect_host`, o "Procurar no computador" do gerenciador, varre `<dados>/engines/` e valida cada candidata por handshake UCI.
- **Montagem das opções:** o Lc0 sobe com a rede de `<engines>/networks/` (`weights_option`, a mesma barreira do app) e com o backend que `recommended_backend` escolhe entre os que ele anuncia.
- **Autojogo:** 20 meios-lances em que cada Lance da Engine passa pelo `chess-core` (`search_legal_move`); `quit` tem de ser limpo.
- **Análise:** `AnalysisController` com 3 linhas (1 se a Engine não tem `MultiPV`) e mate em 1 achado (`Ra8#`).
- **Engine adversária:** `EnginePlayer` de pretas, Simulação FIDE com relógio de 60 s + 1 s, Elo 1500, contra um humano roteirizado; precisa responder 4 vezes.

```text
cargo test -p chess-uci --test real_engines -- --ignored --nocapture
CHESS_STUDIO_REAL_ENGINES=lc0,berserk cargo test ...   # só algumas
```

## Resultado

| Engine | Versão | Autojogo | Análise | Adversária | Força aplicada ao pedir Elo 1500 |
|---|---|---|---|---|---|
| Stockfish | 19 | PASS | PASS (3 linhas) | PASS | `Elo` (1500) |
| Lc0 | 0.32.1, CUDA, rede T1-256x10 | PASS | PASS (3 linhas) | PASS | `Unsupported` |
| Fairy-Stockfish | 14 | PASS | PASS (3 linhas) | PASS | `Elo` (1500) |
| Koivisto | 9.0 | PASS | PASS (1 linha: sem `MultiPV`) | PASS | `Unsupported` |
| Berserk | 14 | PASS | PASS (3 linhas) | PASS | `Unsupported` |
| Ethereal | 14.00 (avaliação clássica) | PASS | PASS (3 linhas) | PASS | `Unsupported` |

- Só Stockfish e Fairy-Stockfish têm `UCI_Elo`; nas outras a Engine joga com força plena e o app diz que não consegue limitá-la (`StrengthApplied::Unsupported`). Não foi conferido aqui se a interface mostra esse aviso com cada uma.
- O Lc0 é busca em árvore: num mate já provado a "profundidade" dele fica em 2. Análise que exija profundidade mínima falha por engano com ele; o teste não exige profundidade no mate.
- **UNVERIFIED:** Windows e macOS; a interface gráfica com cada Engine (clique no gerenciador, troca de Engine no painel, Partida inteira); Lc0 sem GPU (backend `eigen`/`blas` na CPU); força relativa das Engines.

## De onde vieram

| Engine | Origem | Verificação |
|---|---|---|
| Fairy-Stockfish 14 | release `fairy_sf_14`, `fairy-stockfish_x86-64-bmi2` | SHA-256 `9c8ff22d7474100cf067a8f2a83b278619369997aa9d0e69284c465f160e3419` |
| Koivisto 9.0 | release `v9.0`, `Koivisto_9.0-linux-avx2-pgo` | SHA-256 `1c28f937c0dae37e7f3834f8d0ce318d77da33bf2d392b23597c4b4b67e99b36` |
| Lc0 0.32.1 | fonte, tag `v0.32.1` (commit `fd71a2d`); `meson setup build --buildtype=release -Dgtest=false -Dblas=true -Dopenblas=false` e `ninja -C build` (o meson achou CUDA e cuDNN) | rede T1-256x10 do catálogo do app, SHA-256 `bc27a6cae8ad36f2b9a80a6ad9dabb0d6fda25b1e7f481a79bc359e14f563406` conferido |
| Berserk 14 | fonte, tag `14` (commit `8ae895a`); `make build EXE=berserk` | rede `berserk-9b84c340af7e.nn` (SHA-256 `9b84c340af7e45f6e07f0046235ccb327f4ae0840c8ee2c4b97b99121e5c5084`) baixada do release do próprio Berserk, em vez do `curl -k` do makefile |
| Ethereal 14.00 | fonte, tag `v14.00` (commit `96cde8d`); `make basic EXE=ethereal` com clang (o alvo `pgo` exige `llvm-profdata`) | sem rede neural (avaliação clássica) |

O Alexandria 9.1.0 ficou de fora: pede uma rede NNUE de 118 MB cujo vínculo com a versão do código não dá para confirmar pelo release.

Instalação: um executável por pasta em `~/.local/share/ChessStudio/engines/<nome>/`, com nome que o app reconhece (`lc0`, `fairy-stockfish`, `koivisto`, `berserk`, `ethereal`, `stockfish...`); redes em `~/.local/share/ChessStudio/engines/networks/`.

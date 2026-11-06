# Relatório do Gate A — Prova de viabilidade técnica

Formato da §19 do prompt mestre. Os campos `PENDENTE (usuário)` dependem dos builds nas VMs Windows e macOS (DEC-25) e mantêm `STATUS` e `RESULTADO` como `PENDENTE`. Veja "Como completar" no fim.

```text
CHESS_STUDIO_GATE: A
STATUS: PENDENTE (Windows e macOS não verificados; Linux sem bloqueadores)
REPO / BRANCH / COMMIT: ChicoFigueiredo/Chess-Studio / main / 6c6e032f94b9 (#70, benchmark 5 min)
```

## VERSOES

Fonte: `Cargo.lock` e `rust-toolchain.toml` no commit acima.

| Componente | Versão | Observação |
|---|---|---|
| Rust | 1.98.1 (canal `1.98` fixado em `rust-toolchain.toml`, rustfmt e clippy) | edição 2024, resolver 3 |
| Bevy | 0.20.1 | features mínimas, x11 e wayland |
| bevy_egui | 0.43.1 | |
| egui | 0.36.2 | |
| wgpu | 30.0.1 | |
| winit | 0.30.13 | |
| cozy-chess | 0.3.4 | atrás da API própria do `chess-core` |
| SQLite (rusqlite) | **ausente do lockfile** | `chess-store` ainda sem persistência; DEC-26 prevê rusqlite 0.41.x, a integrar no Gate B |
| Engines | Stockfish 19 (release oficial `sf_19`, externo, fora do repositório) e Engine falsa `fake_uci` (de teste) | ADR-0003: nenhum binário embutido |

## SISTEMAS_TESTADOS

| Sistema | SO | CPU | GPU / adaptador | Driver | Resultado |
|---|---|---|---|---|---|
| Linux (físico) | Ubuntu 26.04, kernel 7.0.0-38-generic | AMD Ryzen 7 3700X (16 threads), 128.688 MiB RAM | NVIDIA GeForce RTX 4080 SUPER, Vulkan, DiscreteGpu, 16.376 MiB VRAM | 595.99.02 | Compila, testa, janela abre, capturas e benchmark gerados |
| Windows (VM QEMU) | PENDENTE (usuário) | PENDENTE (usuário) | PENDENTE (usuário): adaptador e backend (DX12 WARP, Vulkan ou GL) | PENDENTE (usuário) | PENDENTE (usuário) |
| macOS (VM QEMU) | PENDENTE (usuário) | PENDENTE (usuário) | PENDENTE (usuário): Metal ausente na VM, janela pode não abrir | PENDENTE (usuário) | PENDENTE (usuário) |

Campos a preencher por plataforma de VM:

| Campo | Windows | macOS |
|---|---|---|
| Versão do SO, arquitetura, hipervisor e versão | PENDENTE (usuário) | PENDENTE (usuário) |
| `rustc --version` | PENDENTE (usuário) | PENDENTE (usuário) |
| `cargo build --workspace` compilou? | PENDENTE (usuário) | PENDENTE (usuário) |
| `cargo test --workspace`: total / aprovados / falhas | PENDENTE (usuário) | PENDENTE (usuário) |
| A janela abriu? | PENDENTE (usuário) | PENDENTE (usuário) |
| Adaptador e backend (`AdapterInfo`, `WGPU_BACKEND` usado) | PENDENTE (usuário) | PENDENTE (usuário) |
| `--screenshot` gerou PNGs? | PENDENTE (usuário) | PENDENTE (usuário) |
| Erros literais encontrados | PENDENTE (usuário) | PENDENTE (usuário) |

Observação (DEC-25): VM sem GPU acelerada não gera evidência de FPS; os campos acima valem como prova de compilação, de testes de domínio e de compatibilidade de backend. A EULA do macOS em QEMU é **risco aceito pelo usuário** (DEC-25). Instruções: `docs/build/windows.md` e `docs/build/macos.md`.

## ITENS_IMPLEMENTADOS

Escopo do Gate A (A1 a A6, mais o spike UCI e o núcleo que o Gate A usou). `PASS` exige teste ou evidência; `UNVERIFIED` diz o que falta.

| ID | Status | Evidência ou motivo |
|---|---|---|
| CORE-02 (perft da posição inicial d1–4) | PASS | `perft_start_position_depth_1_to_4`; d5 e d6 também batem (PR #63) |
| Perft Kiwipete e posições 3 a 5 | PASS | Valores do chessprogramming wiki (PR #63) |
| CORE-03/04/05 (roque, en passant, promoção) no nível do núcleo | PASS | 25 testes de lances (PR #63) |
| CORE-06 (afogamento exige estar fora de xeque) | PASS | Testes de `status` (PR #65, B1) |
| DB-02 (FEN ida e volta; FEN inválido não causa panic) | PASS | Testes de FEN (PR #63); persistência em SQLite não faz parte do Gate A |
| CORE-08 (Engine não altera a posição com lance ilegal), nível do adaptador | PASS | `search_legal_move` valida com o núcleo (PR #64); integração com o app: UNVERIFIED |
| UCI-02 (timeout e crash não travam nem danificam), nível do adaptador | PASS | 20 testes com Engine falsa: handshake, `readyok`, busca, crash e `quit` (PR #64); interface sem travar no app: UNVERIFIED |
| UCI-03 (Engine ilegal é rejeitada, erro recuperável), nível do adaptador | PASS | `IllegalMove` e `MalformedMove` recuperáveis, posição intacta (PR #64) |
| UCI-01 (handshake e jogo offline) | UNVERIFIED | Provado por CLI `uci-probe` com Stockfish 19 (40 meios-lances, incluindo roque); jogo dentro do aplicativo ainda não existe |
| CAM-03 (rotação e inversão não alteram a lógica de a1–h8) | PASS | `inverting_the_view_does_not_change_logical_interpretation` (PR #62) |
| CAM-04 (64 casas selecionáveis nas 4 Vistas) | PASS | Testes de raio nas 64 casas x 4 Vistas, em 4 pontos por casa (PR #62); clique real em janela: UNVERIFIED |
| UX-06 (picking correto após qualquer rotação) | PASS | 72 yaws x 4 pitches (PR #62); mesma ressalva de janela real |
| CAM-01 (botão central orbita e não move peça) | UNVERIFIED | Garantia estrutural (só `MouseButton::Left` altera a casa clicada); sem teste automatizado e sem exercício em janela real |
| CAM-02 (pan, zoom, reset, Vistas) | UNVERIFIED | Limites e matemática testados; mapeamento de botões, teclas e gestos só manual. Pinça e dois dedos só existem em macOS físico (winit) |
| VIS-01 (3D profissional coerente com a referência) | UNVERIFIED | Capturas A5 existem; aguardam a aprovação visual do usuário. Peças são de mármore, não buxo/ébano |
| VIS-02 (Mesa imersiva discreta, desempenho medido) | UNVERIFIED | Mesa sem sala (DEC-04) e 3,2 a 4,4 ms/quadro no RTX 4080 SUPER sem vsync; não medida em GPU integrada nem em VM; sem desfoque; aprovação visual pendente |
| VIS-03 (2D técnico, tabuleiro correto, coordenadas legíveis) | PASS | Conversão cursor-casa testada (a1, h8, centro) e captura `2d-tecnico.png`; legibilidade sujeita à revisão visual do usuário |
| VIS-05 (alternar Apresentações preserva o estado) | UNVERIFIED | Tab alterna sobre a mesma posição de `layout.rs`; sem teste automatizado, e turno/relógio/histórico ainda não existem na UI |
| PERF-03 (teste de 5 min registra FPS, frame time, RAM, VRAM, CPU, GPU, SO, drivers) | UNVERIFIED | O instrumento funciona e há JSON, porém a medição está contaminada (ver PERFORMANCE); repetir no B16 |
| PERF-01, PERF-02, PERF-04 (metas 60/30 FPS e interferência de Engine) | UNVERIFIED | Sem medição válida; sem GPU integrada; sem Engine integrada ao aplicativo |
| Compilação em Windows e macOS (Gate A, §16) | UNVERIFIED | PENDENTE (usuário): builds nas VMs |
| Licenças (ADR-0004) | PASS | `cargo deny check licenses` ok em todos os PRs; ver LICENCAS |

## ITENS_PENDENTES

Fora do escopo do Gate A (entram no Gate B ou depois), sem avaliação agora: INT-01 a INT-05 (toque, arrasto e preview), FIDE e arbitragem, relógio, persistência SQLite, PGN, UCI-04 e UCI-05/06, treino. A Partida Assistido no domínio (PR #69) existe, mas não foi avaliada neste relatório.

Pendentes do Gate A por depender do usuário:

- Build e testes em Windows (VM) e macOS (VM): campos `PENDENTE (usuário)`.
- Aprovação visual de VIS-01 e VIS-02.
- Aprovação do GO/NO-GO.
- Teste manual em janela real de CAM-01, CAM-02 e clique nas 64 casas.
- Linha de base válida (repetir em sessão desbloqueada e máquina ociosa) e medição em GPU integrada.

## TESTES_EXECUTADOS

Linux, commit `6c6e032f94b9`:

| Comando | Total | Aprovados | Falhas | Observação |
|---|---|---|---|---|
| `cargo test --workspace` | 203 | 203 | 0 | 3 testes `#[ignore]` não contam (perft profundo, posição morta por bloqueio, Stockfish real); inclui 2 doctests |
| `cargo test -p chess-core --release --test perft -- --ignored` | 1 | 1 (PR #63) | 0 | d5 e d6 (PR #63: passou em cerca de 1,5 s) |
| `cargo test -p chess-uci --test stockfish -- --ignored` | 1 | 1 | 0 | Stockfish 19 real, 2,92 s (PR #64) |
| `cargo clippy --workspace --all-targets -- -D warnings` | | | | sem avisos nos PRs |
| `cargo fmt --all --check` | | | | ok nos PRs |
| `cargo deny check licenses` | | | | `licenses ok` nos PRs |

Windows: PENDENTE (usuário). macOS: PENDENTE (usuário).

## PERFT

Posição inicial (CORE-02): d1 = 20; d2 = 400; d3 = 8.902; d4 = 197.281; d5 = 4.865.609; d6 = 119.060.324.

| Posição | d1 | d2 | d3 | d4 |
|---|---|---|---|---|
| Kiwipete | 48 | 2.039 | 97.862 | 4.085.603 |
| Posição 3 | 14 | 191 | 2.812 | 43.238 |
| Posição 4 | 6 | 264 | 9.467 | 422.333 |
| Posição 5 | 44 | 1.486 | 62.379 | 2.103.487 |

Profundidades maiores (Kiwipete d5 = 193.690.690; posição 3 d6 = 11.030.083; posição 4 d5 = 15.833.292; posição 5 d5 = 89.941.194) estão em testes `#[ignore]`. Todos os valores coincidem com a referência pública (chessprogramming wiki).

## FIDE

Fora do Gate A. Já existem no núcleo: afogamento, mate, material insuficiente (art. 5.2.2), `can_checkmate` conservador e limiares dos 50/75 lances (PR #65). Posição morta por bloqueio de peões não é detectada (documentado). Versão normativa e cenários BDD: Gate B.

## PERSISTENCIA

Fora do Gate A. FEN ida e volta e FEN inválido já têm testes (DB-02 parcial). SQLite, migrações, retomada e PGN: Gate B.

## ENGINES

- Adaptador UCI com processo filho, timeouts por etapa, detecção de crash por EOF, `stop` com prazo, kill, log limitado e validação de cada Lance pelo `chess-core`. Nenhum binário embutido.
- Engine falsa (`fake_uci`): 11 modos de falha e 20 testes de adaptador, cerca de 0,6 s.
- Stockfish 19 real: `real_stockfish_handshake_search_stop_and_self_play` passou (2,92 s); `uci-probe` jogou 40 meios-lances a 50 ms, com `e1g1` e `e8g8` validados pelo núcleo.
- Uso de CPU e GPU com Engine ativa: UNVERIFIED (o benchmark só mediu `--engine ociosa`; a Engine ainda não está integrada ao aplicativo).

## PERFORMANCE

Primeira linha de base (PR #70, commit `37c380a8a304`), Linux, RTX 4080 SUPER, Vulkan, 1920x1080 (janela 1920x1020), preset Equilibrado, Engine ociosa, 5 min (297 s medidos, 3 s de aquecimento). Arquivos: `docs/evidencias/a6/baseline-linux-rtx4080s.json` e `baseline-linux-rtx4080s-sem-vsync.json`.

| Medida | Com vsync | Sem vsync |
|---|---|---|
| Quadros | 5.937 | 66.246 |
| FPS médio | 19,99 | 223,04 |
| Tempo de quadro médio / p50 / p95 / p99 | 50,03 / 49,97 / 55,21 / 62,43 ms | 4,48 / 3,41 / 10,01 / 15,98 ms |
| FPS 1% low | 13,82 | 40,71 |
| RAM do processo (máx.) | 679,7 MiB | 842,3 MiB |
| CPU do processo (média / máx.) | 17,1% / 19,9% | 167,7% / 254,5% |
| VRAM usada (média / máx.) | 10.757 / 15.579 MiB | 14.354 / 15.895 MiB |
| Utilização da GPU (média / máx.) | 11,7% / 35% | ver JSON |

**RESSALVA: medição contaminada.** A sessão gráfica estava bloqueada, o compositor limitou o vsync a 20 Hz (50 ms por quadro) e outros builds rodavam na máquina. Os números **não** representam o desempenho do app e **não** fecham PERF-01 a PERF-04. A VRAM reportada inclui outros processos (os 10 a 15 GiB não são do Chess Studio). **Recomendação: repetir** com a sessão desbloqueada, janela visível e máquina ociosa (tarefa B16), em Equilibrado 1080p e Econômico 720p, com e sem Engine ativa, e em GPU integrada.

Medição indicativa das capturas A5 (1080p, sem vsync, MSAA 4x, perfil dev otimizado; `docs/evidencias/a5/desempenho.txt`): 3,17 a 4,42 ms/quadro nas Apresentações 3D e 1,83 ms no 2D técnico (226 a 546 quadros/s). Também não é medição formal.

Windows e macOS: sem medição (DEC-25: FPS em VM não vale como evidência).

## VISUAL

Capturas 1920x1080 em `docs/evidencias/a5/` (geradas com `--screenshot`), todas em Linux/RTX 4080 SUPER:

- 3D profissional: `3d-profissional-padrao.png`, `-obliqua.png`, `-superior.png`
- Mesa imersiva: `mesa-imersiva-padrao.png`, `-obliqua.png`, `-superior.png`
- 2D técnico: `2d-tecnico.png`

Picking das 64 casas: provado por testes de geometria (ver CAM-04 e UX-06), não por vídeo. Não há vídeo. A revisão visual (VIS-01, VIS-02) aguarda o usuário. Desvios conhecidos: peças de mármore, sem desfoque, sem IBL; no 2D a câmera é fixa e a Vista invertida não vira o tabuleiro. Capturas em Windows e macOS: PENDENTE (usuário).

## LICENCAS

- Projeto: MIT OR Apache-2.0 (ADR-0004).
- Bibliotecas: `cargo deny check licenses` passa; `deny.toml` permite apenas MIT, MIT-0, Apache-2.0 (+LLVM-exception), BSD-2/3, ISC, Zlib, Unicode-3.0, CC0-1.0, BSL-1.0 e OFL-1.1. GPL, AGPL e LGPL são negadas por ausência. MIT-0 foi aceito por transitivas do Bevy.
- Assets (8,8 MB, registro em `crates/chess-studio/assets/LICENSES.md`): modelo das peças, texturas de tabuleiro e mesa do Poly Haven, **CC0**; fonte de glifos Noto Sans Symbols 2 (subconjunto U+2654–265F), **OFL 1.1**.
- Engines: nada distribuído. Stockfish é GPL e é usado como processo externo baixado do site oficial; qualquer distribuição futura exige consentimento, aviso de licença e link da fonte (DEC-27).

## DECISOES_PENDENTES

| Questão | Proposta | Impacto |
|---|---|---|
| GO/NO-GO para o Gate B | Ver RESULTADO | Libera o Gate B |
| Materiais: peças de mármore (Poly Haven) em vez de buxo e ébano | Aceitar no Gate A; buscar Staunton de madeira no Gate C | Visual (VIS-01) |
| Linha de base válida e máquinas de referência (PEND-03) | Repetir no B16 em hardware físico, registrando a especificação | PERF-01 a PERF-04 |
| Rusqlite ainda fora do lockfile | Integrar no Gate B (DEC-26) | Persistência |

## RISCO_RESIDUAL

| Risco | Severidade | Mitigação | Dono |
|---|---|---|---|
| Compatibilidade de wgpu em Windows e macOS não comprovada | Alta até as VMs reportarem | Preencher os campos das VMs; homologar em hardware físico | Usuário |
| macOS em QEMU: EULA e ausência de Metal | Média; **risco aceito pelo usuário (DEC-25)** | VM só para build e testes de domínio; janela e desempenho no Mac físico | Usuário |
| Windows em VM sem GPU (WARP ou software) | Média | Testar `WGPU_BACKEND=dx12|vulkan|gl`; validar em máquina Windows física | Usuário |
| Linha de base de desempenho contaminada | Média | Repetir no B16 com máquina ociosa; nenhuma meta de FPS declarada | Engenharia |
| Sem medição em GPU integrada (PERF-02) | Média | Medir na máquina com GPU integrada | Usuário |
| Interação (CAM-01/02, clique real) não exercitada em janela | Média | Roteiro manual com sessão desbloqueada | Usuário |
| Gestos de trackpad só em macOS físico | Baixa | Validar no Mac real | Usuário |
| Aparência divergente da referência (mármore, sem desfoque) | Baixa | Revisão visual do usuário | Usuário |
| Posição morta por bloqueio de peões não detectada | Baixa | Já documentada; tratar no Gate B | Engenharia |
| Engine ainda não integrada ao aplicativo | Baixa | Adaptador provado; integração no Gate B | Engenharia |

```text
RESULTADO: PENDENTE
```

Proposta de engenharia (não é decisão): **GO condicional** para o Gate B se as VMs compilarem e passarem nos testes e não houver bloqueador de backend sem mitigação no Windows. Falha de janela em VM sem GPU, isoladamente, não deve ser NO-GO (DEC-25). Se o Windows ou o macOS não compilarem, **REPLANEJAR**. A aprovação é do usuário.

## Como completar

1. Siga `docs/build/windows.md` na VM Windows e `docs/build/macos.md` na VM macOS.
2. Em cada VM, preencha a tabela "Campos a preencher por plataforma de VM" e a linha da VM em SISTEMAS_TESTADOS.
3. Atualize TESTES_EXECUTADOS com total, aprovados e falhas por plataforma. Anexe `test-windows.log` ou `test-macos.log` e 1 ou 2 PNGs de `--screenshot` em `docs/evidencias/gate-a-windows/` e `gate-a-macos/`.
4. Mude o item "Compilação em Windows e macOS" para PASS ou FAIL, com o motivo.
5. Revise VIS-01, VIS-02 e as capturas; marque PASS ou FAIL.
6. Troque `STATUS` por PASS, FAIL ou BLOCKED e `RESULTADO` por GO, NO-GO ou REPLANEJAR (aprovação do usuário).
7. Faça o merge deste PR e feche a issue #24.

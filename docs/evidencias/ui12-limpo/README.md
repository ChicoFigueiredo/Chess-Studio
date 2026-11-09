# UI-12: medição limpa (09/11/2026)

Máquina ociosa (nenhum agente compilando): AMD Ryzen 7 3700X (16 threads), 128 GiB, NVIDIA RTX 4080 SUPER (driver 620.70.06), Ubuntu 26.04, Vulkan, **vsync** (teto de 60 FPS). Binário release da `main` em `7fddcf3` (P0 + V1 + V2, com #137 e #138). Roteiro: `scripts/benchmark_matriz.sh --minutes 5` (cada cenário mede 297 s após 3 s de aquecimento), Análise com Stockfish 19. Janela 1920x1020 no "1080p" (decoração da janela).

| Cenário | FPS médio | p95 (ms) | 1% low | RAM máx. (MiB) | CPU do processo | VRAM máx. (MiB) | GPU média | Carga/núcleo máx. | Máquina ocupada |
|---|---|---|---|---|---|---|---|---|---|
| Dock, Equilibrado 1080p | 60,0 | 17,7 | 50,0 | 791 | 74% | 2.330 | 37% | 0,63 | não |
| Dock + HUD, Equilibrado 1080p (repetição) | 60,0 | 17,6 | 53,4 | — | — | — | — | 0,19 | não |
| Dock + HUD + Análise (Stockfish), Equilibrado 1080p | 60,0 | 17,8 | 49,5 | 738 | 78% | 2.383 | 31% | 0,48 | não |
| ↳ fase sem análise | 60,0 | 17,6 | 51,6 | | | | | | |
| ↳ fase com Stockfish | 60,0 | 17,9 | 47,9 | | | | | | |
| Dock, Econômico 720p | 60,0 | 17,6 | 52,4 | 770 | 74% | 2.328 | 32% | 0,44 | não |
| Dock + HUD, Econômico 720p | 60,0 | 17,5 | 52,0 | 749 | 78% | 2.211 | 30% | 0,12 | não |
| Dock + HUD + Análise (Stockfish), Econômico 720p | 60,0 | 17,5 | 51,8 | 717 | 78% | 2.210 | 28% | 0,33 | não |
| ↳ fase sem análise | 60,0 | 17,4 | 53,1 | | | | | | |
| ↳ fase com Stockfish | 60,0 | 17,5 | 50,7 | | | | | | |

**Primeira rodada do Dock + HUD 1080p** (`dock-hud-equilibrado-1920x1080.json`): 51,3 FPS médio, 1% low 28,8. O run ficou em 60 FPS por 210 s e caiu para ~31 FPS (metade do vsync) nos últimos 90 s, em degrau, com a máquina ociosa (carga/núcleo 0,10). É o padrão de janela que perde foco ou fica coberta durante o run; a repetição isolada (`-r2`) ficou em 60 FPS do início ao fim. Fica registrada, não descartada.

**Leitura:** com a máquina livre, dock, HUD e Análise com Stockfish ficam no teto do vsync em Equilibrado 1080p e em Econômico 720p; a Análise custa ~2 a 4 pontos de 1% low e nada de FPS médio. O baseline pré-dock não foi repetido (o binário antigo não aceita `--hud`/`--quality`).

**Ainda não coberto:** GPU integrada, Windows e macOS, e a meta numérica formal do UI-12 (decisão do usuário). Por isso o UI-12 continua UNVERIFIED na matriz.

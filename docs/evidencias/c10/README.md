# Evidências C10 (Lc0 em GPU com controle de carga)

Máquina: Ryzen 7 3700X (16 threads), RTX 4080 SUPER (driver 595.99.02, Vulkan), Linux, 1920x1020, vsync, qualidade Equilibrada (meta 60 FPS). Lc0 0.32.1 compilado localmente (backend CUDA, `cuda-auto`, em `~/.cache/chess-studio-dev/lc0/`, sem commitar binário), rede T1-512x15 do catálogo (hash conferido), Stockfish 19. Cada execução: `--minutes 2.15 --engine ativa`, 3 fases de ~42 s (sem análise, Stockfish, Lc0); as 2 s iniciais de cada fase de análise ficam fora das estatísticas.

**A máquina estava muito ocupada** (outros agentes compilando em paralelo): média de carga de 1 min entre 14 e 52 em 16 threads, e 10,5 GiB de VRAM já em uso por outros processos. Até a fase de referência perde FPS (50 a 60). Leia os números como comparação **dentro de cada execução**; não são um baseline limpo.

| Execução | Fase | FPS médio | p95 (ms) | 1% low | VRAM MiB | GPU % (méd/máx) | Níveis do governador (s) |
|---|---|---|---|---|---|---|---|
| maximo | sem análise | 57,2 | 26,9 | 24,0 | 10841 | 28/51 | n/a |
| maximo | Stockfish | 42,3 (-26%) | 42,1 | 15,1 | 10841 | 30/48 | pleno 41 |
| maximo | Lc0 cuda-auto | 47,5 (-17%) | 35,5 | 18,7 | 12209 | 52/80 | pleno 42 |
| equilibrado | sem análise | 54,6 | 28,0 | 19,1 | 10842 | 31/50 | n/a |
| equilibrado | Stockfish | 50,4 (-8%) | 33,0 | 19,4 | 10842 | 30/51 | pleno 8, reduzido 18, mínimo 3, intermit. 3, pausado 10 |
| equilibrado | Lc0 cuda-auto | 53,0 (-3%) | 31,0 | 21,4 | 12252 | 22/71 | pleno 6, reduzido 14, mínimo 3, intermit. 3, pausado 15 |
| leve | sem análise | 50,1 | 31,4 | 19,3 | 10842 | 29/52 | n/a |
| leve | Stockfish | 52,3 | 30,6 | 17,6 | 10842 | 31/56 | reduzido 27, mínimo 2, intermit. 3, pausado 10 |
| leve | Lc0 cuda-auto | 51,3 | 32,6 | 19,2 | 12282 | 24/52 | reduzido 26, mínimo 3, intermit. 2, pausado 10 |

Leitura: sem controle (Máximo) a análise custa 17 a 26% do FPS; com o governador (Equilibrado/Leve) a perda cai para perto de zero, ao custo de profundidade (Lc0: ~20 mil nós/s em Máximo, ~8 a 10 mil nos modos reduzidos). O Lc0 usa cerca de 1,4 GiB de VRAM e eleva a utilização da GPU. Como a máquina estava saturada, o governador também recuou por causa de carga alheia (a regra "pausa que não melhora" devolve a análise reduzida após 8 s). Repetir em máquina ociosa antes de usar como baseline de PERF-04.

Arquivos: `maximo/`, `equilibrado/`, `leve/` (`benchmark.json`, `frame_times.csv`, `samples.csv`); `analise-lc0-gpu-cuda-3d.png` mostra o painel Análise com o Lc0 v0.32.1 (CUDA) calculando.

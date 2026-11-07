# Lc0 em GPU com controle de carga (C10, issue #52)

Complementa o [design de segurança das Engines](decisoes/seguranca-engines.md) e o [ADR-0003](adr/0003-engines-externas-download-fixado.md): o app nunca embute o Lc0; baixa (Windows) ou detecta (Linux/macOS) e valida por handshake UCI.

## Como obter o Lc0 em cada sistema

| Sistema | Como | O app faz |
|---|---|---|
| Windows x86-64 | Gerenciador de Engines: builds oficiais v0.32.1 (CPU OpenBLAS, CPU oneDNN, GPU DirectML, GPU CUDA 12) | baixa com SHA-256 fixado |
| Linux | O projeto não publica binário. Pacote da distribuição (quando existir) ou compilar: `meson setup build --buildtype=release -Dblas=false -Dplain_cuda=true` (NVIDIA) ou `-Dblas=true` (CPU) e `ninja -C build lc0`; deixar `lc0` no `PATH`, em `~/.local/bin`, `/usr/local/bin`, `/opt/lc0/bin` ou `~/lc0/build/release`, ou apontar `CHESS_STUDIO_LC0=/caminho/lc0` | "Procurar no computador" acha o `lc0` (handshake UCI antes de registrar) |
| macOS | `brew install lc0` (Metal) ou compilar | idem (`/opt/homebrew/bin`, `/usr/local/bin`, `~/lc0/build/release`) |

A rede neural vem do catálogo em qualquer sistema: no formulário da Engine, "Baixar a rede neural recomendada" (consentimento por ação, hash conferido, só do host oficial) e depois escolher a rede e salvar.

## Backends

O Lc0 anuncia no handshake os backends da **build instalada** (`option name Backend type combo ...`). A escolha automática só considera o que foi anunciado; o usuário pode escolher outro no painel Análise ou no formulário da Engine.

| Backend | Onde | Exige | Ordem automática |
|---|---|---|---|
| `cuda-auto`, `cuda-fp16`, `cuda` | Windows, Linux | GPU NVIDIA + driver CUDA | 1o com NVIDIA (Windows/Linux) |
| `cudnn-auto`, `cudnn-fp16`, `cudnn` | Windows, Linux | NVIDIA + CUDA + cuDNN | depois dos CUDA |
| `onnx-cuda`, `onnx-trt` | Windows, Linux | NVIDIA + onnxruntime (CUDA/TensorRT) | depois dos cuDNN |
| `onnx-dml` | Windows | GPU DirectX 12 + `directml.dll` | 1o em GPU não NVIDIA; depois do CUDA em NVIDIA |
| `metal` | macOS | Apple Silicon | 1o no Mac com Apple Silicon |
| `blas`, `eigen`, `onnx-cpu` | todos | nada | CPU, última opção e destino do recuo |

Linux com AMD/Intel: nenhum backend de GPU é automático (OpenCL só por escolha manual). Falha de backend (sem driver, biblioteca ausente, memória): o Lc0 só carrega a rede na primeira busca, então o app faz uma busca de aquecimento (50 ms) ao subir; se falhar, classifica a saída de erro, avisa no painel ("O backend cuda-auto não subiu: ... Usando blas (CPU).") e reinicia em CPU. Sem backend de CPU na build, o erro explica o que fazer.

## Controle de carga (UCI-06)

O governador (`chess_uci::Governor`) lê o p95 do tempo de quadro dos últimos 2 s e compara com a meta (60 FPS em Equilibrada/Ultra, 30 em Econômica):

| Nível | Efeito |
|---|---|
| Pleno | sem limite |
| Reduzido | metade dos threads, `MinibatchSize` 64, `NNCacheSize` 400 mil |
| Mínimo | 1 thread, lote 16, cache 100 mil |
| Intermitente | rajadas de 200 ms de busca e 600 ms de pausa (`stop` + retomar) |
| Pausado | análise parada até o FPS voltar |

Modos do usuário: **Máximo** (nunca reduz; só avisa), **Equilibrado** (reduz quando o p95 passa de 1,25x a meta por 1 s; solta um nível após 10 s calmos), **Leve** (começa Reduzido, reage a 1,10x em 0,5 s, solta após 20 s). Histerese: espera 2 s depois de cada mudança; recaída rápida alonga a recuperação. Pausa que não melhora o FPS por 8 s (a causa é outra, p.ex. máquina ocupada) devolve a análise reduzida e segura 60 s. Para Stockfish/CPU só o teto de threads tem efeito. O painel mostra "Análise reduzida para manter o FPS" (ou pausada).

## Benchmark com Engine ativa (PERF-04)

```text
chess-studio --benchmark --minutes 2.15 --engine ativa --load equilibrado \
  --engine-path stockfish --engine-path lc0 --network rede.pb.gz [--backend cuda-fp16] --out saida/
```

A medição é dividida em fases iguais: sem análise, depois uma por Engine com o painel Análise ligado de verdade. O `benchmark.json` ganha `analysis` (fases, impacto contra a referência, estado da máquina) e `samples.csv` ganha profundidade, nós/s e nível do governador. Evidências em `docs/evidencias/c10/`.

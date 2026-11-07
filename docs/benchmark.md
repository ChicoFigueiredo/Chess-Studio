# Benchmark (`--benchmark`)

Modo de medição do app: roteiro de câmera determinístico, `benchmark.json`, `frame_times.csv`
e `samples.csv` em `benchmarks/out/<data>/` (ou `--out`). Origem: A6 (#23), C10 (PERF-04) e
C14 (#123).

## Argumentos

| Argumento | Efeito |
|---|---|
| `--benchmark` | liga o modo; sem ele o app roda normalmente |
| `--minutes <n>` | duração (padrão 5); os 3 primeiros segundos são aquecimento |
| `--resolution LARGURAxALTURA` | janela (padrão 1920x1080, fator de escala 1,0) |
| `--quality economico\|equilibrado\|ultra` | qualidade gráfica (MSAA, sombras) via `GraphicsQuality`, aplicada a todas as câmeras; padrão Equilibrado |
| `--hud` | liga o HUD virtual com todos os elementos (perfil Análise) para medir o custo dele |
| `--preset equilibrado\|economico` | legado do A6: `economico` desliga MSAA e sombras da câmera 3D |
| `--engine ativa\|ociosa`, `--engine-path`, `--network`, `--load`, `--backend` | análise real em fases (C10) |
| `--no-vsync` | mede sem o teto do vsync |

## Carga da máquina e `maquina_ocupada`

O `benchmark.json` (`schema_version` 2) traz a seção `machine`, lida na primeira e na última
amostra (janelas de ~1 s):

```json
"machine": {
  "start": { "load_average_1m": 1.35, "system_cpu_percent": 12.0 },
  "end":   { "load_average_1m": 1.52, "system_cpu_percent": 9.5 },
  "cpu_logical_cores": 16,
  "load_per_core_limit": 0.7,
  "system_cpu_limit_percent": 70.0,
  "load_per_core_max": 0.095,
  "maquina_ocupada": false
}
```

- `load_average_1m`: média de carga de 1 min (Linux e macOS); `null` no Windows, que não a tem.
- `system_cpu_percent`: uso de CPU de todo o sistema, em todas as plataformas (no Windows é o
  único sinal).
- **Limiar:** `maquina_ocupada` é `true` se, no início **ou** no fim, a carga por núcleo lógico
  passar de **0,70** ou o uso de CPU do sistema passar de **70%**. Nesse caso o run também
  ganha uma nota em `notes` e o resumo impresso diz "MÁQUINA OCUPADA". Run com a máquina
  ocupada não fecha o UI-12: repita com a máquina ociosa.
- `run.quality` e `run.hud` registram a configuração medida.

## Matriz do UI-12

`scripts/benchmark_matriz.sh` roda baseline x dock x dock+HUD x análise em Equilibrado 1080p e
Econômico 720p (`--dry-run` só lista os comandos):

```sh
cargo build --release -p chess-studio
CHESS_STUDIO_BASELINE=/caminho/do/binario-pre-dock \
  scripts/benchmark_matriz.sh --minutes 5
```

Cenários: `baseline` (binário antigo, só Equilibrado; opcional), `dock` (Engine ociosa),
`dock-hud` (`--hud`) e `analise` (`--hud --engine ativa`).

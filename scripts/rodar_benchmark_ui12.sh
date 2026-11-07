#!/usr/bin/env bash
# Mede o UI-12 (benchmarks com dock/HUD/painéis): roda o benchmark de 5 minutos em três
# configurações e guarda JSON, CSVs e o estado da máquina em docs/evidencias/c13/.
#
#   dock-analise-ativa   binário atual, dock aberto, Análise ligada (--engine ativa:
#                        fase sem análise, depois Stockfish e Lc0 se houver)
#   dock-ociosa          binário atual, dock aberto, Engine ociosa (comparável ao A6)
#   baseline-sem-paineis binário de ANTES do dock (Merge #97), Engine ociosa
#
# O HUD NÃO é ligado por este roteiro: o modo benchmark não grava nem lê o banco e não tem
# argumento de HUD (só o roteiro de capturas, `--hud`), então o perfil de HUD fica desligado.
# O custo do HUD é medido à parte, em modo de captura (`--screenshot ... --hud analysis`),
# com o tempo médio por quadro (etapa 4 abaixo).
#
# Variáveis:
#   BIN         binário atual (padrão: $CARGO_TARGET_DIR/release/chess-studio)
#   BIN_BASE    binário do baseline sem painéis (obrigatório para a terceira medição)
#   STOCKFISH, LC0, REDE   Engines e rede (padrões: ~/.cache/chess-studio-dev/...)
#   MINUTOS     duração de cada medição (padrão 5)
#   OUT         diretório de saída (padrão docs/evidencias/c13)
#   BEVY_ASSET_ROOT=$PWD/crates/chess-studio, se o binário não achar os assets
set -euo pipefail

RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
cd "$RAIZ"
BIN="${BIN:-${CARGO_TARGET_DIR:-target}/release/chess-studio}"
BIN_BASE="${BIN_BASE:-}"
DEV="$HOME/.cache/chess-studio-dev"
STOCKFISH="${STOCKFISH:-$DEV/stockfish/stockfish}"
LC0="${LC0:-$DEV/lc0/build/lc0}"
REDE="${REDE:-$DEV/lc0/networks/t1-512x15x8h-distilled-swa-3395000.pb.gz}"
MINUTOS="${MINUTOS:-5}"
OUT="${OUT:-docs/evidencias/c13}"
export BEVY_ASSET_ROOT="${BEVY_ASSET_ROOT:-$RAIZ/crates/chess-studio}"

mkdir -p "$OUT"

estado() { # $1 = rótulo
  {
    echo "== $1 ($(date -Is))"
    echo "uptime: $(uptime)"
    nvidia-smi --query-gpu=name,driver_version,utilization.gpu,memory.used,memory.total,temperature.gpu --format=csv 2>&1 || true
    echo "processos mais pesados:"
    ps -eo pcpu,pmem,comm --sort=-pcpu | head -6
  } >> "$OUT/maquina.txt"
}

medir() { # $1 = nome, $2 = binário, demais = argumentos
  local nome="$1" bin="$2"; shift 2
  estado "antes de $nome"
  "$bin" --benchmark --minutes "$MINUTOS" --out "$OUT/$nome" "$@" 2>&1 | tee "$OUT/$nome.log"
  estado "depois de $nome"
}

: > "$OUT/maquina.txt"

# 1. Dock aberto, Análise ligada (Equilibrado, como o padrão do app).
args=(--engine ativa --load equilibrado --engine-path "$STOCKFISH")
if [[ -x "$LC0" && -f "$REDE" ]]; then
  args+=(--engine-path "$LC0" --network "$REDE")
fi
medir dock-analise-ativa "$BIN" "${args[@]}"

# 2. Dock aberto, Engine ociosa.
medir dock-ociosa "$BIN" --engine ociosa

# 3. Baseline sem painéis (binário anterior ao dock).
if [[ -n "$BIN_BASE" ]]; then
  medir baseline-sem-paineis "$BIN_BASE" --engine ociosa
fi

# 4. Custo do HUD e do Foco em modo de captura (sem vsync): dock e HUD ligados e desligados.
captura() { # $1 = nome, demais = argumentos
  local nome="$1"; shift
  rm -rf "$OUT/captura-$nome"
  "$BIN" --screenshot "$OUT/captura-$nome" "$@" >/dev/null 2>&1 || true
}
estado "antes das capturas"
captura dock-hud-desligado --hud off
captura dock-hud-analise --hud analysis --moves e2e4,e7e5,g1f3,b8c6,f1c4,g8f6
captura foco-hud-desligado --focus --hud off
captura foco-hud-analise --focus --hud analysis --moves e2e4,e7e5,g1f3,b8c6,f1c4,g8f6
estado "depois das capturas"
# As capturas PNG são descartadas (já existem em docs/evidencias/c3 e c5); fica o desempenho.txt.
find "$OUT" -path "*/captura-*" -name '*.png' -delete

echo "pronto: $OUT"

#!/usr/bin/env bash
# Matriz de benchmark do UI-12 (issue #123): baseline x dock x dock+HUD x análise, em
# Equilibrado 1080p e Econômico 720p. Cada cenário grava benchmark.json (com a carga da
# máquina e o marcador `maquina_ocupada`), frame_times.csv e samples.csv.
#
# Uso:
#   scripts/benchmark_matriz.sh [--minutes N] [--out DIR] [--dry-run]
#
# Variáveis:
#   CHESS_STUDIO_BIN       binário atual (padrão: target/release/chess-studio)
#   CHESS_STUDIO_BASELINE  binário de antes do dock (opcional; sem ele o cenário baseline é pulado)
#   CHESS_STUDIO_ENGINE_ARGS  argumentos extras do cenário de análise (ex.: "--engine-path /x/stockfish")
#
# O baseline usa um binário antigo, que ignora --hud/--quality; por isso ele só vale para a
# qualidade padrão (Equilibrado) e é pulado no Econômico.
# Rode com a máquina ociosa: os JSONs dizem `maquina_ocupada: true` quando não estava.
set -euo pipefail

minutes=5
out="benchmarks/out/matriz-$(date -u +%Y%m%dT%H%M%SZ)"
dry=0
while [ $# -gt 0 ]; do
  case "$1" in
    --minutes) minutes="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    --dry-run) dry=1; shift ;;
    *) echo "argumento desconhecido: $1" >&2; exit 2 ;;
  esac
done

bin="${CHESS_STUDIO_BIN:-target/release/chess-studio}"
baseline="${CHESS_STUDIO_BASELINE:-}"
engine_args="${CHESS_STUDIO_ENGINE_ARGS:-}"

run() { # run <nome> <binário> <args...>
  local name="$1" binary="$2"; shift 2
  echo ">>> $name: $binary --benchmark --minutes $minutes $* --out $out/$name"
  if [ "$dry" -eq 0 ]; then
    "$binary" --benchmark --minutes "$minutes" "$@" --out "$out/$name"
  fi
}

mkdir -p "$out"
for perfil in "equilibrado 1920x1080" "economico 1280x720"; do
  set -- $perfil
  quality="$1"; res="$2"
  tag="${quality}-${res}"
  common=(--quality "$quality" --resolution "$res")
  if [ -n "$baseline" ] && [ "$quality" = "equilibrado" ]; then
    run "baseline-$tag" "$baseline" --resolution "$res" --engine ociosa
  fi
  run "dock-$tag" "$bin" "${common[@]}" --engine ociosa
  run "dock-hud-$tag" "$bin" "${common[@]}" --hud --engine ociosa
  # shellcheck disable=SC2086
  run "analise-$tag" "$bin" "${common[@]}" --hud --engine ativa $engine_args
done

echo "Resultados em $out"
if [ "$dry" -eq 0 ]; then
  grep -H '"maquina_ocupada"' "$out"/*/benchmark.json || true
fi

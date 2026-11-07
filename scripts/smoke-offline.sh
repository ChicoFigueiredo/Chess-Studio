#!/usr/bin/env bash
# Smoke test offline (PLAT-02): roda o binário release SEM rede, com `--screenshot` num diretório
# temporário, e verifica código de saída 0 e as capturas geradas.
#
# Uso: scripts/smoke-offline.sh [caminho-do-binario]
#   padrão: $CARGO_TARGET_DIR/release/chess-studio (ou target/release/chess-studio)
#
# Isolamento de rede (Linux): `unshare -n` (namespace de rede vazio, sem nem o loopback ativo).
# Precisa de namespaces de usuário sem privilégio; se não estiverem disponíveis, tenta
# `unshare -rn` e, por fim, `sudo -n unshare -n`. Sem nenhum deles o script FALHA (não finge
# ter testado offline); alternativa manual: desligar Wi-Fi/cabo ou `docker run --network none`.
set -euo pipefail
cd "$(dirname "$0")/.."

BIN="${1:-${CARGO_TARGET_DIR:-target}/release/chess-studio}"
[ -x "$BIN" ] || { echo "binário não encontrado: $BIN (rode: cargo build --release -p chess-studio)" >&2; exit 2; }
BIN="$(readlink -f "$BIN")"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/chess-studio-smoke.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
OUT="$WORK/capturas"
# Dados do usuário isolados: o teste não toca no banco real e prova que um perfil novo funciona.
export HOME="$WORK/home" XDG_DATA_HOME="$WORK/data"
mkdir -p "$HOME" "$XDG_DATA_HOME"

# Prefixo de isolamento de rede.
if unshare -n true 2>/dev/null; then
  ISOLATE=(unshare -n)
elif unshare -rn true 2>/dev/null; then
  ISOLATE=(unshare -rn)
elif sudo -n unshare -n true 2>/dev/null; then
  ISOLATE=(sudo -n env "HOME=$HOME" "XDG_DATA_HOME=$XDG_DATA_HOME" unshare -n)
else
  echo "FALHA: sem 'unshare -n'; não é possível garantir execução offline neste ambiente." >&2
  exit 3
fi

echo "== verificando que o namespace não tem rede"
if "${ISOLATE[@]}" bash -c 'exec 3<>/dev/tcp/1.1.1.1/53' 2>/dev/null; then
  echo "FALHA: o namespace ainda alcança a rede" >&2
  exit 4
fi
echo "ok: conexão externa recusada dentro do namespace"

echo "== executando $BIN --screenshot $OUT (sem rede)"
set +e
"${ISOLATE[@]}" "$BIN" --screenshot "$OUT" --size 960x540
STATUS=$?
set -e
[ "$STATUS" -eq 0 ] || { echo "FALHA: código de saída $STATUS" >&2; exit 1; }

COUNT=$(find "$OUT" -maxdepth 1 -name '*.png' | wc -l)
[ "$COUNT" -ge 7 ] || { echo "FALHA: $COUNT capturas (esperado >= 7)" >&2; exit 1; }
for f in "$OUT"/*.png; do
  [ -s "$f" ] || { echo "FALHA: captura vazia: $f" >&2; exit 1; }
done
[ -s "$OUT/desempenho.txt" ] || { echo "FALHA: sem desempenho.txt" >&2; exit 1; }
[ ! -e "$XDG_DATA_HOME/ChessStudio/engines" ] || { echo "FALHA: o app baixou/instalou Engine sozinho" >&2; exit 1; }

echo "PASS: saída 0, $COUNT capturas, offline, nenhuma Engine instalada sozinha"
ls -l "$OUT"

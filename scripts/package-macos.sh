#!/usr/bin/env bash
# Empacota o Chess Studio para macOS em dist/: .dmg com "Chess Studio.app" (ícone provisório) e o
# atalho para Aplicativos, gerado pelo cargo-packager. Sem assinatura nem notarização (veja
# docs/instalacao.md, aviso do Gatekeeper).
# Uso, em um Mac: scripts/package-macos.sh [--skip-build]
# Requer cargo-packager (cargo install cargo-packager --locked --version 0.11.8).
set -euo pipefail
cd "$(dirname "$0")/.."

META=$(cargo metadata --no-deps --format-version 1)
VERSION=$(python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="chess-studio"))' <<<"$META")
TARGET_DIR=$(python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])' <<<"$META")
ARCH=$(uname -m)
case "$ARCH" in arm64) ARCH_PKG=aarch64 ;; x86_64) ARCH_PKG=x64 ;; *) ARCH_PKG=$ARCH ;; esac
NAME="chess-studio-$VERSION-macos-$ARCH"

[[ "${1:-}" == "--skip-build" ]] || cargo build --release -p chess-studio
BIN="$TARGET_DIR/release/chess-studio"
[[ -x "$BIN" ]] || { echo "binário não encontrado: $BIN" >&2; exit 1; }
[[ -f THIRD-PARTY-LICENSES ]] || scripts/gerar-licencas.sh
command -v cargo-packager >/dev/null || {
  echo "cargo-packager ausente: cargo install cargo-packager --locked --version 0.11.8" >&2
  exit 1
}

OUT="$PWD/dist/packager"
rm -rf "$OUT"
mkdir -p dist
cargo packager --release -p chess-studio --formats app,dmg --out-dir "$OUT"
mv -f "$OUT/Chess Studio_${VERSION}_${ARCH_PKG}.dmg" "dist/$NAME.dmg"
rm -rf "$OUT"
echo "gerado: dist/$NAME.dmg"
(cd dist && shasum -a 256 ./*.dmg > SHA256SUMS-macos)

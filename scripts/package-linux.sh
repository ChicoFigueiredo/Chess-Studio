#!/usr/bin/env bash
# Empacota o Chess Studio para Linux em dist/: .tar.gz portátil, .deb e AppImage (ícone provisório).
# Uso: scripts/package-linux.sh [--skip-build]
# Requer cargo-packager (versão em PACKAGER_VERSION) e, para o .deb, dpkg-deb.
set -euo pipefail
cd "$(dirname "$0")/.."
PACKAGER_VERSION=0.11.8

META=$(cargo metadata --no-deps --format-version 1)
VERSION=$(python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="chess-studio"))' <<<"$META")
TARGET_DIR=$(python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])' <<<"$META")
ARCH_UNAME=$(uname -m)
case "$ARCH_UNAME" in x86_64) ARCH_DEB=amd64 ;; aarch64) ARCH_DEB=arm64 ;; *) ARCH_DEB=$ARCH_UNAME ;; esac
NAME="chess-studio-$VERSION-linux-$ARCH_UNAME"

[[ "${1:-}" == "--skip-build" ]] || cargo build --release -p chess-studio
BIN="$TARGET_DIR/release/chess-studio"
[[ -x "$BIN" ]] || { echo "binário não encontrado: $BIN" >&2; exit 1; }
[[ -f THIRD-PARTY-LICENSES ]] || scripts/gerar-licencas.sh

rm -rf "dist/$NAME" dist/deb-root
mkdir -p "dist/$NAME"

# --- tar.gz portátil: executável + assets lado a lado ---------------------------------------
install -m755 "$BIN" "dist/$NAME/chess-studio"
cp -r crates/chess-studio/assets "dist/$NAME/assets"
cp LICENSE-MIT LICENSE-APACHE THIRD-PARTY-LICENSES "dist/$NAME/"
cp docs/instalacao.md "dist/$NAME/INSTALACAO.md"
tar -C dist -czf "dist/$NAME.tar.gz" "$NAME"
rm -rf "dist/$NAME"
echo "gerado: dist/$NAME.tar.gz"

# --- .deb e AppImage (cargo-packager): ícone e entrada no menu; assets em /usr/lib/chess-studio --
command -v cargo-packager >/dev/null || {
  echo "cargo-packager ausente: cargo install cargo-packager --locked --version $PACKAGER_VERSION" >&2
  exit 1
}
OUT="$PWD/dist/packager"
rm -rf "$OUT"
cargo packager --release -p chess-studio --formats deb,appimage --out-dir "$OUT"
mv -f "$OUT/chess-studio_${VERSION}_${ARCH_DEB}.deb" "dist/chess-studio_${VERSION}_${ARCH_DEB}.deb"
mv -f "$OUT/chess-studio_${VERSION}_${ARCH_UNAME}.AppImage" "dist/$NAME.AppImage"
rm -rf "$OUT"
echo "gerado: dist/chess-studio_${VERSION}_${ARCH_DEB}.deb"
echo "gerado: dist/$NAME.AppImage"
(cd dist && sha256sum ./*.tar.gz ./*.deb ./*.AppImage > SHA256SUMS)

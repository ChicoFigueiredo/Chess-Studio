#!/usr/bin/env bash
# Empacota o Chess Studio para Linux em dist/: .tar.gz portátil e .deb.
# Uso: scripts/package-linux.sh [--skip-build]   (requer dpkg-deb para o .deb)
set -euo pipefail
cd "$(dirname "$0")/.."

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

# --- .deb: /usr/bin/chess-studio + /usr/share/chess-studio/assets ---------------------------
if command -v dpkg-deb >/dev/null; then
  ROOT=dist/deb-root
  install -Dm755 "$BIN" "$ROOT/usr/bin/chess-studio"
  mkdir -p "$ROOT/usr/share/chess-studio" "$ROOT/usr/share/doc/chess-studio"
  cp -r crates/chess-studio/assets "$ROOT/usr/share/chess-studio/assets"
  cp LICENSE-MIT LICENSE-APACHE THIRD-PARTY-LICENSES "$ROOT/usr/share/doc/chess-studio/"
  install -d "$ROOT/usr/share/applications"
  cat > "$ROOT/usr/share/applications/chess-studio.desktop" <<DESK
[Desktop Entry]
Type=Application
Name=Chess Studio
Comment=Mesa pessoal de treinamento de xadrez
Exec=chess-studio
Terminal=false
Categories=Game;BoardGame;
DESK
  SIZE_KB=$(du -sk "$ROOT" | cut -f1)
  mkdir -p "$ROOT/DEBIAN"
  cat > "$ROOT/DEBIAN/control" <<CTRL
Package: chess-studio
Version: $VERSION
Section: games
Priority: optional
Architecture: $ARCH_DEB
Installed-Size: $SIZE_KB
Depends: libc6, libvulkan1, libxkbcommon0, libudev1
Recommends: mesa-vulkan-drivers
Maintainer: Chico Figueiredo <fran.fig@gmail.com>
Homepage: https://github.com/ChicoFigueiredo/Chess-Studio
Description: Mesa pessoal de treinamento de xadrez
 Tabuleiro 3D e 2D, tres politicas de arbitragem e engine UCI externa.
 Funciona offline, sem login.
CTRL
  find "$ROOT" -type d -exec chmod 755 {} +
  dpkg-deb --root-owner-group --build "$ROOT" "dist/chess-studio_${VERSION}_${ARCH_DEB}.deb" >/dev/null
  rm -rf "$ROOT"
  echo "gerado: dist/chess-studio_${VERSION}_${ARCH_DEB}.deb"
fi
(cd dist && sha256sum ./*.tar.gz ./*.deb > SHA256SUMS)

#!/usr/bin/env bash
# Empacota o Chess Studio para macOS em dist/: "Chess Studio.app" dentro de .tar.gz (e .dmg).
# Sem assinatura nem notarização (veja docs/instalacao.md, aviso do Gatekeeper).
# Uso, em um Mac: scripts/package-macos.sh [--skip-build]
set -euo pipefail
cd "$(dirname "$0")/.."

META=$(cargo metadata --no-deps --format-version 1)
VERSION=$(python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="chess-studio"))' <<<"$META")
TARGET_DIR=$(python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])' <<<"$META")
ARCH=$(uname -m)
NAME="chess-studio-$VERSION-macos-$ARCH"

[[ "${1:-}" == "--skip-build" ]] || cargo build --release -p chess-studio
BIN="$TARGET_DIR/release/chess-studio"
[[ -x "$BIN" ]] || { echo "binário não encontrado: $BIN" >&2; exit 1; }
[[ -f THIRD-PARTY-LICENSES ]] || scripts/gerar-licencas.sh

STAGE="dist/$NAME"
APP="$STAGE/Chess Studio.app"
rm -rf "$STAGE"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
install -m755 "$BIN" "$APP/Contents/MacOS/chess-studio"
cp -r crates/chess-studio/assets "$APP/Contents/Resources/assets"
cp LICENSE-MIT LICENSE-APACHE THIRD-PARTY-LICENSES "$APP/Contents/Resources/"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Chess Studio</string>
  <key>CFBundleDisplayName</key><string>Chess Studio</string>
  <key>CFBundleIdentifier</key><string>io.github.chicofigueiredo.chess-studio</string>
  <key>CFBundleExecutable</key><string>chess-studio</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.board-games</string>
</dict>
</plist>
PLIST
cp docs/instalacao.md "$STAGE/INSTALACAO.md"

tar -C dist -czf "dist/$NAME.tar.gz" "$NAME"
echo "gerado: dist/$NAME.tar.gz"
if command -v hdiutil >/dev/null; then
  hdiutil create -volname "Chess Studio" -srcfolder "$STAGE" -ov -format UDZO "dist/$NAME.dmg" >/dev/null
  echo "gerado: dist/$NAME.dmg"
fi
rm -rf "$STAGE"
(cd dist && shasum -a 256 ./*.tar.gz ./*.dmg > SHA256SUMS-macos 2>/dev/null || true)

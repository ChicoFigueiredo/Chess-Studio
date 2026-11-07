#!/usr/bin/env bash
# Gera o THIRD-PARTY-LICENSES na raiz do repositório (requer: cargo install cargo-about --locked --features cli).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo about generate --manifest-path crates/chess-studio/Cargo.toml -c about.toml about.hbs -o THIRD-PARTY-LICENSES
echo "gerado: THIRD-PARTY-LICENSES"

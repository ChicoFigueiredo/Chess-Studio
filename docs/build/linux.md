# Build e testes no Linux

Referência: Ubuntu 26.04, kernel 7.0, GPU NVIDIA RTX 4080 SUPER (Vulkan). Outras distribuições funcionam com os pacotes equivalentes.

## Pré-requisitos

```bash
# Dependências de sistema do Bevy (X11 + Wayland); ajuste ao gerenciador da distribuição
sudo apt install -y build-essential pkg-config git libx11-dev libxkbcommon-dev \
  libxkbcommon-x11-0 libwayland-dev libudev-dev libasound2-dev libvulkan1 mesa-vulkan-drivers

# Rust via rustup; o rust-toolchain.toml do repositório fixa o canal 1.98 (rustfmt e clippy)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
```

Driver de GPU: o da NVIDIA (referência 595.99.02) ou Mesa. `vulkaninfo --summary` deve listar o adaptador.

## Compilar e testar

```bash
git clone https://github.com/ChicoFigueiredo/Chess-Studio.git
cd Chess-Studio
rustc --version                 # o rustup instala o 1.98.x na primeira execução
cargo test --workspace          # suíte completa (a primeira compilação do Bevy leva vários minutos)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo install cargo-deny --locked && cargo deny check licenses
```

Perft profundo (d5/d6, testes `#[ignore]`):

```bash
cargo test -p chess-core --release --test perft -- --ignored
```

## Executar

```bash
cargo run -p chess-studio                                      # janela; Tab alterna as Apresentações
cargo run -p chess-studio -- --screenshot docs/evidencias/a5   # gera PNGs + desempenho.txt e encerra
cargo run --release -p chess-studio -- --benchmark             # 5 min, 1080p, Equilibrado, vsync
cargo run --release -p chess-studio -- --benchmark --no-vsync --out /tmp/bench
# opções: --minutes N  --resolution 1920x1080  --preset equilibrado|economico  --engine ativa|ociosa
```

Spike UCI, com o Stockfish baixado do site oficial e mantido fora do repositório (ADR-0003):

```bash
cargo run -p chess-uci --bin uci-probe -- --engine /caminho/stockfish --movetime 50 --max-plies 40
CHESS_STUDIO_STOCKFISH=/caminho/stockfish cargo test -p chess-uci --test stockfish -- --ignored
```

## Backend gráfico

`WGPU_BACKEND=vulkan|gl` força o backend. A linha `AdapterInfo` aparece no log (`RUST_LOG=info`) e o JSON do benchmark registra `gpu.name`, `gpu.backend` e `gpu.driver_info`.

## O que anotar no relatório

- SO, kernel, CPU, RAM, GPU, backend (Vulkan) e driver.
- Saída do `cargo test --workspace`: total de testes, aprovados, falhas.
- Se a janela abriu e se as capturas foram geradas.
- O JSON do `--benchmark` (copiar para `docs/evidencias/a6/`).
- Para medir de forma válida: sessão desbloqueada, janela visível, máquina ociosa (sem outros builds).

## Empacotar (B17)

Gera `dist/chess-studio-<versão>-linux-<arch>.tar.gz` (executável + `assets/` + licenças) e `dist/chess-studio_<versão>_<arch>.deb` (`/usr/bin/chess-studio` + `/usr/share/chess-studio/assets`), mais `dist/SHA256SUMS`. Só precisa de `dpkg-deb` para o `.deb`.

```bash
scripts/package-linux.sh                 # compila em release e empacota (--skip-build reaproveita o binário)
mkdir /tmp/cs && tar -C /tmp/cs -xzf dist/chess-studio-*-linux-*.tar.gz
cd /tmp/cs/chess-studio-* && ./chess-studio --screenshot /tmp/cs/cap   # o 3D deve aparecer nos PNGs, não preto
```

O app acha os `assets/` (nesta ordem) por `CHESS_STUDIO_ASSETS`, `./assets` ao lado do executável, `../share/chess-studio/assets` e, por último, a pasta do código-fonte. Para regenerar `THIRD-PARTY-LICENSES` (após mudar dependências): `cargo install cargo-about --locked --features cli && scripts/gerar-licencas.sh`.

Teste sem rede (PLAT-02): `unshare -rn ./chess-studio` (sem interfaces de rede) ou desligue a rede e jogue; veja `docs/instalacao.md`.

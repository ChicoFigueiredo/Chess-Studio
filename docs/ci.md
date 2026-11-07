# CI/CD

Implementa a issue #56 (DEC-25: CI depois que o Linux estava sólido). Workflows em `.github/workflows/`.

## Jobs

| Workflow | Job | O que faz |
|---|---|---|
| `ci.yml` | `linux` (obrigatório) | matriz P0 (`--check`), `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`, `cargo deny check licenses` |
| `ci.yml` | `build-test` (Windows, macOS) | testes de domínio (`chess-core`, `-game`, `-store`, `-uci`, `-pgn`, `-training`, `-input`) e build do `chess-studio` |
| `release.yml` | `linux`, `windows`, `macos`, `draft-release` | empacota com `scripts/package-*`, gera `SHA256SUMS` e `THIRD-PARTY-LICENSES` e anexa a um **draft release** (nunca publica sozinho) |

Execuções antigas do mesmo branch são canceladas (`concurrency`). O cache de cargo usa `Swatinem/rust-cache`.

## Sem GPU no CI

O job Linux roda os testes dentro de `xvfb-run` (display virtual) com `mesa-vulkan-drivers` (lavapipe, Vulkan por software). Nenhum teste é removido. Windows e macOS rodam só os crates de domínio (sem janela/GPU) e apenas compilam o app: a homologação visual e de desempenho (VIS/PERF) é em hardware físico (DEC-25).

## Como pular

- Commit só de docs: inclua `[skip ci]` na mensagem.
- Release: só roda em tag `v*` ou manualmente (Actions > Release > Run workflow, informando a tag).

## Release

```bash
git tag v0.1.0 && git push origin v0.1.0   # ou dispare manualmente
```

O resultado é um draft em Releases; revise e publique à mão. macOS sem assinatura/notarização (veja `docs/instalacao.md`).

## Reproduzir localmente

```bash
python3 scripts/gerar_matriz_p0.py --check
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
xvfb-run -a cargo test --workspace      # ou sem xvfb-run, em máquina com display
cargo deny check licenses
scripts/package-linux.sh                # pacotes em dist/
```

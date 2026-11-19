# CI/CD

Implementa a issue #56 (DEC-25: CI depois que o Linux estava sólido). Workflows em `.github/workflows/`.

## Jobs

| Workflow | Job | O que faz |
|---|---|---|
| `ci.yml` | `linux` (obrigatório) | matriz P0 (`--check`), `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`, `cargo deny check licenses` |
| `ci.yml` | `build-test` (Windows, macOS) | testes de domínio (`chess-core`, `-game`, `-store`, `-uci`, `-pgn`, `-training`, `-input`) e build do `chess-studio` |
| `release.yml` | `linux`, `windows`, `macos`, `release` | empacota com `scripts/package-*` (instaladores pelo `cargo-packager`: `-setup.exe` NSIS, `.dmg`, `.deb` e AppImage; mais os portáteis `.zip` e `.tar.gz`), gera `SHA256SUMS` e `THIRD-PARTY-LICENSES` e anexa a um **draft release**; só publica se disparado à mão com `publicar` marcado |

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

Por tag ou sem `publicar`, o resultado é um draft em Releases; revise e publique à mão. Disparado à mão com `publicar` marcado, o release sai público (tag com `-`, como `v0.1.0-rc1`, vira pre-release). O campo `alvo` (SHA) cria a tag em outro commit, desde que o código fora de `.github/` seja idêntico ao compilado; serve para rodar uma versão nova do workflow a partir de um branch e marcar o commit da `main`. macOS sem assinatura/notarização (veja `docs/instalacao.md`).

## Reproduzir localmente

```bash
python3 scripts/gerar_matriz_p0.py --check
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
xvfb-run -a cargo test --workspace      # ou sem xvfb-run, em máquina com display
cargo deny check licenses
cargo install cargo-packager --locked --version 0.11.8   # instaladores (uma vez)
scripts/package-linux.sh                # pacotes em dist/ (.tar.gz, .deb, AppImage)
```

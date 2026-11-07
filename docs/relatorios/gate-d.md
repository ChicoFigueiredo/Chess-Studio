# Relatório do Gate D — Homologação e liberação (release)

Formato da §19 do prompt mestre. Escopo: issue #57 (D2) e a seção "Gate D" do prompt mestre. Este relatório junta a evidência de licenças, instaladores, atualização e recuperação, smoke offline e CI. Tudo que depende de VMs, de hardware físico ou de julgamento do usuário está marcado `PENDENTE (usuário)`; enquanto existir, `STATUS` e `RESULTADO` ficam `PENDENTE`. Veja "O que depende do usuário" no fim.

```text
CHESS_STUDIO_GATE: D (homologação e liberação; issue #57)
STATUS: PENDENTE (Linux sem bloqueadores conhecidos; Windows/macOS, release em andamento, benchmark limpo e homologação visual não verificados)
REPO / BRANCH / COMMIT: ChicoFigueiredo/Chess-Studio / main / 9caed02e6b31 (#126, C14); este relatório só acrescenta documentação e scripts
```

## VERSOES

Mesmas do relatório da V1 ([`v1.md`](v1.md)): Rust 1.98 (edição 2024), Bevy 0.20.1, bevy_egui 0.43.1, egui 0.36.2, wgpu 30.0.1, winit 0.30.13, rusqlite 0.41.1 (`bundled`, esquema 8). Engines (Stockfish 19, Lc0 0.32.1) são externas e nunca embutidas (ADR-0003). Fonte: `Cargo.lock` e `rust-toolchain.toml`.

## SISTEMAS_TESTADOS

| Sistema | Resultado |
|---|---|
| Linux físico (Ubuntu 26.04, Ryzen 7 3700X, RTX 4080 SUPER, Vulkan) | testes, `cargo deny`, smoke offline |
| Linux, CI (ubuntu, xvfb + lavapipe) | job obrigatório `linux` do `ci.yml`; ver CI |
| Windows | CI compila e roda testes de domínio; build/smoke na VM ou máquina real: PENDENTE (usuário) |
| macOS | CI compila e roda testes de domínio; build/smoke, Metal e trackpad: PENDENTE (usuário) |
| GPU integrada (720p Econômico) | PENDENTE (usuário) |

## CRITERIOS_DE_RELEASE

Critérios da issue #57 e do Gate D do prompt mestre. `PASS` exige teste ou evidência; `UNVERIFIED` diz o que falta.

| Critério | Status | Evidência ou motivo |
|---|---|---|
| Auditoria de licenças de crates (ADR-0004) | PASS | `cargo deny check` completo (advisories, bans, licenses, sources) ok; nenhuma GPL/AGPL/LGPL. [`licencas.md`](licencas.md) §1 (PR #124) |
| `THIRD-PARTY-LICENSES` reproduzível | PASS | `scripts/gerar-licencas.sh` (cargo about); o release o regenera. A feature `cli` do cargo-about foi corrigida no PR #125 (a falha anterior era `no such command: about`) |
| Auditoria de licenças de assets | PASS | Todos os binários registrados em `crates/chess-studio/assets/LICENSES.md` (CC0 Poly Haven, OFL das fontes Noto). [`licencas.md`](licencas.md) §2 |
| Auditoria de Engines (ADR-0003) | PASS | Nenhum binário de Engine no repositório nem nos pacotes; download só com consentimento; `tests/no_network_without_consent.rs` (4 testes). [`licencas.md`](licencas.md) §3 |
| Instaladores para os três sistemas: scripts | PASS | `scripts/package-linux.sh`, `package-macos.sh`, `package-windows.ps1`, `.github/workflows/release.yml` (PR #112, D1) |
| Instaladores para os três sistemas: artefatos gerados | UNVERIFIED | Run de release `42339946088` (workflow_dispatch, após #125) ainda em andamento na redação (ver CI). O run anterior `42336357264`: Windows e macOS `success`, Linux `failure` (cargo-about sem `cli`, corrigido em #125), `draft-release` pulado. Falta confirmar o draft com os três pacotes e `SHA256SUMS` |
| Instalação dos pacotes em Windows/macOS | UNVERIFIED | PENDENTE (usuário): instalar e abrir o pacote em cada sistema. macOS sem assinatura/notarização (ver `docs/instalacao.md`) |
| Documentação de instalação | PASS | [`docs/instalacao.md`](../instalacao.md), guias de build [`linux`](../build/linux.md), [`windows`](../build/windows.md), [`macos`](../build/macos.md) |
| Atualização e recuperação documentadas | PASS | [`docs/atualizacao-e-recuperacao.md`](../atualizacao-e-recuperacao.md): pasta de dados por SO, atualização, backup/restauração, banco e Engines corrompidos (PR #124) |
| Smoke test offline (PLAT-02), Linux | PASS | `scripts/smoke-offline.sh` em namespace sem rede: saída 0, 7 PNGs, nenhuma Engine instalada sozinha. [`licencas.md`](licencas.md) §4 |
| Smoke test offline, Windows e macOS | UNVERIFIED | PENDENTE (usuário): o script usa `unshare` (Linux); em Windows/macOS, desligar a rede e jogar 2P, Exercícios e Engine local |
| PLAT-01 (compila e executa nos três sistemas) | UNVERIFIED | Linux ok; Windows e macOS compilam no CI (domínio + build), execução em janela PENDENTE (usuário) |
| Suíte completa (regras, BDD, persistência, UCI) em Linux | PASS | `cargo test --workspace`: 937 aprovados, 0 falhas no commit da V1 ([`v1.md`](v1.md)); job `linux` do CI |
| Suíte de domínio em Windows e macOS | UNVERIFIED | Jobs `build-test` do `ci.yml` rodam os crates de domínio; confirmar verde no commit final (ver CI) |
| Benchmarks reproduzíveis de 5 min, todos os perfis | UNVERIFIED | Instrumento pronto (`--hud`, `--quality`, `--resolution`, `maquina_ocupada`; `scripts/benchmark_matriz.sh`, [`docs/benchmark.md`](../benchmark.md)). A medição UI-12 existente foi feita com a máquina ocupada ([`v1.md`](v1.md)). Falta rodar a matriz com a máquina ociosa e em GPU integrada |
| Auditoria final da matriz de rastreabilidade | PASS (parcial) | P0: 54 PASS, 14 UNVERIFIED, 0 FAIL em 68 IDs (CORE-07 resolvido pelo #93) ([`p0.md`](p0.md), [`matriz-p0.md`](../homologacao/matriz-p0.md)). V1: 29 PASS, 2 UNVERIFIED em 31 IDs ([`v1.md`](v1.md), [`matriz-v1.md`](../homologacao/matriz-v1.md)). As matrizes são geradas e validadas por script; os UNVERIFIED restantes estão em "O que depende do usuário". |
| Relatório de riscos | PASS | seção RISCO_RESIDUAL abaixo |
| GO da V1 (homologação) | UNVERIFIED | `v1.md` termina em `PENDENTE`; aguarda o usuário |
| Proposta de release com versão, notas e evidência | UNVERIFIED | Depende do release em draft e das homologações; nenhuma tag `v*` foi criada |

## CI

| Workflow | Run | Resultado |
|---|---|---|
| `ci.yml`, último com sucesso | [42338149192](https://github.com/ChicoFigueiredo/Chess-Studio/actions/runs/42338149192) (PR #126, `bf3fade`) | success |
| `ci.yml`, `main` após os merges #121 a #126 | runs 42338929949, 42338957478, 42339939655, 42339943616 | `cancelled` pela regra de `concurrency` (merges em sequência); o último, [42340402672](https://github.com/ChicoFigueiredo/Chess-Studio/actions/runs/42340402672) (`9caed02`), estava em andamento na redação |
| `release.yml` | [42339946088](https://github.com/ChicoFigueiredo/Chess-Studio/actions/runs/42339946088) (manual, `main` após #125) | em andamento na redação (linux, windows e macos) |
| `release.yml`, anterior | [42336357264](https://github.com/ChicoFigueiredo/Chess-Studio/actions/runs/42336357264) | failure: Linux sem `cargo about` (corrigido em #125); Windows e macOS success |

Estado atual: `gh run list --workflow ci.yml --branch main` e `gh run list --workflow release.yml`. O release cria sempre um **draft**; nunca publica sozinho ([`docs/ci.md`](../ci.md)). Quem fechar este relatório deve substituir as linhas "em andamento" pelo resultado final e marcar a linha "artefatos gerados" como PASS ou FAIL.

## DECISOES_PENDENTES

| Questão | Proposta | Impacto |
|---|---|---|
| GO da V1 | Ver `v1.md` | Pré-requisito do Gate D |
| GO/NO-GO do Gate D e versão a publicar (sugestão: `v0.1.0`) | Só depois das homologações abaixo | Libera o release |
| Assinatura/notarização do macOS e do instalador do Windows | Fora do escopo; documentado como pacote sem assinatura | Aviso do SO na primeira execução |
| Linha de base de desempenho em máquina de referência (PEND-03) | Rodar `scripts/benchmark_matriz.sh` ociosa e registrar valores reais, sem prometer metas | PERF-01 a PERF-04, UI-12 |

## RISCO_RESIDUAL

| Risco | Severidade | Mitigação | Dono |
|---|---|---|---|
| Windows e macOS só compilam no CI, sem janela nem GPU | Alta até homologar | Instalar o draft nas VMs/máquinas físicas e rodar o smoke | Usuário |
| Release ainda não confirmado como draft completo | Média | Acompanhar o run `42339946088`; reexecutar se falhar | Engenharia |
| Pacotes sem assinatura (aviso do SO) | Baixa | Documentado em `docs/instalacao.md` | Usuário |
| Benchmark UI-12 contaminado por máquina ocupada; sem GPU integrada | Média | `scripts/benchmark_matriz.sh` com máquina ociosa | Usuário |
| F11 real, restauração de janela e atalho interceptado (UI-03) | Média | Roteiro manual em janela real | Usuário |
| `ttf-parser` sem manutenção (RUSTSEC-2026-0192), transitiva do `winit` | Baixa | Ignorado com justificativa em `deny.toml`; reavaliar ao atualizar o Bevy | Engenharia |
| Aprovação visual (VIS-01/02) e gestos de trackpad | Baixa | Revisão do usuário no Mac e no Linux | Usuário |

## O que depende do usuário

1. Builds, testes e smoke nas VMs/máquinas Windows e macOS, seguindo `docs/build/` e `docs/instalacao.md`; instalar os pacotes do draft e abrir o app.
2. Smoke offline fora do Linux (rede desligada: 2P, Exercícios, Engine local).
3. Homologação visual e de interação (VIS-01/02, CAM-01/02, UI-03, UX-09/10) em janela real.
4. Benchmark limpo do UI-12 e dos perfis (máquina ociosa; GPU integrada): `scripts/benchmark_matriz.sh --minutes 5` (ou `scripts/rodar_benchmark_ui12.sh --matriz`).
5. GO da V1 ([`v1.md`](v1.md)) e GO do Gate D; depois, criar a tag `v*`, revisar o draft e publicar.

```text
RESULTADO: PENDENTE (aguarda GO do usuário)
```

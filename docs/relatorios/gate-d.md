# Relatório do Gate D — Homologação e liberação (release)

Formato da §19 do prompt mestre. Escopo: issue #57 (D2) e a seção "Gate D" do prompt mestre. Este relatório junta a evidência de licenças, instaladores, atualização e recuperação, smoke offline e CI. Tudo que depende de VMs, de hardware físico ou de julgamento do usuário está marcado `PENDENTE (usuário)`; enquanto existir, `STATUS` e `RESULTADO` ficam `PENDENTE`. Veja "O que depende do usuário" no fim.

```text
CHESS_STUDIO_GATE: D (homologação e liberação; issue #57)
STATUS: PENDENTE (RC-1 publicado como pre-release `v0.1.0-rc1` em 20/11/2026; Linux sem bloqueadores conhecidos; instalação em Windows/macOS, integrações com serviço real, benchmark limpo e homologação visual não verificados; roteiro do RC-1 com 46 itens aguarda você)
REPO / BRANCH / COMMIT: ChicoFigueiredo/Chess-Studio / main / 0820e535bcf8 (#319, instaladores e release `v0.1.0-rc1`); atualizado em 21/11/2026; este relatório só acrescenta documentação
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
| Instaladores para os três sistemas: artefatos gerados | PASS | **Atual: pre-release `v0.1.0-rc1`** (seção RC-1 abaixo): AppImage, `.deb`, `.tar.gz`, `setup.exe` e `.zip` do Windows, `.dmg` do macOS, `SHA256SUMS` e `THIRD-PARTY-LICENSES`. Histórico: run de release [42362640315](https://github.com/ChicoFigueiredo/Chess-Studio/actions/runs/42362640315) (`v0.0.0-rc3`, `main` em `5d4e00e`): linux, windows, macos e draft-release `success`. Draft com `chess-studio-0.1.0-linux-x86_64.tar.gz`, `chess-studio_0.1.0_amd64.deb`, `chess-studio-0.1.0-windows-x86_64.zip`, `chess-studio-0.1.0-macos-arm64.dmg`, `chess-studio-0.1.0-macos-arm64.tar.gz`, `SHA256SUMS` e `THIRD-PARTY-LICENSES`. Tentativas anteriores: `42336357264` (cargo-about sem `cli`, #125) e `42339946088` (`.deb` em `dist/` no draft, #128) |
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
| V2 entregue (Súmula manual) | PASS (parcial) | V2 está na `main` desde o PR #129 (issues #58 a #60); 3 PASS e 4 UNVERIFIED (visual e decisões do usuário) em [`v2.md`](v2.md) e [`matriz-v2.md`](../homologacao/matriz-v2.md). O draft `v0.0.0-rc3` **não contém** a V2 (nem o B28 e o B29, que vieram depois); o `v0.1.0-rc1` contém a V2 e o V4 |
| GO da V1 (homologação) | UNVERIFIED | `v1.md` termina em `PENDENTE`; aguarda o usuário |
| Proposta de release com versão, notas e evidência | PASS (parcial) | `v0.1.0-rc1` existe como pre-release público (tag no commit `c8aa3d5`; Linux e Windows refeitos de `13fcff4`), com notas e `SHA256SUMS`. O release final `v0.1.0` não existe e só sai com o GO do usuário |

## RC-1 (atualização de 21/11/2026)

Primeiro pacote publicado, em pre-release. Fonte dos números: `gh release view v0.1.0-rc1`, `git diff` entre os commits e o relatório local abaixo.

| Item | Valor |
|---|---|
| Tag e página | [`v0.1.0-rc1`](https://github.com/ChicoFigueiredo/Chess-Studio/releases/tag/v0.1.0-rc1), pre-release, publicado em 20/11/2026 (01:19 UTC), com a V2 e o V4 |
| Origem dos pacotes | `.dmg` do macOS: commit `c8aa3d5`. Linux e Windows: refeitos em 20/11 a partir de `13fcff4` (Linux com glibc 2.39; `.exe` do Windows com o runtime do Visual C++ embutido, `crt-static`) |
| Código na `main` | `0820e53` (#319, squash). Fora de `.github/`, o código é idêntico ao de `13fcff4`; só mudam `.gitignore` e `docs/prompts/anotacoes`. O `.cargo/config.toml` do Windows entrou depois de `c8aa3d5`, por isso o `.exe` só vale a partir do refeito |
| Arquivos | AppImage (44,3 MB), `.deb` (47,4 MB), `.tar.gz` (47,3 MB), `setup.exe` (43,7 MB), `.zip` do Windows (43,5 MB), `.dmg` (42,9 MB), `SHA256SUMS`, `THIRD-PARTY-LICENSES` |
| Assinatura | nenhuma (macOS sem notarização, Windows sem assinatura); ícone provisório |
| Instalado e aberto em janela real | **ninguém** (nem Linux, nem Windows, nem macOS) fora da CI |

### Roteiro de homologação do RC-1

[`docs/homologacao/rc1.md`](../homologacao/rc1.md): 46 itens em 8 grupos (preparar, instalar e rodar sem rede, jogar, interface, treino e Súmula, Provedores sem conta, Provedores com conta de teste, fechamento), cada um com passos, critérios de aceite, campos de registro e o resultado. A mesma lista, marcável, está no artefato **Partida do Chess Studio**, seção Homologação do RC-1. Os 3 itens de risco mais alto, nesta ordem: autorizar o Lichess (RC1-34), cofre (RC1-33) e a conversão em usuário BOT (RC1-42, só com conta nova).

### Build local do RC-1 (21/11/2026)

Do zero (alvos de build apagados antes), no commit `0820e535bcf8` (a `main` depois do #319), em Linux x86_64 (Ubuntu 26.04). O RC-1 que você homologa é este build ou o pre-release `v0.1.0-rc1`, que tem o mesmo código.

| Passo | Resultado |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo deny check licenses` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace` | PASS: 2342 aprovados, 0 falhas, 14 ignorados |
| `cargo build --release -p chess-studio` | PASS |
| `scripts/package-linux.sh` | PASS: `.tar.gz`, `.deb` e AppImage em `dist/` |
| `scripts/smoke-offline.sh` (binário release) | PASS: saída 0, 7 capturas, sem rede, nenhuma Engine instalada sozinha |
| `THIRD-PARTY-LICENSES` regenerado com `cargo about` | igual ao da raiz e ao do release (480357 bytes) |
| `cargo deny check advisories` | não rodado aqui: falha conhecida SEC-09 (#285) |

Arquivos em `dist/` (não versionados; conferidos com `sha256sum -c dist/SHA256SUMS`):

| Arquivo | SHA-256 |
|---|---|
| `chess-studio-0.1.0-linux-x86_64.AppImage` (44,3 MB) | `f0eb47877e6482c6217614d56bfd8e649d9135be93ccd4b68bc0557e056fc078` |
| `chess-studio_0.1.0_amd64.deb` (47,4 MB) | `13e21d5e38c8f785905e88815f34495897e8929bd43fc653a9b732e0de5bbd1e` |
| `chess-studio-0.1.0-linux-x86_64.tar.gz` (47,4 MB) | `8a31d329bddcf9de38dcb571de5373dd8baaaccd799ada50351476d59fd5f635` |

Os hashes não batem com os do release porque o build não é reproduzível byte a byte (outra máquina, outro instante); o que se compara é o código, e ele é o mesmo. Não foram testados em janela real nem em Windows e macOS: isso é o roteiro do RC-1. O AppImage e o `.deb` desta máquina foram gerados com o `cargo-packager` 0.11.8, instalado em `~/.cargo/bin` para isso.

## CI

> **CI/CD desligado (07/11):** por decisão do usuário, a cota do GitHub Actions estourou e os workflows foram desligados; a validação é local (`cargo test --workspace`, `cargo clippy`, `cargo fmt`, `cargo deny`) e nada deve reativar ou disparar workflows sem pedido do usuário. O release `v0.1.0-rc1` foi gerado pelo `release.yml` disparado à mão, em 19 e 20/11, fora desta regra de rotina. A tabela abaixo é o histórico até então.

| Workflow | Run | Resultado |
|---|---|---|
| `ci.yml`, último com sucesso | [42338149192](https://github.com/ChicoFigueiredo/Chess-Studio/actions/runs/42338149192) (PR #126, `bf3fade`) | success |
| `ci.yml`, `main` após os merges #121 a #126 | runs 42338929949, 42338957478, 42339939655, 42339943616 | `cancelled` pela regra de `concurrency` (merges em sequência); o último em `main`, [42362636990](https://github.com/ChicoFigueiredo/Chess-Studio/actions/runs/42362636990) (`5d4e00e`, após #127/#128), `success` |
| `release.yml` | [42362640315](https://github.com/ChicoFigueiredo/Chess-Studio/actions/runs/42362640315) (manual, `v0.0.0-rc3`) | `success` nos 4 jobs; draft completo |
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
| Pacotes do draft nunca instalados fora do CI | Média | Instalar o draft `v0.0.0-rc3` nas VMs/máquinas e rodar o smoke | Usuário |
| Pacotes sem assinatura (aviso do SO) | Baixa | Documentado em `docs/instalacao.md` | Usuário |
| Benchmark UI-12 contaminado por máquina ocupada; sem GPU integrada | Média | `scripts/benchmark_matriz.sh` com máquina ociosa | Usuário |
| F11 real, restauração de janela e atalho interceptado (UI-03) | Média | Roteiro manual em janela real | Usuário |
| `ttf-parser` sem manutenção (RUSTSEC-2026-0192), transitiva do `winit` | Baixa | Ignorado com justificativa em `deny.toml`; reavaliar ao atualizar o Bevy | Engenharia |
| Aprovação visual (VIS-01/02) e gestos de trackpad | Baixa | Revisão do usuário no Mac e no Linux | Usuário |

## O que depende do usuário

0. Percorrer o [roteiro do RC-1](../homologacao/rc1.md) (46 itens) e marcar o resultado no artefato.
1. Builds, testes e smoke nas VMs/máquinas Windows e macOS, seguindo `docs/build/` e `docs/instalacao.md`; instalar os pacotes do `v0.1.0-rc1` e abrir o app.
2. Smoke offline fora do Linux (rede desligada: 2P, Exercícios, Engine local).
3. Homologação visual e de interação (VIS-01/02, CAM-01/02, UI-03, UX-09/10) em janela real.
4. Benchmark limpo do UI-12 e dos perfis (máquina ociosa; GPU integrada): `scripts/benchmark_matriz.sh --minutes 5` (ou `scripts/rodar_benchmark_ui12.sh --matriz`).
5. GO da V1 ([`v1.md`](v1.md)) e GO do Gate D; depois, criar a tag `v*`, revisar o draft e publicar.

```text
RESULTADO: PENDENTE (aguarda GO do usuário)
```

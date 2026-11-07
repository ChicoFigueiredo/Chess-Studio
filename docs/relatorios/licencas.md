# Relatório D2 (parte 1): auditoria de licenças e smoke offline

Issue #57. Data: 2026-11-07. Escopo: licenças de código, crates, assets e Engines (ADR-0003, ADR-0004), e o smoke test offline (PLAT-02). Instaladores e CI ficam no PR do D1 (#112).

## Resultado

| Item | Resultado |
|---|---|
| `cargo deny check` (licenses, bans, advisories, sources) | PASS (antes: bans e advisories falhavam; corrigido, ver abaixo) |
| `THIRD-PARTY-LICENSES` regenerado | PASS (`scripts/gerar-licencas.sh`, cargo about) |
| Assets com licença registrada (CC0/OFL) | PASS |
| Engines (Stockfish, Lc0) só baixadas pelo usuário | PASS (teste automatizado + inspeção) |
| Smoke offline (PLAT-02), Linux | PASS (ver `smoke-offline`) |

## 1. Crates

`cargo deny check licenses` já passava (todas as dependências em MIT, Apache-2.0, ISC, BSD, Zlib, Unicode-3.0, MIT-0, CDLA-Permissive-2.0; nenhuma GPL/AGPL/LGPL). Saída resumida: `licenses ok`.

Problemas encontrados ao rodar `cargo deny check` completo, e correção:

1. **Aviso `license-not-encountered` para OFL-1.1** (e depois BSL-1.0, que deixou de ocorrer após a atualização do `zip`): removidos da allowlist de `deny.toml` e de `about.toml`. A OFL-1.1 vale só para as fontes Noto (asset, não crate) e é registrada em `assets/LICENSES.md`.
2. **`bans` FAILED, 5 crates com "wildcard dependency"**: falso positivo das dependências por caminho do workspace (`x.workspace = true`). Corrigido com `allow-wildcard-paths = true`; `wildcards = "deny"` continua valendo para dependências de registro.
3. **`advisories` FAILED, `zip 2.6.1`** (4 avisos: bit SUID/SGID, symlinks e abort em `extract`, `chess-uci` -> instalador do Stockfish). O código não chama `ZipArchive::extract` (grava só entradas de arquivo, validando o caminho), mas atualizamos para `zip 9.4.1` por precaução (única mudança de API: `ZipFile::name()` agora devolve `Result`); os 44 testes de unidade e as suítes de `chess-uci` seguem verdes.
4. **`advisories`, `ttf-parser 0.25.1` sem manutenção (RUSTSEC-2026-0192)**: transitiva do `winit` (`sctk-adwaita`, decoração de janela no Linux), sem correção disponível e sem processar dados do usuário. Ignorada em `deny.toml` com justificativa; reavaliar ao atualizar o Bevy.

Comandos e saída final:

```
$ cargo deny check
advisories ok, bans ok, licenses ok, sources ok
$ scripts/gerar-licencas.sh
gerado: THIRD-PARTY-LICENSES        # 501 MIT, 24 ISC, 23 Unicode-3.0, 19 Apache-2.0, 7 zlib, 4 MIT-0, 3 BSD-3, 1 BSD-2, 1 CDLA-Permissive-2.0
```

O `THIRD-PARTY-LICENSES` é reproduzível: `cargo install cargo-about --locked && scripts/gerar-licencas.sh` (configuração em `about.toml` e `about.hbs`, `ignore-dev-dependencies`, `ignore-build-dependencies`, sem dependência GPL).

## 2. Assets

`crates/chess-studio/assets/` contém: `models/chess_pieces.glb` (Poly Haven, CC0), 9 texturas JPG (`maple_*`, `walnut_*`, `oak_*`, Poly Haven, CC0), 2 fontes (`NotoSans-Regular.ttf`, `NotoSansSymbols2-chess.ttf`, SIL OFL 1.1, texto em `fonts/OFL.txt`) e `i18n/pt-BR/*.ftl` (texto do próprio projeto, MIT OR Apache-2.0). Todos os arquivos binários têm linha com origem, autor e licença em `assets/LICENSES.md`; nenhum asset sem registro, nenhuma licença incompatível (OFL permite redistribuir embutida, com o texto da licença junto).

## 3. Engines

- Nenhum binário de Engine está no repositório nem nos pacotes (`git ls-files` não lista executáveis de Engine; o catálogo `chess-uci` só guarda URL, tamanho e SHA-256).
- Stockfish (GPL-3.0) e Lc0 (GPL-3.0) são baixados **pelo usuário**, depois do consentimento (licença, link do código-fonte e tamanho mostrados antes) e executados como processo separado via UCI (ADR-0003).
- Teste automatizado novo, `crates/chess-studio/tests/no_network_without_consent.rs` (4 testes, todos PASS): só `chess-uci/src/install.rs` usa `ureq`; nenhuma outra crate declara cliente de rede nem usa `TcpStream`/`UdpSocket`; as funções de instalação só são chamadas do assistente e do gerenciador de Engines; no assistente, `start_download` só é chamado no ramo do botão de consentimento (`wizard-download-confirm`).

## 4. Smoke test offline (PLAT-02)

`scripts/smoke-offline.sh [binario]` roda o binário release dentro de um namespace de rede vazio (`unshare -n`, com fallback `unshare -rn`, depois `sudo -n unshare -n`; sem nenhum, falha em vez de fingir), com `HOME` e `XDG_DATA_HOME` temporários (perfil novo), `--screenshot <tmp> --size 960x540`. Verifica antes que o namespace realmente não alcança a rede, e depois: saída 0, 7 PNGs não vazios, `desempenho.txt`, e nenhuma Engine baixada sozinha em `ChessStudio/engines`. Alternativas se não houver `unshare`: desligar a rede, ou `docker run --network none`.

Execução (Linux, `unshare -rn`, build `cargo build --release -p chess-studio`, GPU NVIDIA RTX 4080 SUPER/Vulkan):

```
== verificando que o namespace não tem rede
ok: conexão externa recusada dentro do namespace
== executando .../release/chess-studio --screenshot /tmp/chess-studio-smoke.XXXXXX/capturas --size 960x540
PASS: saída 0, 7 capturas, offline, nenhuma Engine instalada sozinha
```

Capturas: `3d-profissional-{padrao,obliqua,superior}`, `mesa-imersiva-{padrao,obliqua,superior}`, `2d-tecnico` (6,7 a 13,4 ms/quadro). Windows e macOS não foram testados aqui (cobertos pelas VMs do D1/`docs/build/`): UNVERIFIED.

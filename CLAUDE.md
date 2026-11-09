# Chess Studio — instruções para agentes

Desktop nativo, offline, em Rust (Bevy 0.20 + bevy_egui + SQLite). Leia antes de trabalhar: [GLOSSARY.md](GLOSSARY.md), [docs/adr/](docs/adr/), [decisões da rodada 2](docs/decisoes/2026-11-06-rodada-2.md) e a issue de spec do marco em que você está.

## Regras do projeto

- **Linguagem:** termos de domínio do `GLOSSARY.md`; identificadores no código em inglês (o glossário dá o nome `_en_`). Interface em pt-BR via Fluent. Docs, ADRs, issues e PRs em português.
- **Autoridade única (ADR-0001):** só o controlador da Partida escreve a Partida. Bevy lê o estado e envia intenções; a Posição visual provisória nunca muta a Partida. Regras, arbitragem, relógio e treino não dependem do Bevy.
- **Log de eventos (ADR-0002):** a Partida é reconstruída do log; não existe "Salvar". O tempo vem de instantes injetados (relógio falso nos testes), nunca de `Instant::now()` dentro do domínio.
- **Licenças (ADR-0004):** MIT OR Apache-2.0. Nenhuma dependência GPL/AGPL/LGPL; `cargo deny check licenses` precisa passar. Assets só CC0 ou compatíveis, com licença registrada.
- **Engines (ADR-0003):** nunca embutir binários de Engine; toda proposta da Engine é validada pelo núcleo de regras.
- **Câmeras (issues #82 e #142):** todas as câmeras que renderizam na mesma janela (3D, 2D técnico, egui e futuras) precisam do **mesmo `Msaa` e do mesmo `Hdr`**; câmeras diferentes nisso não compartilham a textura principal: o 3D pode sair preto (MSAA) ou a interface ser composta sobre quadros antigos, com janela e tooltip duplicados (HDR do bloom). Nova câmera não define `Msaa` nem `Hdr` próprios: `presentation::apply_quality` aplica os de `GraphicsQuality` a todas; bloom e tonemapping ficam só na câmera 3D.
- **TDD:** teste primeiro, nas interfaces acordadas — `chess-core` (regras), `chess-game` (comando + instante → eventos; interface principal), intérprete de gestos, geometria/picking, `chess-store`, `chess-uci`. Teste comportamento externo, não detalhes internos.
- **Não declarar pronto** sem evidência: cada critério da matriz (CORE-, INT-, AC-, UX-, DB-, UCI-, TRN-, PERF-…) termina como PASS/FAIL/UNVERIFIED/N/A com teste ou evidência.

## Comandos

- `cargo test --workspace` — suíte completa
- `cargo test -p <crate> --test <arquivo>` — um arquivo de teste
- `cargo clippy --workspace --all-targets -- -D warnings` — lint (precisa passar)
- `cargo fmt --all --check`
- `cargo deny check licenses` — licenças (ADR-0004; instalar com `cargo install cargo-deny --locked`)
- `cargo run -p chess-studio` — abre a janela (`--features dynamic_linking` acelera a recompilação em dev)

## Fluxo de trabalho

- Um ticket (issue) = um branch `chess-<n>-<slug>` = um PR que fecha a issue (`Closes #n`).
- PRs de visual/UX, conteúdo de Exercícios e relatórios de Gate levam a label `ready-for-human` e esperam o merge do usuário; os demais são revisados e mergeados pelo supervisor.
- Commits terminam com `Co-Authored-By: Claude …`; PRs terminam com a linha "Generated with Claude Code".

## Agent skills

### Issue tracker

Issues, épicos e features vivem no GitHub Issues de `ChicoFigueiredo/Chess-Studio`, com Project e Milestones por Gate. See `docs/agents/issue-tracker.md`.

### Triage labels

Os cinco papéis canônicos com os nomes padrão (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `GLOSSARY.md` e `docs/adr/` na raiz. See `docs/agents/domain.md`.

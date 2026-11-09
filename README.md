# Chess Studio

[![CI](https://github.com/ChicoFigueiredo/Chess-Studio/actions/workflows/ci.yml/badge.svg)](https://github.com/ChicoFigueiredo/Chess-Studio/actions/workflows/ci.yml) · CI/CD: [docs/ci.md](docs/ci.md)

Mesa pessoal de treinamento de xadrez: desktop nativo, offline e multiplataforma (Windows, Linux, macOS), para aprender a jogar no tabuleiro físico **sem depender de auxílios digitais**. Tabuleiro 3D sobre uma mesa discreta, 2D técnico, três Políticas de arbitragem (Assistido, Simulação FIDE, Eliminação Imediata), adversário por Engine UCI externa, Exercícios de regras, finais e memória, tudo gravado localmente em SQLite.

> **Estado:** P0, V1 e V2 (Súmula manual, conferência e impressão A4) estão na `main`: Partida nas três Políticas, relógio, Engine UCI, SQLite, PGN/FEN, Exercícios, painéis, F11 e Foco, HUD, gerenciador de Engines e Súmula. O **CI/CD está desligado** por decisão do usuário em 07/11 (a cota do GitHub Actions estourou): a validação é local (`cargo test --workspace`, clippy, fmt, `cargo deny`) e os workflows não rodam sozinhos; o release (`release.yml`) só roda manualmente e gera um **draft**. A **homologação está em andamento**: o usuário já passou parte do checklist em janela real em 09/11 (ver [P0](docs/relatorios/p0.md)); faltam reteste das correções, Windows/macOS, avaliação visual, benchmark limpo e o GO do usuário. Nada aqui é evidência de software pronto até a matriz de aceite marcar `PASS`. Situação do release: [docs/relatorios/gate-d.md](docs/relatorios/gate-d.md); V2: [docs/relatorios/v2.md](docs/relatorios/v2.md).

## Rodar e testar

Pré-requisito: [rustup](https://rustup.rs) (o toolchain certo é instalado sozinho pelo `rust-toolchain.toml`). Guias por sistema, com as dependências de cada um: [Linux](docs/build/linux.md) · [Windows](docs/build/windows.md) · [macOS](docs/build/macos.md). Para instalar um pacote pronto: [docs/instalacao.md](docs/instalacao.md).

```bash
git clone https://github.com/ChicoFigueiredo/Chess-Studio.git && cd Chess-Studio
cargo run -p chess-studio            # abre o app (a primeira compilação demora)
cargo test --workspace               # suíte automatizada
cargo run -p chess-studio --release -- --benchmark          # benchmark de 5 min (JSON + CSV em benchmarks/out/)
cargo run -p chess-studio -- --screenshot docs/evidencias/x # gera capturas e sai
scripts/benchmark_matriz.sh --minutes 5                     # matriz de benchmark do UI-12 (ver docs/benchmark.md)
scripts/smoke-offline.sh                                    # smoke test offline (Linux, binário release)
```

**Controles**

| Ação | Mouse / teclado |
|---|---|
| Mover peça | clique na peça e na casa de destino, ou arraste e solte |
| Orbitar a câmera | botão do meio + mover (trackpad: dois dedos) |
| Deslocar (pan) | Shift + botão do meio, ou botão direito + mover |
| Zoom | roda (trackpad: pinça) |
| Vistas | `1` superior · `2` oblíqua · `3` padrão · `4` invertida · `0` reset |
| Apresentação | `Tab` alterna 3D profissional / Mesa imersiva / 2D técnico |
| Cancelar gesto | `Esc` |

Jogar contra o Stockfish (opcional): instale-o pelo gerenciador do sistema ou baixe do [site oficial](https://stockfishchess.org/download/) e registre-o no assistente/gerenciador de Engines do app (que faz o handshake UCI), ou informe o caminho do executável no painel. O Chess Studio nunca executa uma Engine sem a sua ação: `CHESS_STUDIO_ENGINE`, o `PATH` e o caminho digitado são apenas candidatos; antes de executar, o app mostra um aviso, você confirma e ele verifica que o arquivo fala UCI (Engines já registradas no gerenciador dispensam o aviso). Ao reabrir uma Partida contra a Engine, nada é executado até você clicar em "Conectar Engine e retomar". O Chess Studio nunca distribui Engines ([ADR-0003](docs/adr/0003-engines-externas-download-fixado.md)).

**O que testar:** a lista de critérios de aceite manuais fica no artefato de progresso do projeto, na seção **Homologação** (cada item tem passos e campos PASS/FAIL/N/A com observação, e um botão para baixar o JSON). Os resultados de cada Gate ficam em [docs/relatorios/](docs/relatorios/) (P0, V1, [V2](docs/relatorios/v2.md), licenças e [Gate D](docs/relatorios/gate-d.md)). Atualizar e recuperar dados: [docs/atualizacao-e-recuperacao.md](docs/atualizacao-e-recuperacao.md).

## Documentos

| Documento | Para quê |
|---|---|
| [Prompt mestre](docs/prompts/veronica/Chess_Studio_Prompt_Mestre_Consolidado_P0_V1_V2.md) | Especificação original consolidada (fonte histórica, não editada) |
| [Decisões da Rodada 2](docs/decisoes/2026-11-06-rodada-2.md) | DEC-21 a DEC-51: pendências resolvidas e detalhes do P0 (prevalecem sobre o prompt mestre) |
| [GLOSSARY.md](GLOSSARY.md) | Linguagem de domínio canônica (pt-BR, com o nome usado no código) |
| [docs/adr/](docs/adr/) | Decisões de arquitetura difíceis de reverter |
| [Issues e Project](https://github.com/ChicoFigueiredo/Chess-Studio/issues) | Controle mestre: épicos por Marco, features por Área, tickets por Gate |

## Marcos

- **P0** — protótipo jogável, offline e homologável: regras completas, 3 Apresentações, 3 Políticas de arbitragem, relógio, Engine UCI, SQLite, PGN/FEN, treino mínimo.
- **V1** — estação de estudo: painéis, F11 e modo Foco, HUD, biblioteca, gerenciador de Engines.
- **V2** — súmula manual, conferência e impressão A4 (na `main`, homologação pendente).

## Licença

Licenciado sob sua escolha de [MIT](LICENSE-MIT) ou [Apache-2.0](LICENSE-APACHE). Engines como o Stockfish são software de terceiros, nunca distribuídos com o app (ver [ADR-0003](docs/adr/0003-engines-externas-download-fixado.md)).

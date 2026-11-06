# Chess Studio

Mesa pessoal de treinamento de xadrez: desktop nativo, offline e multiplataforma (Windows, Linux, macOS), para aprender a jogar no tabuleiro físico **sem depender de auxílios digitais**. Tabuleiro 3D sobre uma mesa discreta, 2D técnico, três Políticas de arbitragem (Assistido, Simulação FIDE, Eliminação Imediata), adversário por Engine UCI externa, Exercícios de regras, finais e memória, tudo gravado localmente em SQLite.

> **Estado:** P0 em implementação — a Partida Assistido já é jogável no app; Simulação FIDE, Engine, persistência no app e treino estão chegando. Nada aqui é evidência de software pronto até a matriz de aceite marcar `PASS`.

## Rodar e testar

Pré-requisito: [rustup](https://rustup.rs) (o toolchain certo é instalado sozinho pelo `rust-toolchain.toml`). Guias por sistema, com as dependências de cada um: [Linux](docs/build/linux.md) · [Windows](docs/build/windows.md) · [macOS](docs/build/macos.md). Para instalar um pacote pronto: [docs/instalacao.md](docs/instalacao.md).

```bash
git clone https://github.com/ChicoFigueiredo/Chess-Studio.git && cd Chess-Studio
cargo run -p chess-studio            # abre o app (a primeira compilação demora)
cargo test --workspace               # suíte automatizada
cargo run -p chess-studio --release -- --benchmark          # benchmark de 5 min (JSON + CSV em benchmarks/out/)
cargo run -p chess-studio -- --screenshot docs/evidencias/x # gera capturas e sai
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

Jogar contra o Stockfish (opcional): instale-o pelo gerenciador do sistema ou baixe do [site oficial](https://stockfishchess.org/download/) e deixe `stockfish` no `PATH` (ou aponte `CHESS_STUDIO_ENGINE` para o executável). O Chess Studio nunca distribui Engines ([ADR-0003](docs/adr/0003-engines-externas-download-fixado.md)).

**O que testar:** a lista de critérios de aceite manuais fica no artefato de progresso do projeto, na seção **Homologação** (cada item tem passos e campos PASS/FAIL/N/A com observação, e um botão para baixar o JSON). Os resultados de cada Gate ficam em [docs/relatorios/](docs/relatorios/).

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
- **V2** — súmula manual e conferência.

## Licença

Licenciado sob sua escolha de [MIT](LICENSE-MIT) ou [Apache-2.0](LICENSE-APACHE). Engines como o Stockfish são software de terceiros, nunca distribuídos com o app (ver [ADR-0003](docs/adr/0003-engines-externas-download-fixado.md)).

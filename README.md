# Chess Studio

Mesa pessoal de treinamento de xadrez: desktop nativo, offline e multiplataforma (Windows, Linux, macOS), para aprender a jogar no tabuleiro físico **sem depender de auxílios digitais**. Tabuleiro 3D sobre uma mesa discreta, 2D técnico, três Políticas de arbitragem (Assistido, Simulação FIDE, Eliminação Imediata), adversário por Engine UCI externa, Exercícios de regras, finais e memória, tudo gravado localmente em SQLite.

> **Estado:** especificação aprovada; implementação do P0 em andamento. Nada aqui é evidência de software pronto até a matriz de aceite marcar `PASS`.

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

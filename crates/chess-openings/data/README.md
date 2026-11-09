# Dados de aberturas

Os arquivos `a.tsv` a `e.tsv` são cópias, sem alteração, do conjunto
[lichess-org/chess-openings](https://github.com/lichess-org/chess-openings) (colunas `eco`, `name`, `pgn`).

- **Licença: CC0 1.0** (domínio público). O `README.md` do repositório de origem declara: "As a collection of
  facts, this data set is in the public domain [...] the work is released under the CC0 Public Domain
  Dedication"; a API do repositório também informa `spdx_id: CC0-1.0`. Compatível com a ADR-0004.
- Commit de origem: `3ba6b20d31b2e6125cb0e6a2e792f400717d9d39` (2026-11-05).
- Baixados em 2026-11-09. Para atualizar, substitua os cinco arquivos e rode `cargo test -p chess-openings`.

O código do crate é MIT OR Apache-2.0; só estes arquivos de dados são CC0.

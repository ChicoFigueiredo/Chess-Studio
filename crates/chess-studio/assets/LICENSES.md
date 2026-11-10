# Licenças dos assets

Todos os assets de modelo e textura são **CC0 1.0** (domínio público, sem atribuição obrigatória;
registramos a origem mesmo assim, conforme DEC-47 e ADR-0004). As fontes são **SIL OFL 1.1** (Noto, Inter e JetBrains Mono); os ícones Phosphor são uma dependência Rust (`egui-phosphor`, MIT OR Apache-2.0).
Tamanho total do diretório `assets/`: cerca de 11 MB (orçamento: 30 MB).

Data de download de tudo: **2026-11-06**.

| Arquivo | Origem | Autor | Licença | Observações |
|---|---|---|---|---|
| `models/chess_pieces.glb` | [Poly Haven — Chess Set](https://polyhaven.com/a/chess_set) (glTF 1k, `https://dl.polyhaven.org/file/ph-assets/Models/gltf/1k/chess_set/chess_set.gltf`) | Riley Queen | CC0 1.0 | Só as 32 peças (o tabuleiro de mármore do modelo foi removido); 1 nó por peça, texturas 1k embutidas (JPG), UVs secundárias descartadas. |
| `textures/maple_{diff,nor_gl,arm}_1k.jpg` | [Poly Haven — White Maple Veneer](https://polyhaven.com/a/white_maple_veneer) (1k, JPG) | Jenelle van Heerden | CC0 1.0 | Casas claras. Sem alteração. |
| `textures/walnut_{diff,nor_gl,arm}_1k.jpg` | [Poly Haven — Black Walnut Veneer 01](https://polyhaven.com/a/black_walnut_veneer_01) (1k, JPG) | Jenelle van Heerden | CC0 1.0 | Casas escuras e moldura. Sem alteração. |
| `textures/oak_{diff,nor_gl,arm}_1k.jpg` | [Poly Haven — Oak Veneer 01](https://polyhaven.com/a/oak_veneer_01) (1k, JPG) | Jenelle van Heerden | CC0 1.0 | Mesa da Mesa imersiva (B19), dessaturada por tinta em tempo de execução. Sem alteração do arquivo. Substitui `table_*` (Wood Table 001, veio avermelhado), removido. |
| `fonts/NotoSansSymbols2-chess.ttf` | [Noto Sans Symbols 2](https://github.com/notofonts/symbols) (`NotoSansSymbols2-Regular.ttf`) | The Noto Project Authors | SIL OFL 1.1 (`fonts/OFL.txt`) | Subconjunto com os glifos de xadrez U+2654–U+265F (`pyftsubset`), usado no 2D técnico. |
| `fonts/NotoSans-Regular.ttf` | Noto Sans (pacote `fonts-noto-core`, 2020-12) | The Noto Project Authors | SIL OFL 1.1 (`fonts/OFL.txt`) | Fonte da interface egui (texto pt-BR com acentos), sem alteração; também embutida (subconjunto) nos PDFs da Súmula (V2-SUM-03). As fontes embutidas do egui usam a Ubuntu Font Licence e por isso não são ativadas. |
| `fonts/Inter-Regular.ttf`, `fonts/Inter-SemiBold.ttf` | [Inter 4.1](https://github.com/rsms/inter/releases/tag/v4.1) (`Inter-4.1.zip`, `extras/ttf/`) | The Inter Project Authors | SIL OFL 1.1 (`fonts/OFL-Inter.txt`) | Fonte da interface no design system (DS-12, #160): regular e semibold (`body-strong`), sem alteração. Baixado em 2026-11-09. SHA-256: `40d692fc…0c82` (Regular), `78a843fa…cde3` (SemiBold). |
| `fonts/JetBrainsMono-Regular.ttf` | [JetBrains Mono 2.304](https://github.com/JetBrains/JetBrainsMono/releases/tag/v2.304) (`JetBrainsMono-2.304.zip`, `fonts/ttf/`) | The JetBrains Mono Project Authors | SIL OFL 1.1 (`fonts/OFL-JetBrainsMono.txt`) | Log UCI e FEN (estilo `log`), sem alteração. Baixado em 2026-11-09. SHA-256: `a0bf60ef…138f`. |

Os ícones Phosphor vêm do crate [`egui-phosphor`](https://crates.io/crates/egui-phosphor) 0.14.0 (MIT OR Apache-2.0), que embute a fonte de ícones Phosphor (MIT) no peso regular; a licença da dependência entra no `THIRD-PARTY-LICENSES` (`scripts/gerar-licencas.sh`).

As peças em madeira (B19) não têm arquivos novos: buxo = bordo (`maple_*`) com tinta amarelada; ébano = nogueira negra (`walnut_*`) com tinta quase preta, ambos com materiais PBR próprios no lugar dos materiais do glTF (`board.rs::piece_wood`).

`diff` = cor base (sRGB), `nor_gl` = normal (convenção OpenGL), `arm` = oclusão/rugosidade/metal.

## Como o modelo foi preparado

O Blender não é necessário: o glTF original da Poly Haven (`.gltf` + `.bin` + texturas separadas) foi
reempacotado em um único `.glb` por script Python (sem alterar a geometria): removido o nó `board`
(tabuleiro de mármore, substituído pelo tabuleiro de madeira gerado por código), removidos
dados não usados, texturas embutidas. Escala e posição das peças são ajustadas em tempo de execução
(`crates/chess-studio/src/board.rs`), a partir do nome dos nós (`piece_<tipo>_<cor>[_NN]`).

## Dados de aberturas (fora de `assets/`)

| Arquivo | Origem | Autor | Licença | Observações |
|---|---|---|---|---|
| `crates/chess-openings/data/{a,b,c,d,e}.tsv` | [lichess-org/chess-openings](https://github.com/lichess-org/chess-openings) (commit `3ba6b20d31b2e6125cb0e6a2e792f400717d9d39`, 2026-11-05) | Colaboradores do Lichess | CC0 1.0 (README da origem: "released under the CC0 Public Domain Dedication"; API do repositório: `spdx_id: CC0-1.0`) | Cópia sem alteração, embutida no binário (`include_str!`) para o painel Aberturas (C19, #147). Baixado em 2026-11-09. Detalhes em `crates/chess-openings/data/README.md`. |

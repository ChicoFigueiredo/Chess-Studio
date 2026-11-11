# Evidência: rating FIDE, dados guardados e Partidas importadas por Origem (#242)

Capturas com dados **fictícios** (jogadores "Exemplo, ...", nenhuma rede, banco em memória), em Escuro (`escuro/`) e Claro (`claro/`).

```
chess-studio --screenshot <pasta> --single <nome> --size 1600x1000 --backstage-page connections|local-profile \
  --theme dark|light --connections-demo [--connections-open fide] [--connections-fide-search] \
  [--connections-clear-confirm] [--connections-scroll <px>]
chess-studio --screenshot <pasta> --single <nome> --size 1400x900 --panel --tabs library \
  --library imports|imports-confirm --theme dark|light
```

| Arquivo | O que mostra |
|---|---|
| `central-fide.png` | Cartão da FIDE: nome, título, federação e as caixas Standard, Rápido e Blitz separadas (um ritmo sem rating mostra "—"), "FIDE, via Lichess, consultado em ... (há 2 dias)", o aviso de que a data é a da consulta e o botão Atualizar. |
| `central-fide-busca.png` | Busca por nome: três resultados com os ratings separados, "Vinculado", "Vincular este ID" e o selo Inativo. |
| `perfil-local-fide.png` | Página Arquivo > Perfil local com o mesmo bloco. |
| `central-limpar-dados.png` | Faixa "Dados guardados" e a confirmação "Limpar dados guardados?" com o que some e o que fica. |
| `biblioteca-origens.png` | Aba Origens da Biblioteca: "Tudo de um Provedor" e "Por importação" (nome, modo, data, contagem). |
| `biblioteca-apagar.png` | Confirmação com a contagem, a lista do que será apagado e "As Partidas que você jogou ... não são tocadas". |

Os estados sem captura (consulta em andamento, Lichess desligado, falhas, "ficam porque também vieram de outra origem") são cobertos por testes: [`fide_profile.rs`](../../../crates/chess-studio/src/fide_profile.rs), [`library_bulk.rs`](../../../crates/chess-studio/src/library_bulk.rs), [`fide_and_bulk.rs`](../../../crates/chess-store/tests/fide_and_bulk.rs).

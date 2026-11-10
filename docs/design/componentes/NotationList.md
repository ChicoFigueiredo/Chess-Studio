# NotationList

O Painel Notação lista os Lances em SAN pt-BR, em três colunas: número, brancas, pretas.

- Números em `ink-muted`; o lance atual em `accent-subtle` e `accent-ink`. Clicar num lance navega sem alterar a Partida.
- **egui:** `egui::Grid` com 3 colunas dentro de uma `ScrollArea` que segue o lance atual.

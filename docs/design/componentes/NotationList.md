# NotationList

O Painel Notação lista os Lances em SAN pt-BR, em três colunas: número, brancas, pretas.

- Números em `ink-muted`; o lance atual em `accent-subtle` e `accent-ink`. Clicar num lance navega sem alterar a Partida.
- **egui:** `egui::Grid` com 3 colunas dentro de uma `ScrollArea` que segue o lance atual.
- **Nota rápida (#180):** segunda vista da Anotação de estudo ao lado de cada Lance: indicador e 1ª linha (até 14 caracteres) em `ink-muted`; clicar abre um campo de uma linha (Enter salva, Esc cancela, Shift+Enter salva e abre o painel Anotações naquele Lance). A edição troca só a 1ª linha e preserva as demais; oculta em Partida competitiva e em Exercício de Treino.

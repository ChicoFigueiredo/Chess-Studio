# NotationList

O Painel Notação lista os Lances em SAN pt-BR, em três colunas: número, brancas, pretas.

- Números em `ink-muted`, tabulares; o lance atual em `accent-subtle` e `accent-ink`. Clicar num lance navega sem alterar a Partida.
- **Nota rápida:** segunda vista da Anotação de estudo ao lado de cada Lance: indicador e primeira linha (até 14 caracteres) em `ink-muted`. Clicar abre um campo de uma linha: Enter salva, Esc cancela, Shift+Enter salva e abre o Painel Anotações naquele Lance. A edição troca só a primeira linha e preserva as demais. Fica oculta em Partida competitiva e em Exercício de Treino.
- **egui:** `egui::Grid` com 3 colunas dentro de uma `ScrollArea` que segue o lance atual.

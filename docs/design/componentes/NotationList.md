# NotationList

O Painel Notação lista os Lances em SAN pt-BR, em três colunas: número, brancas, pretas.

- Números em `ink-muted`, tabulares; o lance atual em `accent-subtle` e `accent-ink`. Clicar num lance navega sem alterar a Partida.
- **Nota rápida:** segunda vista da Anotação de estudo ao lado de cada Lance: indicador e primeira linha (até 14 caracteres) em `ink-muted`. Clicar abre um campo de uma linha: Enter salva, Esc cancela, Shift+Enter salva e abre o Painel Anotações naquele Lance. A edição troca só a primeira linha e preserva as demais. Fica oculta em Partida competitiva e em Exercício de Treino.
- **egui:** `egui::Grid` com 3 colunas dentro de uma `ScrollArea` que segue o lance atual.
- **Botões e teclas (#321, #323):** o cabeçalho traz os quatro botões (início, anterior, próximo, posição atual); ← → Home End valem na janela inteira, exceto em campo de texto.
- **Linha de estudo (#323):** enquanto o Modo de estudo está ligado, a lista da Partida **congela** no topo, com o Lance de onde a linha partiu destacado (`accent-subtle`), e **abaixo, empilhada** (nunca lado a lado), vem a lista "Linha de estudo": o mesmo `Grid` de três colunas (número, brancas, pretas), a mesma tipografia, numeração a partir do Lance de partida e o Lance atual da linha em `accent-subtle`. Sem Lances ainda, a lista mostra a dica "Mova uma peça para começar a linha." A lista só existe durante o estudo.
- **Navegação no estudo:** clicar num Lance da linha vai até ele, ainda no estudo. Início, anterior e próximo (botões e ← → Home) andam **dentro da linha** e nunca a descartam; o botão de fim e a tecla End **encerram o estudo** e voltam à Partida ao vivo. Clicar num Lance da lista da Partida também encerra o estudo e mostra esse Lance. Lances novos da Partida (Engine adversária) seguem entrando na lista de cima.
- **Medidas:** cabe no painel estreito (cerca de 230 px) e em 1280x720, nos Temas claro e escuro; a rolagem do painel cobre as duas listas.

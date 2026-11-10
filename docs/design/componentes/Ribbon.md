# Ribbon

O Ribbon é o menu superior em abas (Arquivo, Início, Análise, Biblioteca, Treino, Exibir), com grupos de Ações. Abas contextuais (ex.: Súmula) aparecem em `accent-ink` quando fazem sentido.

- **Clássico**: botões grandes (ícone `icon-l` e rótulo) para a Ação principal do grupo e pilhas de botões pequenos para as demais; rótulo do grupo embaixo, em `caption`.
- **Simples**: uma linha (`ribbon-simple-h`) com ícone `icon-s` e rótulo. Grupos que não cabem vão para o menu "⋯", da direita para a esquerda.
- **Recolhido** (Ctrl+F1): só as abas; o corpo abre por cima do tabuleiro ao clicar numa aba e fecha ao executar a Ação.
- Aba ativa: texto `ink` e sublinhado `accent` de 3px. A aba Arquivo é sempre `accent`/`on-accent` e abre os bastidores.
- Todo item é uma Ação da lista única: rótulo, ícone, atalho, estado e KeyTip vêm dela.
- O modo do Ribbon e a aba ativa fazem parte do Perfil de interface.
- **egui:** `TopBottomPanel::top` com `ui.horizontal` por grupo e um widget próprio `ribbon_group`. O modo simples mede os grupos e move os excedentes para um `menu_button`.

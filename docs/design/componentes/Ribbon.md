# Ribbon

O Ribbon é o menu superior em abas (Arquivo, Início, Análise, Biblioteca, Treino, Exibir), com grupos de Ações. Abas contextuais aparecem em `accent-ink` quando fazem sentido: Tabuleiro (nas Apresentações 3D profissional e Mesa imersiva) e Súmula.

- **Clássico** (`ribbon-classic-h`, 96px): botões grandes (ícone `icon-l` e rótulo em `ribbon-label`, até duas linhas) para a Ação principal do grupo e pilhas de até três botões pequenos de 24px para as demais; rótulo do grupo embaixo, em `caption`, sempre na mesma linha de base.
- **Simples**: uma linha (`ribbon-simple-h`) com ícone `icon-s` e rótulo. Grupos que não cabem vão para o menu "⋯", da direita para a esquerda.
- **Recolhido** (Ctrl+F1): só as abas; o corpo abre por cima do tabuleiro ao clicar numa aba e fecha ao executar a Ação.
- Aba ativa: texto `ink` e sublinhado `accent` de 3px. A aba Arquivo é sempre `accent`/`on-accent` e abre os bastidores.
- **Início** começa pelo grupo Nova Partida (Nova Partida, Contra a Engine, Dois jogadores, Retomar).
- **Exibir** tem uma caixa de seleção por Painel, como no Word, e o grupo Layout de Tela: uma lista com os layouts de fábrica e os salvos, e o botão "Salvar layout…", que abre um diálogo para nomear, renomear e excluir.
- **Tabuleiro** (contextual) liga e desliga o Relógio 3D, as Peças tomadas 3D, as coordenadas e a Análise.
- Todo item é uma Ação da lista única: rótulo, ícone, atalho, estado e KeyTip vêm dela.
- O modo do Ribbon e a aba ativa fazem parte do Perfil de interface.
- **egui:** `TopBottomPanel::top` com `ui.horizontal` por grupo e um widget próprio `ribbon_group`. O modo simples mede os grupos e move os excedentes para um `menu_button`.

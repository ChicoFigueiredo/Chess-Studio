# Button

O botão executa uma Ação. Há três variantes: primário, secundário e sutil.

- **Primário** (`accent` + `on-accent`): uma por tela ou diálogo, para a Ação que conclui a tarefa (Iniciar Partida, Fechar Opções).
- **Secundário** (`surface-2` com contorno `border-strong`) é o padrão.
- **Sutil** (sem fundo): botões de ícone no Ribbon simples, na Barra de Acesso Rápido e no cabeçalho dos Painéis.
- **Destrutivo**: Ações irreversíveis (Abandonar, Excluir perfil) usam o texto em `danger` e sempre pedem confirmação.
- Os estados são sempre os cinco: repouso, hover (`surface-3`), pressionado (`surface-4`), desabilitado (`ink-disabled`) e foco (anel `focus-ring` de 2px). Um botão desabilitado explica o motivo no tooltip.
- O rótulo é um verbo no infinitivo ou o nome da Ação, como no Ribbon. Botão só com ícone exige tooltip com nome e atalho.
- **egui:** widget `cs_button(ui, &Action, Variant)` sobre `egui::Button`, com `Visuals.widgets.{inactive,hovered,active}` preenchidos pelos tokens. Altura `control-h` (Confortável) ou `control-h-compact`.

# PanelTabs

O cabeçalho de um grupo do dock mostra uma aba por Painel e, à direita, Flutuar, Destacar e o menu "⋯".

- Aba ativa: `surface-1` com traço superior `accent`. As inativas ficam em `ink-muted`.
- Cada aba tem um botão X que fecha o Painel; ele volta pela aba Exibir do Ribbon ou pela faixa de ícones à direita do dock.
- Sem espaço, as abas inativas encolhem com reticências; a aba ativa e os botões da direita nunca somem.
- O menu do Painel tem Flutuar, Destacar em janela, Encaixar (só fora do dock) e Fechar.
- Duplo clique na aba alterna entre encaixado e flutuante.
- A faixa de ícones na borda direita mostra um ícone por Painel: clicar abre ou fecha o Painel; um Painel destacado fica marcado.
- **egui:** `egui_tiles::Behavior::tab_ui` e `top_bar_right_ui` com os tokens.

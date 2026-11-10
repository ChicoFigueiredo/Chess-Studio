# PanelTabs

O cabeçalho de um grupo do dock mostra uma aba por Painel e, à direita, Flutuar, Destacar e o menu "⋯".

- Aba ativa: `surface-1` com traço superior `accent`. As inativas ficam em `ink-muted`.
- O menu do Painel tem Flutuar, Destacar em janela, Encaixar (só fora do dock) e Fechar.
- Duplo clique na aba alterna entre encaixado e flutuante.
- **egui:** `egui_tiles::Behavior::tab_ui` e `top_bar_right_ui` com os tokens.
